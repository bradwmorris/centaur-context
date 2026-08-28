use std::collections::HashMap;

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    db::{self, ContextConnection, DbError, Object, SearchCandidate},
    embeddings::EmbeddingClient,
};

const RRF_K: f64 = 60.0;

#[derive(Clone, Debug, Serialize)]
pub struct RetrievedObject {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub description: String,
    pub revision: i64,
    pub relevance: Relevance,
    pub connections: Vec<ContextConnection>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Relevance {
    pub score: f64,
    pub rationale: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchPacket {
    pub query: String,
    pub retrieval: String,
    pub objects: Vec<RetrievedObject>,
}

#[derive(Clone)]
struct Fused {
    object: Object,
    score: f64,
    connection_count: i64,
    matched_fts: bool,
    matched_semantic: bool,
    graph_reason: Option<String>,
}

pub async fn search(
    pool: &PgPool,
    embeddings: Option<&EmbeddingClient>,
    query: &str,
    kind: Option<&str>,
    limit: i64,
    context_builder: bool,
) -> Result<SearchPacket, DbError> {
    let limit = if context_builder {
        limit.clamp(1, 10)
    } else {
        limit.clamp(1, 100)
    };
    let candidate_limit = (limit * 4).clamp(20, 100);
    let fts = db::full_text_candidates(pool, query, kind, candidate_limit, context_builder).await?;
    let semantic = if let Some(client) = embeddings {
        match client.embed(query).await {
            Ok(vector) => db::semantic_candidates(
                pool,
                &vector,
                client.model(),
                client.dimensions(),
                kind,
                candidate_limit,
                context_builder,
            )
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "semantic Object search failed; using full text");
                Vec::new()
            }),
            Err(error) => {
                tracing::warn!(%error, "query embedding failed; using full text");
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    let retrieval = if semantic.is_empty() {
        "full_text"
    } else {
        "hybrid"
    };
    let mut fused = fuse(fts, semantic, context_builder);

    if context_builder && !fused.is_empty() {
        let seed_scores: HashMap<Uuid, f64> = fused
            .iter()
            .take(6)
            .map(|item| (item.object.id, item.score))
            .collect();
        let seed_ids = fused
            .iter()
            .take(6)
            .map(|item| item.object.id)
            .collect::<Vec<_>>();
        let neighbors = db::one_hop_neighbors(pool, &seed_ids, kind, candidate_limit).await?;
        for neighbor in neighbors {
            if fused.iter().any(|item| item.object.id == neighbor.id) {
                continue;
            }
            let Some(seed_score) = seed_scores.get(&neighbor.seed_object_id) else {
                continue;
            };
            fused.push(Fused {
                object: neighbor.object(),
                score: seed_score * 0.45 + connection_boost(neighbor.connection_count),
                connection_count: neighbor.connection_count,
                matched_fts: false,
                matched_semantic: false,
                graph_reason: Some(format!(
                    "Connected by {}: {}",
                    neighbor.connection_kind, neighbor.connection_description
                )),
            });
        }
        fused.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.object.id.cmp(&right.object.id))
        });
    }
    fused.truncate(limit as usize);

    let ids = fused.iter().map(|item| item.object.id).collect::<Vec<_>>();
    let mut connections = if context_builder {
        db::context_connections(pool, &ids).await?
    } else {
        HashMap::new()
    };
    let objects = fused
        .into_iter()
        .map(|item| {
            let relevance = Relevance {
                score: item.score,
                rationale: rationale(&item, context_builder),
            };
            RetrievedObject {
                id: item.object.id,
                kind: item.object.kind,
                title: item.object.title,
                description: item.object.description,
                revision: item.object.revision,
                relevance,
                connections: connections.remove(&item.object.id).unwrap_or_default(),
            }
        })
        .collect();
    Ok(SearchPacket {
        query: query.to_owned(),
        retrieval: retrieval.to_owned(),
        objects,
    })
}

pub async fn read_object(pool: &PgPool, id: Uuid) -> Result<RetrievedObject, DbError> {
    let object = db::get_object(pool, id).await?;
    let mut connections = db::context_connections(pool, &[id]).await?;
    Ok(RetrievedObject {
        id: object.id,
        kind: object.kind,
        title: object.title,
        description: object.description,
        revision: object.revision,
        relevance: Relevance {
            score: 1.0,
            rationale: "Read directly by canonical Object ID.".to_owned(),
        },
        connections: connections.remove(&id).unwrap_or_default(),
    })
}

fn fuse(
    full_text: Vec<SearchCandidate>,
    semantic: Vec<SearchCandidate>,
    boost_connections: bool,
) -> Vec<Fused> {
    let mut fused: HashMap<Uuid, Fused> = HashMap::new();
    for (rank, candidate) in full_text.into_iter().enumerate() {
        let score = 1.0 / (RRF_K + rank as f64 + 1.0);
        fused.insert(
            candidate.object.id,
            Fused {
                object: candidate.object,
                score,
                connection_count: candidate.connection_count,
                matched_fts: true,
                matched_semantic: false,
                graph_reason: None,
            },
        );
    }
    for (rank, candidate) in semantic.into_iter().enumerate() {
        let score = 1.0 / (RRF_K + rank as f64 + 1.0);
        fused
            .entry(candidate.object.id)
            .and_modify(|item| {
                item.score += score;
                item.matched_semantic = true;
            })
            .or_insert(Fused {
                object: candidate.object,
                score,
                connection_count: candidate.connection_count,
                matched_fts: false,
                matched_semantic: true,
                graph_reason: None,
            });
    }
    let mut fused = fused.into_values().collect::<Vec<_>>();
    if boost_connections {
        for item in &mut fused {
            item.score += connection_boost(item.connection_count);
        }
    }
    fused.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.object.id.cmp(&right.object.id))
    });
    fused
}

fn connection_boost(connection_count: i64) -> f64 {
    (connection_count.max(0) as f64 + 1.0).ln() * 0.0025
}

fn rationale(item: &Fused, context_builder: bool) -> String {
    if let Some(reason) = &item.graph_reason {
        return reason.clone();
    }
    let match_reason = match (item.matched_fts, item.matched_semantic) {
        (true, true) => "Matched both its language and meaning.",
        (false, true) => "Matched by meaning.",
        _ => "Matched words in its title or description.",
    };
    if context_builder && item.connection_count > 0 {
        format!(
            "{match_reason} Context ranking also considered its {} active connection(s).",
            item.connection_count
        )
    } else {
        match_reason.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{RRF_K, connection_boost};

    #[test]
    fn connection_boost_is_small_but_monotonic() {
        assert_eq!(connection_boost(0), 0.0);
        assert!(connection_boost(10) > connection_boost(1));
        assert!(connection_boost(10) < 1.0 / RRF_K);
    }
}
