use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    config::TextSearchConfig,
    db::{self, ContextConnection, DbError, Object, SearchCandidate},
    embeddings::{EmbeddingClient, OBJECT_EMBEDDING_FORMAT},
};

const RRF_K: f64 = 60.0;
pub const CONTEXT_MAX_CHARACTERS: usize = 12_000;

#[derive(Clone, Debug, Serialize)]
pub struct RetrievedObject {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub description: String,
    pub revision: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtype: Option<Value>,
    pub relevance: Relevance,
    pub connections: Vec<ContextConnection>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Relevance {
    pub score: f64,
    pub rationale: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContextBudget {
    pub max_characters: usize,
    pub serialized_characters: usize,
    pub omitted_objects: usize,
    pub omitted_connections: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchPacket {
    pub query: String,
    pub retrieval: String,
    pub objects: Vec<RetrievedObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget: Option<ContextBudget>,
}

#[derive(Clone)]
struct Fused {
    object: Object,
    score: f64,
    connection_count: i64,
    matched_fts: bool,
    matched_semantic: bool,
    graph_reason: Option<String>,
    anchor_reason: Option<String>,
}

pub async fn search(
    pool: &PgPool,
    embeddings: Option<&EmbeddingClient>,
    text_search_config: TextSearchConfig,
    query: &str,
    kind: Option<&str>,
    limit: i64,
) -> Result<SearchPacket, DbError> {
    let limit = limit.clamp(1, 100);
    let (retrieval, mut fused) = retrieve(
        pool,
        embeddings,
        text_search_config,
        query,
        kind,
        limit,
        false,
    )
    .await?;
    fused.truncate(limit as usize);
    Ok(SearchPacket {
        query: query.to_owned(),
        retrieval,
        objects: fused
            .into_iter()
            .map(|item| retrieved(item, None, Vec::new(), false))
            .collect(),
        budget: None,
    })
}

pub async fn context(
    pool: &PgPool,
    embeddings: Option<&EmbeddingClient>,
    text_search_config: TextSearchConfig,
    query: &str,
    kind: Option<&str>,
    chat_object_id: Uuid,
    limit: i64,
) -> Result<SearchPacket, DbError> {
    let limit = limit.clamp(1, 10);
    let (retrieval, mut fused) = retrieve(
        pool,
        embeddings,
        text_search_config,
        query,
        kind,
        limit,
        true,
    )
    .await?;

    for anchored in db::context_anchor_candidates(pool, chat_object_id).await? {
        let score = 10.0 - f64::from(anchored.priority);
        if let Some(existing) = fused
            .iter_mut()
            .find(|candidate| candidate.object.id == anchored.object.id)
        {
            existing.score = existing.score.max(score);
            existing.anchor_reason = Some(anchored.rationale);
        } else {
            fused.push(Fused {
                object: anchored.object,
                score,
                connection_count: 0,
                matched_fts: false,
                matched_semantic: false,
                graph_reason: None,
                anchor_reason: Some(anchored.rationale),
            });
        }
    }
    fused.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.object.id.cmp(&right.object.id))
    });

    let ids = fused.iter().map(|item| item.object.id).collect::<Vec<_>>();
    let mut connections = db::context_connections(pool, &ids).await?;
    let mut subtypes = db::context_subtypes(pool, &ids, Some(chat_object_id)).await?;
    let candidates = fused
        .into_iter()
        .map(|item| {
            let id = item.object.id;
            retrieved(
                item,
                subtypes.remove(&id),
                connections.remove(&id).unwrap_or_default(),
                true,
            )
        })
        .collect::<Vec<_>>();
    Ok(build_budgeted_packet(
        query,
        &retrieval,
        candidates,
        limit as usize,
        CONTEXT_MAX_CHARACTERS,
    ))
}

pub async fn read_object(pool: &PgPool, id: Uuid) -> Result<RetrievedObject, DbError> {
    let object = db::get_object(pool, id).await?;
    let mut connections = db::context_connections(pool, &[id]).await?;
    let mut subtypes = db::context_subtypes(pool, &[id], None).await?;
    Ok(RetrievedObject {
        id: object.id,
        kind: object.kind,
        title: object.title,
        description: object.description,
        revision: object.revision,
        subtype: subtypes.remove(&id),
        relevance: Relevance {
            score: 1.0,
            rationale: "Read directly by canonical Object ID.".to_owned(),
        },
        connections: connections.remove(&id).unwrap_or_default(),
    })
}

async fn retrieve(
    pool: &PgPool,
    embeddings: Option<&EmbeddingClient>,
    text_search_config: TextSearchConfig,
    query: &str,
    kind: Option<&str>,
    limit: i64,
    context_builder: bool,
) -> Result<(String, Vec<Fused>), DbError> {
    let candidate_limit = (limit * 4).clamp(20, 100);
    let fts = db::full_text_candidates(
        pool,
        text_search_config,
        query,
        kind,
        candidate_limit,
        context_builder,
    )
    .await?;
    let semantic = if let Some(client) = embeddings {
        match client.embed_query(query).await {
            Ok(vector) => db::semantic_candidates(
                pool,
                &vector,
                client.model(),
                client.dimensions(),
                OBJECT_EMBEDDING_FORMAT,
                client.document_mode(),
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
                anchor_reason: None,
            });
        }
        fused.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.object.id.cmp(&right.object.id))
        });
    }
    Ok((retrieval.to_owned(), fused))
}

