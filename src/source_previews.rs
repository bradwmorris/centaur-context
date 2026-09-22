//! Optional, bounded, same-origin Source previews. No canonical data is mutated.
use axum::{
    Router,
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use reqwest::{Client, Url};
use scraper::{Html, Selector};
use serde::Deserialize;
use sqlx::PgPool;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Semaphore};
use uuid::Uuid;

const PAGE_LIMIT: usize = 1_048_576;
const IMAGE_LIMIT: usize = 2_097_152;
const CACHE_ENTRIES: usize = 32;
const SUCCESS_TTL: Duration = Duration::from_secs(3600);
const FAILURE_TTL: Duration = Duration::from_secs(300);
const REFRESH_FLOOR: Duration = Duration::from_secs(60);

#[derive(Clone)]
struct Image {
    bytes: Vec<u8>,
    mime: &'static str,
}
#[derive(Clone)]
struct Entry {
    image: Option<Image>,
    fetched: Instant,
}
#[derive(Clone)]
struct PreviewState {
    pool: PgPool,
    enabled: bool,
    cache: Arc<Mutex<HashMap<String, Entry>>>,
    slots: Arc<Semaphore>,
    pending: Arc<Semaphore>,
}
#[derive(Default, Deserialize)]
struct Options {
    #[serde(default)]
    refresh: bool,
}

pub fn router(pool: PgPool, enabled: bool) -> Router {
    Router::new()
        .route("/api/v2/sources/{id}/thumbnail", get(preview))
        .with_state(PreviewState {
            pool,
            enabled,
            cache: Arc::new(Mutex::new(HashMap::new())),
            slots: Arc::new(Semaphore::new(2)),
            pending: Arc::new(Semaphore::new(16)),
        })
}

