mod support;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use centaur_context::{
    api::{AppState, agent_router},
    config::TextSearchConfig,
    db,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

async fn setup() -> Option<(PgPool, Router)> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    assert!(
        url.rsplit('/')
            .next()
            .unwrap()
            .starts_with("centaur_context_test")
    );
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .unwrap();
    db::migrate(&pool).await.unwrap();
    let app = agent_router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        "test-correction-token".repeat(2),
    );
    Some((pool, app))
}
async fn call(app: &Router, path: &str, body: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(path)
        .header(
            "authorization",
            format!("Bearer {}", "test-correction-token".repeat(2)),
        )
        .header("x-centaur-principal-id", "synthetic-correction-agent")
        .header("x-centaur-thread-key", "synthetic:correction:thread")
        .header("content-type", "application/json")
        .header(
            "idempotency-key",
            body["idempotency_key"].as_str().unwrap_or("read"),
        )
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    (status, value)
}
fn batch(ops: Value) -> Value {
    json!({"contract_version":"1.1.0","idempotency_key":Uuid::new_v4(),"operations":ops})
}
async fn apply(app: &Router, ops: Value) -> Value {
    let (status, value) = call(app, "/api/v2/apply", batch(ops)).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    value["data"].clone()
}
async fn seed(pool: &PgPool, kind: &str, protected: bool) -> Uuid {
    let id = Uuid::new_v4();
    let owner = support::task_owner(pool).await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO objects(id,kind,title,description,protected,created_by_type,created_by_id,updated_by_type,updated_by_id) VALUES($1,$2,$3,'Original imported description.', $4,'system','synthetic-import','system','synthetic-import')")
        .bind(id).bind(kind).bind(format!("Synthetic {kind}")).bind(protected).execute(&mut *tx).await.unwrap();
    let sql = match kind {
        "task" => "INSERT INTO tasks(object_id,owner_object_id,due_at) VALUES($1,$2,'2099-01-01')",
        "entity" => "INSERT INTO entities(object_id,entity_kind) VALUES($1,'person')",
        "source" => "INSERT INTO sources(object_id,source_kind) VALUES($1,'article')",
        "note" => {
            "INSERT INTO notes(object_id,content,intent) VALUES($1,'Original note text.','idea')"
        }
        "theme" => "INSERT INTO themes(object_id,slug) VALUES($1,$1::text)",
        "user" => "INSERT INTO users(object_id,user_kind) VALUES($1,'human')",
        "chat" => "INSERT INTO chats(object_id) VALUES($1)",
        "memory" => "INSERT INTO memories(object_id,happened_at) VALUES($1,now())",
        _ => panic!(),
    };
    let q = sqlx::query(sql).bind(id);
    if kind == "task" {
        q.bind(owner).execute(&mut *tx).await.unwrap();
    } else {
        q.execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();
    id
}

#[tokio::test]
async fn every_kind_corrects_restores_and_preserves_real_imported_preimage() {
    let Some((pool, app)) = setup().await else {
        return;
    };
    for kind in [
        "task", "entity", "source", "note", "theme", "user", "chat", "memory",
    ] {
        for protected in [false, true] {
            let id = seed(&pool, kind, protected).await;
            let mut changes = json!({"title":format!("Corrected {kind}"),"description":"Corrected current representation."});
            match kind {
                "task" => changes["priority"] = json!("high"),
                "entity" => changes["entity_kind"] = json!("organization"),
                "source" => changes["byline"] = json!("Corrected author metadata"),
                "note" => changes["content"] = json!("Corrected note text."),
                "theme" => changes["slug"] = json!(format!("corrected-{id}")),
                "user" => changes["user_kind"] = json!("agent"),
                "chat" => changes["channel_name"] = json!("Corrected channel"),
                "memory" => changes["happened_at"] = json!("2026-01-01T00:00:00Z"),
                _ => (),
            }
            let request = batch(
                json!([{"operation":"update_object","object_id":id,"expected_revision":1,"changes":changes}]),
            );
            let mut dry = request.clone();
            dry["validate_only"] = json!(true);
            assert_eq!(call(&app, "/api/v2/apply", dry).await.0, StatusCode::OK);
            assert_eq!(db::get_object(&pool, id).await.unwrap().revision, 1);
            let (status, result) = call(&app, "/api/v2/apply", request.clone()).await;
            assert_eq!(status, StatusCode::OK, "{kind}: {result}");
            assert_eq!(
                call(&app, "/api/v2/apply", request.clone()).await.1["data"]["replayed"],
                true
            );
            let mut stale = request.clone();
            stale["idempotency_key"] = json!(Uuid::new_v4());
            assert_eq!(
                call(&app, "/api/v2/apply", stale).await.0,
                StatusCode::CONFLICT
            );
            let events = db::list_events(&pool, id).await.unwrap();
            assert_eq!(
                events[0].before_state.as_ref().unwrap()["description"],
                "Original imported description."
            );
            assert_eq!(events[0].actor_id, "synthetic-correction-agent");
            assert_eq!(
                db::get_object(&pool, id).await.unwrap().protected,
                protected
            );
            apply(
                &app,
                json!([{"operation":"archive_object","object_id":id,"expected_revision":2}]),
            )
            .await;
            apply(
                &app,
                json!([{"operation":"restore_object","object_id":id,"expected_revision":3}]),
            )
            .await;
            assert_eq!(db::get_object(&pool, id).await.unwrap().lifecycle, "active");
            let bad = batch(json!([
                {"operation":"update_object","object_id":id,"expected_revision":4,"changes":{"description":"Would be rolled back."}},
                {"operation":"update_object","object_id":id,"expected_revision":5,"changes":{"actor_id":"forged-principal"}}
            ]));
            assert_eq!(
                call(&app, "/api/v2/apply", bad).await.0,
                StatusCode::UNPROCESSABLE_ENTITY
            );
            assert_eq!(
                db::get_object(&pool, id).await.unwrap().description,
                "Corrected current representation."
            );
        }
    }
}

#[tokio::test]
async fn canonical_repair_keeps_exact_citations_and_rejects_fabricated_derivation() {
    let Some((pool, app)) = setup().await else {
        return;
    };
    let source = seed(&pool, "source", true).await;
    let other = seed(&pool, "source", true).await;
    let note = seed(&pool, "note", true).await;
    let append = |rev, text: &str, prev: Option<Uuid>| json!([{"operation":"append_artifact","object":{"object_id":source},"expected_revision":rev,"kind":"transcript","content":text,"supersedes_artifact_id":prev}]);
    let first = apply(&app, append(1, "Original exact quote.", None)).await;
    let old: Uuid = serde_json::from_value(first["results"][0]["data"]["id"].clone()).unwrap();
    apply(&app,json!([{"operation":"promote_source_artifact","source_id":source,"expected_revision":2,"artifact_id":old,"expected_sha256":format!("{:x}",Sha256::digest(b"Original exact quote."))}])).await;
    apply(&app,json!([{"operation":"update_object","object_id":note,"expected_revision":1,"changes":{"intent":"excerpt","content":"Original exact quote.","source_artifact_id":old,"source_locator":{"paragraph":1}}}])).await;
    let second = apply(&app, append(3, "Repaired exact transcription.", Some(old))).await;
    let new = second["results"][0]["data"]["id"].clone();
    let promoted=apply(&app,json!([{"operation":"promote_source_artifact","source_id":source,"expected_revision":4,"artifact_id":new,"expected_sha256":format!("{:x}",Sha256::digest(b"Repaired exact transcription."))}])).await;
    assert_eq!(
        promoted["results"][0]["retained_citation_note_ids"],
        json!([note])
    );
    assert_eq!(
        db::get_note(&pool, note).await.unwrap().source_artifact_id,
        Some(old)
    );
    let original: String = sqlx::query_scalar("SELECT content FROM artifacts WHERE id=$1")
        .bind(old)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(original, "Original exact quote.");
    let bad = batch(
        json!([{"operation":"update_object","object_id":note,"expected_revision":2,"changes":{"source_artifact_id":new}}]),
    );
    assert_eq!(
        call(&app, "/api/v2/apply", bad).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let wrong = batch(
        json!([{"operation":"create_connection","source":{"object_id":note},"target":{"object_id":other},"kind":"derived_from","description":"Incorrect claim of quotation derivation."}]),
    );
    assert_eq!(
        call(&app, "/api/v2/apply", wrong).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    apply(&app,json!([{"operation":"create_connection","source":{"object_id":note},"target":{"object_id":other},"kind":"related_to","description":"Contextual secondary source, not quotation evidence."}])).await;
    apply(&app,json!([{"operation":"update_object","object_id":note,"expected_revision":2,"changes":{"source_artifact_id":new,"content":"Repaired exact transcription."}}])).await;
    let found =
        db::artifact_full_text_candidates(&pool, "transcription", Some("source"), 100, false)
            .await
            .unwrap();
    assert!(found.iter().any(|candidate| candidate.object.id == source));
    let old_found =
        db::artifact_full_text_candidates(&pool, "Original", Some("source"), 100, false)
            .await
            .unwrap();
    assert!(
        !old_found
            .iter()
            .any(|candidate| candidate.object.id == source)
    );
}

#[tokio::test]
async fn message_event_run_and_artifact_corrections_are_attributed_and_recoverable() {
    let Some((pool, app)) = setup().await else {
        return;
    };
    let chat = seed(&pool, "chat", true).await;
    let user = seed(&pool, "user", true).await;
    let message = Uuid::new_v4();
    sqlx::query("INSERT INTO chat_messages(id,chat_object_id,provider_message_id,sender_user_object_id,content,source_created_at) VALUES($1,$2,'synthetic-message',$3,'Original captured text.',now())").bind(message).bind(chat).bind(user).execute(&pool).await.unwrap();
    let response=apply(&app,json!([{"operation":"correct_evidence","object_id":chat,"expected_revision":1,"target_type":"message","target_id":message,"reason":"Repair a transcription error; retain original evidence.","representation":{"content":"Corrected current transcription."}}])).await;
    let messages = db::list_chat_messages(&pool, chat).await.unwrap();
    assert_eq!(messages[0].content, "Original captured text.");
    assert_eq!(messages[0].sender_user_object_id, user);
    assert_eq!(
        messages[0].correction.as_ref().unwrap()["representation"]["content"],
        "Corrected current transcription."
    );
    for (rev, kind, target) in [
        (2, "event", response["event_ids"][0].clone()),
        (3, "run", response["run_id"].clone()),
    ] {
        apply(&app,json!([{"operation":"correct_evidence","object_id":chat,"expected_revision":rev,"target_type":kind,"target_id":target,"reason":"Annotate the synthetic historical assertion.","representation":{"note":"This historical assertion needs qualification."}}])).await;
    }
    let attachment=apply(&app,json!([{"operation":"append_artifact","object":{"object_id":chat},"expected_revision":4,"kind":"supporting_text","content":"Original evidence."}])).await;
    apply(&app,json!([{"operation":"correct_evidence","object_id":chat,"expected_revision":5,"target_type":"artifact","target_id":attachment["results"][0]["data"]["id"],"reason":"Incorrect historical label.","representation":{"title":"Corrected evidence label"}}])).await;
    apply(&app,json!([{"operation":"correct_evidence","object_id":chat,"expected_revision":6,"target_type":"message","target_id":message,"reason":"Withdraw the earlier proposed repair.","representation":{}}])).await;
    let (_, read) = call(
        &app,
        "/api/v2/read",
        json!({"object_ids":[chat],"include":["messages","events","artifacts"]}),
    )
    .await;
    assert!(
        read.to_string()
            .contains("Corrected current transcription.")
    );
    let all = db::list_corrections(&pool, chat).await.unwrap();
    assert_eq!(all.len(), 5);
    assert!(!all[0]["supersedes_correction_id"].is_null());
    let wrong = batch(
        json!([{"operation":"correct_evidence","object_id":user,"expected_revision":1,"target_type":"message","target_id":message,"reason":"Must not attach to wrong owner.","representation":{}}]),
    );
    assert_eq!(
        call(&app, "/api/v2/apply", wrong).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert!(
        sqlx::query("UPDATE evidence_corrections SET reason='erased' WHERE object_id=$1")
            .bind(chat)
            .execute(&pool)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn protected_connections_edit_archive_restore_and_enforce_endpoints() {
    let Some((pool, app)) = setup().await else {
        return;
    };
    let source = seed(&pool, "source", true).await;
    let entity = seed(&pool, "entity", true).await;
    let theme = seed(&pool, "theme", true).await;
    let note = seed(&pool, "note", true).await;
    let other = seed(&pool, "source", true).await;
    for (from, kind, to) in [
        (source, "themed", theme),
        (entity, "themed", theme),
        (note, "about", entity),
        (source, "related_to", other),
    ] {
        let edge=apply(&app,json!([{"operation":"create_connection","source":{"object_id":from},"target":{"object_id":to},"kind":kind,"description":"Synthetic relationship with protected endpoints."}])).await;
        let id: Uuid =
            serde_json::from_value(edge["results"][0]["data"]["connection"]["id"].clone()).unwrap();
        sqlx::query("UPDATE connections SET protected=true WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        apply(&app,json!([{"operation":"update_connection","connection_id":id,"expected_revision":1,"description":"Corrected current explanation."}])).await;
        apply(
            &app,
            json!([{"operation":"archive_connection","connection_id":id,"expected_revision":2}]),
        )
        .await;
        apply(
            &app,
            json!([{"operation":"restore_connection","connection_id":id,"expected_revision":3}]),
        )
        .await;
        let bad =
            batch(json!([{"operation":"archive_object","object_id":from,"expected_revision":1}]));
        assert_eq!(
            call(&app, "/api/v2/apply", bad).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
}

#[tokio::test]
async fn wrong_kind_replacement_relinks_both_directions_and_retains_old_identity() {
    let Some((pool, app)) = setup().await else {
        return;
    };
    let wrong = seed(&pool, "entity", true).await;
    let incoming = seed(&pool, "note", true).await;
    let outgoing = seed(&pool, "theme", true).await;
    let edges=apply(&app,json!([
        {"operation":"create_connection","source":{"object_id":incoming},"kind":"related_to","target":{"object_id":wrong},"description":"Incoming relationship to misclassified item."},
        {"operation":"create_connection","source":{"object_id":wrong},"kind":"themed","target":{"object_id":outgoing},"description":"Outgoing thematic relationship."}
    ])).await;
    let original_edges = vec![
        edges["results"][0]["data"]["connection"]["id"].clone(),
        edges["results"][1]["data"]["connection"]["id"].clone(),
    ];
    let replacement=apply(&app,json!([
        {"operation":"create_object","local_ref":"replacement","kind":"source","title":"Correctly classified source","description":"Replacement of a misclassified synthetic publication.","fields":{"source_kind":"paper"},"provenance":{"source_ref":wrong,"note":"A publication was imported as a person."}},
        {"operation":"archive_connection","connection_id":original_edges[0],"expected_revision":1},
        {"operation":"archive_connection","connection_id":original_edges[1],"expected_revision":1},
        {"operation":"create_connection","source":{"object_id":incoming},"kind":"related_to","target":{"local_ref":"replacement"},"description":"Incoming relationship now points to the corrected publication."},
        {"operation":"create_connection","source":{"local_ref":"replacement"},"kind":"themed","target":{"object_id":outgoing},"description":"The corrected publication retains its thematic relation."},
        {"operation":"correct_evidence","object_id":wrong,"expected_revision":1,"target_type":"object","target_id":wrong,"reason":"Replaced by a Source because the original type was incorrect.","representation":{"type_correction":"source"}},
        {"operation":"archive_object","object_id":wrong,"expected_revision":2}
    ])).await;
    let replacement_id: Uuid =
        serde_json::from_value(replacement["results"][0]["data"]["id"].clone()).unwrap();
    assert_eq!(
        db::list_connections(&pool, replacement_id)
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(db::get_object(&pool, wrong).await.unwrap().kind, "entity");
    assert_eq!(
        db::get_object(&pool, wrong).await.unwrap().lifecycle,
        "archived"
    );
    assert_eq!(db::list_events(&pool, wrong).await.unwrap().len(), 2);
    let old: Vec<Uuid> = original_edges
        .into_iter()
        .map(|id| serde_json::from_value(id).unwrap())
        .collect();
    let retained: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM connections WHERE id=ANY($1) AND archived_at IS NOT NULL",
    )
    .bind(old)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(retained, 2);
}

#[tokio::test]
async fn identity_reassignment_repairs_future_mapping_without_forging_authorship() {
    let Some((pool, app)) = setup().await else {
        return;
    };
    let old = seed(&pool, "user", true).await;
    let new = seed(&pool, "user", true).await;
    let chat = seed(&pool, "chat", true).await;
    let msg = Uuid::new_v4();
    let identity = Uuid::new_v4();
    sqlx::query("UPDATE users SET identities=$2 WHERE object_id=$1").bind(old).bind(json!([{"id":identity,"provider":"synthetic","workspace_id":"fixture","provider_user_id":identity.to_string(),"display_name":"Original profile"}])).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO chat_messages(id,chat_object_id,provider_message_id,sender_user_object_id,content,source_created_at) VALUES($1,$2,'original',$3,'Original attributed evidence.',now())").bind(msg).bind(chat).bind(old).execute(&pool).await.unwrap();
    let task = seed(&pool, "task", true).await;
    apply(&app,json!([{"operation":"update_object","object_id":task,"expected_revision":1,"changes":{"owner_object_id":old}}])).await;
    let request = batch(json!([
        {"operation":"reassign_identity","object_id":old,"expected_revision":1,"target_object_id":new,"expected_target_revision":1,"identity_id":identity,"reason":"The imported provider identity belongs to the replacement profile."},
        {"operation":"update_object","object_id":task,"expected_revision":2,"changes":{"owner_object_id":new}},
        {"operation":"archive_object","object_id":old,"expected_revision":2}
    ]));
    let (status, response) = call(&app, "/api/v2/apply", request.clone()).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        call(&app, "/api/v2/apply", request).await.1["data"]["replayed"],
        true
    );
    assert_eq!(
        db::get_user(&pool, new).await.unwrap().identities[0]["id"],
        json!(identity)
    );
    assert_eq!(
        db::get_user(&pool, old).await.unwrap().identities,
        json!([])
    );
    assert_eq!(
        db::list_chat_messages(&pool, chat).await.unwrap()[0].sender_user_object_id,
        old
    );
    assert_eq!(
        db::list_events(&pool, new).await.unwrap()[0].actor_id,
        "synthetic-correction-agent"
    );
    assert_eq!(
        db::get_task(&pool, task).await.unwrap().owner_object_id,
        Some(new)
    );
}

#[tokio::test]
async fn producer_replay_preserves_explicit_user_chat_and_message_corrections() {
    use centaur_context::ingest::{self, ApprovedSlackSurfaces};
    let Some((pool, app)) = setup().await else {
        return;
    };
    let suffix = Uuid::new_v4().simple().to_string();
    let workspace = format!("T{suffix}");
    let channel = format!("C{suffix}");
    let capture = ingest::router(
        AppState {
            pool: pool.clone(),
            embeddings: None,
            text_search_config: TextSearchConfig::SIMPLE,
        },
        "test-correction-token".repeat(2),
        ApprovedSlackSurfaces::parse(&format!("{workspace}:{channel}")).unwrap(),
    );
    let payload = json!({"workspace_id":workspace,"channel_id":channel,"channel_name":"Imported channel","thread_id":"1780000000.001","surface_kind":"channel","interaction_finished":false,"run":{"interaction_id":"1780000001.001","status":"running","started_at":"2026-08-28T00:00:01Z"},"messages":[{"provider_message_id":"1780000001.001","sender":{"provider_user_id":format!("U{suffix}"),"display_name":"Imported user","user_kind":"human"},"content":"Original captured phrase.","source_created_at":"2026-08-28T00:00:01Z"}]});
    let (status, response) = call(
        &capture,
        "/api/v2/ingest/slack/interactions",
        payload.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{response}");
    let chat: Uuid = serde_json::from_value(response["data"]["chat_object_id"].clone()).unwrap();
    let message = db::list_chat_messages(&pool, chat).await.unwrap().remove(0);
    let user = message.sender_user_object_id;
    let chatrev = db::get_object(&pool, chat).await.unwrap().revision;
    let userrev = db::get_object(&pool, user).await.unwrap().revision;
    apply(&app,json!([
        {"operation":"update_object","object_id":user,"expected_revision":userrev,"changes":{"title":"Corrected profile name","description":"Explicitly corrected domain profile."}},
        {"operation":"update_object","object_id":chat,"expected_revision":chatrev,"changes":{"title":"Corrected conversation title","channel_name":"Corrected channel label"}},
        {"operation":"correct_evidence","object_id":chat,"expected_revision":chatrev+1,"target_type":"message","target_id":message.id,"reason":"Original capture had a transcription error.","representation":{"content":"uniquecorrectionsearchword"}}
    ])).await;
    let mut later = payload.clone();
    later["messages"][0]["sender"]["display_name"] = json!("Stale upstream profile");
    later["channel_name"] = json!("Stale upstream channel");
    assert_eq!(
        call(&capture, "/api/v2/ingest/slack/interactions", later)
            .await
            .0,
        StatusCode::ACCEPTED
    );
    assert_eq!(
        db::get_object(&pool, user).await.unwrap().title,
        "Corrected profile name"
    );
    let label: String = sqlx::query_scalar("SELECT channel_name FROM chats WHERE object_id=$1")
        .bind(chat)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(label, "Corrected channel label");
    let current = db::list_chat_messages(&pool, chat).await.unwrap();
    assert_eq!(current[0].content, "Original captured phrase.");
    assert_eq!(
        current[0].correction.as_ref().unwrap()["representation"]["content"],
        "uniquecorrectionsearchword"
    );
    let (_, found) = call(
        &app,
        "/api/v2/search",
        json!({"query":"uniquecorrectionsearchword","lexical_only":true}),
    )
    .await;
    assert!(found.to_string().contains(&chat.to_string()), "{found}");
    assert!(
        found
            .to_string()
            .contains("Original capture had a transcription error.")
    );
    let packet = centaur_context::search::context(
        &pool,
        None,
        TextSearchConfig::SIMPLE,
        "uniquecorrectionsearchword",
        None,
        chat,
        10,
    )
    .await
    .unwrap();
    let corrected = packet.objects.iter().find(|item| item.id == chat).unwrap();
    assert_eq!(
        corrected.corrections[0]["representation"]["content"],
        "uniquecorrectionsearchword"
    );
    db::queue_missing_embeddings(
        &pool,
        "synthetic-correction-model",
        3,
        "centaur-object-v1",
        "shared",
    )
    .await
    .unwrap();
    let hash:String=sqlx::query_scalar("SELECT source_hash FROM embeddings WHERE object_id=$1 AND model='synthetic-correction-model' AND artifact_id IS NULL").bind(chat).fetch_one(&pool).await.unwrap();
    let rev = db::get_object(&pool, chat).await.unwrap().revision;
    apply(&app,json!([{"operation":"correct_evidence","object_id":chat,"expected_revision":rev,"target_type":"message","target_id":message.id,"reason":"Second reviewed correction.","representation":{"content":"newcorrectionsearchword"}}])).await;
    let (newhash,status):(String,String)=sqlx::query_as("SELECT source_hash,status FROM embeddings WHERE object_id=$1 AND model='synthetic-correction-model' AND artifact_id IS NULL").bind(chat).fetch_one(&pool).await.unwrap();
    assert_ne!(hash, newhash);
    assert_eq!(status, "pending");
    let (_, oldfound) = call(
        &app,
        "/api/v2/search",
        json!({"query":"uniquecorrectionsearchword","lexical_only":true}),
    )
    .await;
    assert!(
        !oldfound.to_string().contains(&chat.to_string()),
        "superseded correction remained current search truth: {oldfound}"
    );
    apply(
        &app,
        json!([{"operation":"rebuild_derived","object_id":chat,"expected_revision":rev+1}]),
    )
    .await;
}

#[tokio::test]
async fn system_types_create_and_chat_replacement_preserves_original_capture() {
    let Some((pool, app)) = setup().await else {
        return;
    };
    let old = seed(&pool, "chat", true).await;
    let sender = seed(&pool, "user", false).await;
    let msg = Uuid::new_v4();
    let thread = Uuid::new_v4().to_string();
    sqlx::query("UPDATE chats SET provider='slack',workspace_id='TSYNTHETIC',channel_id='CSYNTHETIC',thread_id=$2,surface_kind='channel' WHERE object_id=$1").bind(old).bind(&thread).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO chat_messages(id,chat_object_id,provider_message_id,sender_user_object_id,content,source_created_at) VALUES($1,$2,'old-capture',$3,'Retained in original Chat.',now())").bind(msg).bind(old).bind(sender).execute(&pool).await.unwrap();
    let mut created = Vec::new();
    for kind in ["chat", "user", "memory"] {
        let result=apply(&app,json!([
            {"operation":"create_object","local_ref":"new","kind":kind,"title":format!("New {kind}"),"description":"Explicitly created replacement domain record.","fields":{}},
            {"operation":"create_connection","source":{"local_ref":"new"},"target":{"object_id":old},"kind":"related_to","description":"Replacement context for the original captured conversation."}
        ])).await;
        created.push(
            serde_json::from_value::<Uuid>(result["results"][0]["data"]["id"].clone()).unwrap(),
        );
    }
    let replacement = created[0];
    let request = batch(
        json!([{"operation":"reassign_chat","object_id":old,"expected_revision":1,"target_object_id":replacement,"expected_target_revision":1,"reason":"Correct which current conversation receives future captures."}]),
    );
    let mut invalid = request.clone();
    invalid["operations"][0]["expected_target_revision"] = json!(99);
    assert_eq!(
        call(&app, "/api/v2/apply", invalid).await.0,
        StatusCode::CONFLICT
    );
    let (status, value) = call(&app, "/api/v2/apply", request.clone()).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(
        call(&app, "/api/v2/apply", request).await.1["data"]["replayed"],
        true
    );
    let current:Uuid=sqlx::query_scalar("SELECT object_id FROM chats WHERE provider='slack' AND workspace_id='TSYNTHETIC' AND channel_id='CSYNTHETIC' AND thread_id=$1").bind(thread).fetch_one(&pool).await.unwrap();
    assert_eq!(current, replacement);
    assert_eq!(db::list_chat_messages(&pool, old).await.unwrap()[0].id, msg);
    assert!(
        db::list_chat_messages(&pool, replacement)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        db::list_events(&pool, old).await.unwrap()[0]
            .before_state
            .as_ref()
            .unwrap()["subtype"]["provider"],
        "slack"
    );
}

#[tokio::test]
async fn restore_respects_active_task_ownership_and_corrections_cannot_cross_store_ids() {
    let Some((pool, app)) = setup().await else {
        return;
    };
    let task = seed(&pool, "task", true).await;
    let owner = db::get_task(&pool, task)
        .await
        .unwrap()
        .owner_object_id
        .unwrap();
    apply(&app,json!([{"operation":"archive_object","object_id":task,"expected_revision":1},{"operation":"archive_object","object_id":owner,"expected_revision":1}])).await;
    let blocked =
        batch(json!([{"operation":"restore_object","object_id":task,"expected_revision":2}]));
    assert_eq!(
        call(&app, "/api/v2/apply", blocked).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    apply(&app,json!([{"operation":"restore_object","object_id":owner,"expected_revision":2},{"operation":"restore_object","object_id":task,"expected_revision":2}])).await;
    let missing = batch(
        json!([{"operation":"correct_evidence","object_id":task,"expected_revision":3,"target_type":"artifact","target_id":Uuid::new_v4(),"reason":"An ID absent from the authorized store cannot be corrected.","representation":{}}]),
    );
    assert_eq!(
        call(&app, "/api/v2/apply", missing).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}