fn retrieved(
    item: Fused,
    subtype: Option<Value>,
    connections: Vec<ContextConnection>,
    context_builder: bool,
) -> RetrievedObject {
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
        subtype,
        relevance,
        connections,
    }
}

fn build_budgeted_packet(
    query: &str,
    retrieval: &str,
    candidates: Vec<RetrievedObject>,
    limit: usize,
    max_characters: usize,
) -> SearchPacket {
    let total_objects = candidates.len();
    let total_connections = candidates
        .iter()
        .map(|object| object.connections.len())
        .sum::<usize>();
    let mut objects = Vec::new();
    for mut candidate in candidates {
        if objects.len() >= limit {
            break;
        }
        loop {
            let mut proposed = objects.clone();
            proposed.push(candidate.clone());
            let packet = packet_with_budget(
                query,
                retrieval,
                proposed,
                max_characters,
                total_objects,
                total_connections,
            );
            if serialized_characters(&packet) <= max_characters {
                objects.push(candidate);
                break;
            }
            if candidate.connections.pop().is_none() {
                return packet_with_budget(
                    query,
                    retrieval,
                    objects,
                    max_characters,
                    total_objects,
                    total_connections,
                );
            }
        }
    }
    packet_with_budget(
        query,
        retrieval,
        objects,
        max_characters,
        total_objects,
        total_connections,
    )
}

fn packet_with_budget(
    query: &str,
    retrieval: &str,
    objects: Vec<RetrievedObject>,
    max_characters: usize,
    total_objects: usize,
    total_connections: usize,
) -> SearchPacket {
    let included_connections = objects
        .iter()
        .map(|object| object.connections.len())
        .sum::<usize>();
    let mut packet = SearchPacket {
        query: query.to_owned(),
        retrieval: retrieval.to_owned(),
        budget: Some(ContextBudget {
            max_characters,
            serialized_characters: 0,
            omitted_objects: total_objects.saturating_sub(objects.len()),
            omitted_connections: total_connections.saturating_sub(included_connections),
        }),
        objects,
    };
    for _ in 0..3 {
        let used = serialized_characters(&packet);
        if packet
            .budget
            .as_ref()
            .is_some_and(|budget| budget.serialized_characters == used)
        {
            break;
        }
        if let Some(budget) = &mut packet.budget {
            budget.serialized_characters = used;
        }
    }
    packet
}

fn serialized_characters(packet: &SearchPacket) -> usize {
    serde_json::to_string(packet)
        .expect("context packet is serializable")
        .chars()
        .count()
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
                anchor_reason: None,
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
                anchor_reason: None,
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
    if let Some(reason) = &item.anchor_reason {
        return reason.clone();
    }
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
    use super::{
        ContextConnection, RRF_K, Relevance, RetrievedObject, build_budgeted_packet,
        connection_boost, serialized_characters,
    };
    use uuid::Uuid;

    fn candidate(description: &str, connections: usize) -> RetrievedObject {
        RetrievedObject {
            id: Uuid::new_v4(),
            kind: "memory".to_owned(),
            title: "A complete event".to_owned(),
            description: description.to_owned(),
            revision: 1,
            subtype: Some(
                serde_json::json!({"kind":"memory","happened_at":"2026-08-29T00:00:00Z"}),
            ),
            relevance: Relevance {
                score: 1.0,
                rationale: "Test candidate.".to_owned(),
            },
            connections: (0..connections)
                .map(|_| ContextConnection {
                    id: Uuid::new_v4(),
                    direction: "outgoing".to_owned(),
                    kind: "about".to_owned(),
                    description: "A complete supporting relationship.".repeat(8),
                    other_object_id: Uuid::new_v4(),
                    other_object_kind: "entity".to_owned(),
                    other_object_title: "Related entity".to_owned(),
                })
                .collect(),
        }
    }

    #[test]
    fn connection_boost_is_small_but_monotonic() {
        assert_eq!(connection_boost(0), 0.0);
        assert!(connection_boost(10) > connection_boost(1));
        assert!(connection_boost(10) < 1.0 / RRF_K);
    }

    #[test]
    fn context_budget_keeps_complete_unicode_objects_and_reports_omissions() {
        let complete_description = "東京 launch approved. ".repeat(20);
        let packet = build_budgeted_packet(
            "launch",
            "full_text",
            vec![
                candidate(&complete_description, 5),
                candidate(&"Another complete event. ".repeat(20), 5),
                candidate("Third complete event.", 0),
            ],
            10,
            1_600,
        );
        let budget = packet.budget.as_ref().unwrap();
        assert!(serialized_characters(&packet) <= budget.max_characters);
        assert!(budget.omitted_objects > 0 || budget.omitted_connections > 0);
        assert_eq!(packet.objects[0].description, complete_description);
    }

    #[test]
    fn context_budget_never_exceeds_ten_objects() {
        let packet = build_budgeted_packet(
            "all",
            "full_text",
            (0..20).map(|_| candidate("Complete.", 0)).collect(),
            10,
            100_000,
        );
        assert_eq!(packet.objects.len(), 10);
        assert_eq!(packet.budget.unwrap().omitted_objects, 10);
    }
}