async fn preview(
    State(state): State<PreviewState>,
    Path(id): Path<Uuid>,
    Query(options): Query<Options>,
) -> Response {
    if !state.enabled {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Ok(source) = crate::db::get_source(&state.pool, id).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if source.lifecycle != "active" {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(uri) = source.canonical_uri else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(url) = checked_url(&uri) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let key = url.to_string();
    let cached = state.cache.lock().await.get(&key).cloned();
    if let Some(entry) = cached.filter(|e| fresh(e, options.refresh)) {
        return image_response(entry.image);
    }
    // Bound both active fetches and queued requests; a Grid must not fail every
    // visible image beyond the first two simply because it renders concurrently.
    let Ok(_pending) = state.pending.clone().try_acquire_owned() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Ok(Ok(_permit)) =
        tokio::time::timeout(Duration::from_secs(20), state.slots.clone().acquire_owned()).await
    else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    // A second requester may have populated the cache while this request loaded its Source.
    if let Some(entry) = state
        .cache
        .lock()
        .await
        .get(&key)
        .cloned()
        .filter(|e| fresh(e, options.refresh))
    {
        return image_response(entry.image);
    }
    let image = tokio::time::timeout(Duration::from_secs(15), resolve(url))
        .await
        .ok()
        .flatten();
    let mut cache = state.cache.lock().await;
    if cache.len() >= CACHE_ENTRIES
        && !cache.contains_key(&key)
        && let Some(oldest) = cache
            .iter()
            .min_by_key(|(_, e)| e.fetched)
            .map(|(k, _)| k.clone())
    {
        cache.remove(&oldest);
    }
    cache.insert(
        key,
        Entry {
            image: image.clone(),
            fetched: Instant::now(),
        },
    );
    image_response(image)
}
fn fresh(entry: &Entry, refresh: bool) -> bool {
    entry.fetched.elapsed()
        < if refresh {
            REFRESH_FLOOR
        } else if entry.image.is_some() {
            SUCCESS_TTL
        } else {
            FAILURE_TTL
        }
}
fn image_response(image: Option<Image>) -> Response {
    let Some(image) = image else {
        return ([(header::CACHE_CONTROL, "no-store")], StatusCode::NOT_FOUND).into_response();
    };
    (
        [
            (header::CONTENT_TYPE, image.mime),
            (header::CACHE_CONTROL, "private, max-age=60"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::REFERRER_POLICY, "no-referrer"),
        ],
        Body::from(image.bytes),
    )
        .into_response()
}

fn checked_url(raw: &str) -> Result<Url, ()> {
    if raw.len() > 4096 {
        return Err(());
    }
    let mut url = Url::parse(raw).map_err(|_| ())?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
    {
        return Err(());
    }
    let host = url.host_str().ok_or(())?;
    if host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.ends_with(".internal")
    {
        return Err(());
    }
    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>()
        && !public_ip(ip)
    {
        return Err(());
    }
    url.set_fragment(None);
    Ok(url)
}
fn public_ip(ip: IpAddr) -> bool {
    let IpAddr::V4(ip) = ip else {
        return false;
    };
    let [a, b, c, _] = ip.octets();
    !(a == 0
        || a == 10
        || a == 127
        || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && (b == 168 || (b == 0 && (c == 0 || c == 2)) || (b == 88 && c == 99)))
        || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
        || (a == 203 && b == 0 && c == 113))
}

async fn fetch(mut url: Url, limit: usize) -> Result<(Url, Vec<u8>), ()> {
    for _ in 0..=3 {
        url = checked_url(url.as_str())?;
        let host = url.host_str().ok_or(())?.to_owned();
        let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 443))
            .await
            .map_err(|_| ())?
            .collect();
        if addresses.is_empty() {
            return Err(());
        }
        // Ignore IPv6 (unsupported), but reject any non-public IPv4 result.
        let ipv4: Vec<_> = addresses.into_iter().filter(|a| a.is_ipv4()).collect();
        if ipv4.is_empty() || ipv4.iter().any(|a| !public_ip(a.ip())) {
            return Err(());
        }
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .resolve_to_addrs(&host, &ipv4)
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(3))
            .user_agent("CentaurContext-SourcePreview/1")
            .build()
            .map_err(|_| ())?;
        let response = client.get(url.clone()).send().await.map_err(|_| ())?;
        if response.status().is_redirection() {
            let target = response
                .headers()
                .get(header::LOCATION)
                .ok_or(())?
                .to_str()
                .map_err(|_| ())?;
            url = checked_url(url.join(target).map_err(|_| ())?.as_str())?;
            continue;
        }
        if !response.status().is_success() {
            return Err(());
        }
        return Ok((url, limited_body(response, limit).await?));
    }
    Err(())
}
async fn limited_body(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>, ()> {
    if response.content_length().is_some_and(|n| n > limit as u64) {
        return Err(());
    }
    let mut data = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| ())? {
        if data.len().saturating_add(chunk.len()) > limit {
            return Err(());
        }
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}

fn youtube_image(url: &Url) -> Option<Url> {
    let host = url.host_str()?;
    let segments: Vec<_> = url.path_segments()?.collect();
    let id = if host == "youtu.be" {
        segments.first()?.to_string()
    } else if [
        "youtube.com",
        "www.youtube.com",
        "m.youtube.com",
        "www.youtube-nocookie.com",
    ]
    .contains(&host)
    {
        if url.path() == "/watch" {
            url.query_pairs().find(|(k, _)| k == "v")?.1.into_owned()
        } else if ["shorts", "embed", "live"].contains(segments.first()?) {
            segments.get(1)?.to_string()
        } else {
            return None;
        }
    } else {
        return None;
    };
    if id.len() != 11
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return None;
    }
    Url::parse(&format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg")).ok()
}
fn page_image(base: &Url, bytes: &[u8]) -> Option<Url> {
    let document = Html::parse_document(std::str::from_utf8(bytes).ok()?);
    for selector in [
        "meta[property='og:image:secure_url']",
        "meta[property='og:image']",
        "meta[name='twitter:image']",
        "meta[property='twitter:image']",
    ] {
        for node in document.select(&Selector::parse(selector).ok()?) {
            let Some(raw) = node.value().attr("content") else {
                continue;
            };
            if let Ok(url) = base.join(raw.trim())
                && let Ok(url) = checked_url(url.as_str())
            {
                return Some(url);
            }
        }
    }
    None
}
fn raster(bytes: Vec<u8>) -> Option<Image> {
    let mime = if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else {
        return None;
    };
    let dimensions = imagesize::blob_size(&bytes).ok()?;
    if dimensions.width == 0
        || dimensions.height == 0
        || dimensions.width > 8192
        || dimensions.height > 8192
        || dimensions.width.saturating_mul(dimensions.height) > 16_000_000
    {
        return None;
    }
    Some(Image { bytes, mime })
}
async fn resolve(url: Url) -> Option<Image> {
    let image_url = if let Some(image) = youtube_image(&url) {
        image
    } else {
        let (base, page) = fetch(url, PAGE_LIMIT).await.ok()?;
        page_image(&base, &page)?
    };
    let (_, bytes) = fetch(image_url, IMAGE_LIMIT).await.ok()?;
    raster(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_private_reserved_and_non_https_destinations() {
        for raw in [
            "http://example.com",
            "https://user:secret@example.com",
            "https://example.com:8443",
            "https://127.0.0.1",
            "https://2130706433",
            "https://[::1]",
            "https://[::ffff:127.0.0.1]",
            "https://169.254.169.254",
            "https://100.64.0.1",
            "https://192.0.2.1",
            "https://198.18.0.1",
            "https://203.0.113.1",
            "https://x.local",
        ] {
            assert!(checked_url(raw).is_err(), "{raw}");
        }
        assert!(public_ip(IpAddr::V4(std::net::Ipv4Addr::new(8, 8, 8, 8))));
        assert!(checked_url("https://example.com/article").is_ok());
        // 192.0.66.0/24 is public publisher hosting, not the reserved 192.0.0.0/24.
        assert!(public_ip(IpAddr::V4(std::net::Ipv4Addr::new(
            192, 0, 66, 2
        ))));
    }
    #[test]
    fn provider_ids_and_html_are_data_not_code() {
        for raw in [
            "https://youtu.be/dQw4w9WgXcQ",
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtube.com/shorts/dQw4w9WgXcQ",
        ] {
            assert_eq!(
                youtube_image(&Url::parse(raw).unwrap()).unwrap().host_str(),
                Some("i.ytimg.com")
            );
        }
        assert!(
            youtube_image(
                &Url::parse("https://youtube.com.evil.test/watch?v=dQw4w9WgXcQ").unwrap()
            )
            .is_none()
        );
        let base = Url::parse("https://example.com/story/").unwrap();
        assert_eq!(
            page_image(
                &base,
                b"<meta property='og:image' content='/banner.png?a=1&amp;b=2'>"
            )
            .unwrap()
            .as_str(),
            "https://example.com/banner.png?a=1&b=2"
        );
        assert!(
            page_image(
                &base,
                b"<meta property='og:image' content='https://127.0.0.1/private'>"
            )
            .is_none()
        );
        assert!(raster(b"<svg onload='alert(1)'></svg>".to_vec()).is_none());
    }
    #[test]
    fn cache_ttls_and_force_refresh_are_bounded() {
        let mut e = Entry {
            image: None,
            fetched: Instant::now() - Duration::from_secs(61),
        };
        assert!(fresh(&e, false));
        assert!(!fresh(&e, true));
        e.fetched = Instant::now() - FAILURE_TTL;
        assert!(!fresh(&e, false));
    }

    #[tokio::test]
    async fn disabled_endpoint_never_accesses_database_or_network() {
        use tower::ServiceExt;
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1/centaur_context_test_unused")
            .unwrap();
        let response = router(pool, false)
            .oneshot(
                axum::http::Request::builder()
                    .uri("/api/v2/sources/63000000-0000-4000-8000-000000000001/thumbnail")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    #[test]
    fn metadata_and_redirect_candidates_cannot_escape_network_policy() {
        let base = Url::parse("https://example.com/article").unwrap();
        for candidate in [
            "//127.0.0.1/a",
            "https://10.0.0.1/a",
            "file:///etc/passwd",
            "data:image/svg+xml,bad",
            "https://example.com:22/a",
        ] {
            let target = base.join(candidate).unwrap();
            assert!(checked_url(target.as_str()).is_err());
            let page = format!("<meta property='og:image' content='{candidate}'>");
            assert!(page_image(&base, page.as_bytes()).is_none());
        }
        assert!(page_image(&base, b"<meta property='og:image'>").is_none());
        assert!(page_image(&base, &[0xff, 0xfe]).is_none());
    }
    #[tokio::test]
    async fn limits_bodies_even_without_a_content_length() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for body in [
            "Content-Length: 8\r\n\r\n12345678",
            "Transfer-Encoding: chunked\r\n\r\n8\r\n12345678\r\n0\r\n\r\n",
        ] {
            // This local fixture tests only the body reader. Production fetch()
            // never permits loopback or HTTP, including under test configuration.
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let serve = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    assert!(request.len() < 4096);
                    request.push(socket.read_u8().await.unwrap());
                }
                socket
                    .write_all(format!("HTTP/1.1 200 OK\r\nConnection: close\r\n{body}").as_bytes())
                    .await
                    .unwrap();
            });
            let response = Client::builder()
                .no_proxy()
                .build()
                .unwrap()
                .get(format!("http://{address}"))
                .send()
                .await
                .unwrap();
            assert!(limited_body(response, 4).await.is_err());
            serve.await.unwrap();
        }
    }

    #[test]
    fn rejects_unbounded_dimensions_and_active_image_formats() {
        assert!(
            raster(vec![
                137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 10, 0, 0, 0,
                10, 8, 2, 0, 0, 0, 0, 0, 0, 0
            ])
            .is_some()
        );
        assert!(
            raster(vec![
                137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 35, 40, 0, 0,
                35, 40, 8, 2, 0, 0, 0, 0, 0, 0, 0
            ])
            .is_none()
        );
        assert!(raster(b"GIF89a".to_vec()).is_none());
        assert!(raster(b"<html>not an image</html>".to_vec()).is_none());
    }
}
