//! Narrow trusted-host receipts: verify Git object bytes, never completion prose.
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{db::DbError, domain::ActorContext};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GitReceipt {
    pub turn_id: Uuid,
    pub baseline: String,
    pub commits: Vec<GitCommit>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GitCommit {
    pub oid: String,
    pub raw: String,
}

fn oid(value: &str, size: usize) -> bool {
    value.len() == size
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Object hashes prove the claimed commit contents/parent chain. The reviewed
/// host adapter additionally proves observation in the allowlisted worktree.
/// Neither establishes authorship, tests, deployment, or task completion.
pub fn verify(receipt: &GitReceipt) -> Result<String, DbError> {
    let size = receipt.baseline.len();
    if !matches!(size, 40 | 64)
        || !oid(&receipt.baseline, size)
        || receipt.commits.is_empty()
        || receipt.commits.len() > 10
    {
        return Err(DbError::Invalid("Invalid bounded Git receipt".into()));
    }
    let mut parent = receipt.baseline.as_str();
    let mut subject = String::new();
    for commit in &receipt.commits {
        if !oid(&commit.oid, size) || commit.raw.len() > 16_384 {
            return Err(DbError::Invalid(
                "Invalid Git commit size or identity".into(),
            ));
        }
        let bytes = [
            format!("commit {}\0", commit.raw.len()).as_bytes(),
            commit.raw.as_bytes(),
        ]
        .concat();
        let actual = if size == 40 {
            format!("{:x}", sha1::Sha1::digest(&bytes))
        } else {
            format!("{:x}", Sha256::digest(&bytes))
        };
        let (headers, message) = commit
            .raw
            .split_once("\n\n")
            .ok_or_else(|| DbError::Invalid("Invalid Git commit format".into()))?;
        let first_parent = headers.lines().find_map(|v| v.strip_prefix("parent "));
        if actual != commit.oid
            || first_parent != Some(parent)
            || !headers
                .lines()
                .next()
                .is_some_and(|v| v.strip_prefix("tree ").is_some_and(|v| oid(v, size)))
        {
            return Err(DbError::Invalid(
                "Git proof hash or parent chain mismatch".into(),
            ));
        }
        parent = &commit.oid;
        subject = message
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .filter(|c| !c.is_control())
            .take(240)
            .collect();
        // Never preserve credential-looking commit subjects in durable knowledge.
        if [
            "sk-",
            "ghp_",
            "github_pat_",
            "xoxb-",
            "PRIVATE KEY",
            "Bearer ",
        ]
        .iter()
        .any(|v| subject.contains(v))
        {
            subject = "[subject omitted: possible credential]".into();
        }
    }
    Ok(subject)
}

pub async fn capture(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    host: Uuid,
    repository: &str,
    chat: Uuid,
    receipt: &GitReceipt,
) -> Result<Option<Uuid>, DbError> {
    verify(receipt)?;
    let last = &receipt.commits.last().expect("verified nonempty").oid;
    let key = format!("codex-git:{host}:{repository}:{last}");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(&key)
        .execute(&mut **tx)
        .await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM runs WHERE kind='memory_capture' AND idempotency_key=$1)",
    )
    .bind(&key)
    .fetch_one(&mut **tx)
    .await?;
    if exists {
        return Ok(None);
    }
    let run = Uuid::new_v4();
    let evidence = json!({"source_type":"codex_git_commit","repository":repository,"host_id":host,"turn_id":receipt.turn_id,"baseline":receipt.baseline,"commits":receipt.commits.iter().map(|c| &c.oid).collect::<Vec<_>>(),"observation":"allowlisted worktree HEAD advanced during the turn; no authorship or test claim"});
    // Retain identifiers and proof digest, not raw authors/emails/signatures/messages.
    let digest = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(receipt).expect("serializable"))
    );
    sqlx::query("INSERT INTO runs(id,kind,status,actor_type,actor_id,idempotency_key,chat_object_id,primary_object_id,input,result,completed_at) VALUES($1,'memory_capture','completed','system',$2,$3,$4,NULL,$5,'{}',now())")
        .bind(run).bind(&actor.actor_id).bind(key).bind(chat).bind(json!({"evidence":evidence,"proof_sha256":digest})).execute(&mut **tx).await?;
    // Git movement is technical evidence, never a standalone semantic outcome.
    // Keep the existing run kind/key so retries also recognise legacy receipts.
    sqlx::query("UPDATE runs SET result=$2 WHERE id=$1")
        .bind(run)
        .bind(json!({"evidence_only":true,"reason":"Git observation does not establish a contextual Task milestone"}))
        .execute(&mut **tx)
        .await?;
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn git_proof_rejects_forged_hash_parent_and_completion_flags() {
        let baseline = "a".repeat(40);
        let raw = format!(
            "tree {}\nparent {baseline}\nauthor Example <example@example.invalid> 1 +0000\ncommitter Example <example@example.invalid> 1 +0000\n\nAdd bounded capture\n",
            "b".repeat(40)
        );
        let bytes = [format!("commit {}\0", raw.len()).as_bytes(), raw.as_bytes()].concat();
        let mut proof = GitReceipt {
            turn_id: Uuid::new_v4(),
            baseline,
            commits: vec![GitCommit {
                oid: format!("{:x}", sha1::Sha1::digest(bytes)),
                raw,
            }],
        };
        assert_eq!(verify(&proof).unwrap(), "Add bounded capture");
        proof.commits[0].raw.push_str("Tests passed");
        assert!(verify(&proof).is_err());
        let original_len = proof.commits[0].raw.len() - 12;
        proof.commits[0].raw.truncate(original_len);
        proof.baseline = "c".repeat(40);
        assert!(verify(&proof).is_err());
        assert!(serde_json::from_value::<GitReceipt>(json!({"turn_id":Uuid::new_v4(),"baseline":"a".repeat(40),"commits":[],"trusted":true})).is_err());
    }
}
