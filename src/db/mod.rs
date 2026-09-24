//! PostgreSQL persistence organized by canonical Context responsibility.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Transaction};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::domain::{ActorContext, ValidationError, theme_slug, validate_object_description};

#[derive(Debug, Error)]
pub enum DbError {
    #[error("record not found")]
    NotFound,
    #[error("revision conflict")]
    Conflict,
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Validation(#[from] ValidationError),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

mod artifacts;
mod chats;
mod connections;
mod embeddings;
mod events;
mod models;
mod notes;
mod objects;
mod retrieval;
mod routines;
mod sources;
mod tasks;

pub use artifacts::*;
pub use chats::*;
pub use connections::*;
pub use embeddings::*;
pub use models::*;
pub use notes::*;
pub use objects::*;
pub use retrieval::*;
pub use routines::*;
pub use sources::*;
pub use tasks::*;

use embeddings::vector_literal;
pub(crate) use events::target_snapshot;
use events::{idempotent_entity, insert_event};
pub(crate) use events::{insert_event_for_run, insert_event_for_run_with_before};
use models::{
    ArtifactSearchCandidateRow, ContextAnchorCandidateRow, ObjectVisualSource, SearchCandidateRow,
};
use notes::{insert_note_connection, validate_note_links};
use objects::{push_object_list_cursor, push_object_list_order};
