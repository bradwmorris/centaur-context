use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

#[derive(Clone, Debug, Serialize)]
pub struct ActorContext {
    pub actor_type: &'static str,
    pub actor_id: String,
    pub centaur_thread_key: Option<String>,
    pub centaur_execution_id: Option<String>,
    pub is_agent: bool,
}

impl ActorContext {
    pub fn human() -> Self {
        Self {
            actor_type: "human",
            actor_id: "brad-local".to_owned(),
            centaur_thread_key: None,
            centaur_execution_id: None,
            is_agent: false,
        }
    }
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("{0} is required")]
    Required(&'static str),
    #[error("{field} must be at most {max} characters")]
    TooLong { field: &'static str, max: usize },
    #[error("unsupported {field}: {value}")]
    Unsupported { field: &'static str, value: String },
    #[error("provenance must be a JSON object")]
    ProvenanceObject,
    #[error("source and target objects must be different")]
    SelfConnection,
}

pub fn required_text(
    value: String,
    field: &'static str,
    max: usize,
) -> Result<String, ValidationError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(ValidationError::Required(field));
    }
    if value.chars().count() > max {
        return Err(ValidationError::TooLong { field, max });
    }
    Ok(value)
}

pub fn optional_text(
    value: Option<String>,
    field: &'static str,
    max: usize,
) -> Result<Option<String>, ValidationError> {
    value
        .map(|value| required_text(value, field, max))
        .transpose()
}

pub fn allowed(
    value: String,
    field: &'static str,
    values: &[&str],
) -> Result<String, ValidationError> {
    let value = value.trim().to_lowercase();
    if values.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(ValidationError::Unsupported { field, value })
    }
}

pub fn provenance(value: Option<Value>) -> Result<Value, ValidationError> {
    let value = value.unwrap_or_else(|| Value::Object(Map::new()));
    let object = value.as_object().ok_or(ValidationError::ProvenanceObject)?;
    for key in object.keys() {
        if !["source_type", "source_ref", "note"].contains(&key.as_str()) {
            return Err(ValidationError::Unsupported {
                field: "provenance key",
                value: key.clone(),
            });
        }
    }
    if let Some(source_type) = object.get("source_type") {
        let source_type = source_type
            .as_str()
            .ok_or(ValidationError::Required("provenance.source_type"))?;
        if source_type.trim().is_empty() {
            return Err(ValidationError::Required("provenance.source_type"));
        }
    }
    Ok(value)
}

pub const OBJECT_KINDS: &[&str] = &["task", "chat", "user", "entity", "memory"];
pub const CONNECTION_KINDS: &[&str] = &[
    "involves",
    "about",
    "related_to",
    "depends_on",
    "derived_from",
];
pub const TASK_STATUSES: &[&str] = &["todo", "doing", "blocked", "review", "done"];
pub const TASK_PRIORITIES: &[&str] = &["low", "medium", "high"];
pub const USER_KINDS: &[&str] = &["human", "agent"];

#[derive(Clone, Debug, Deserialize)]
pub struct ProvenanceInput {
    pub source_type: Option<String>,
    pub source_ref: Option<String>,
    pub note: Option<String>,
}

impl From<ProvenanceInput> for Value {
    fn from(value: ProvenanceInput) -> Self {
        serde_json::to_value(value).unwrap_or_else(|_| Value::Object(Map::new()))
    }
}

impl Serialize for ProvenanceInput {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = Map::new();
        if let Some(value) = &self.source_type {
            map.insert("source_type".to_owned(), Value::String(value.clone()));
        }
        if let Some(value) = &self.source_ref {
            map.insert("source_ref".to_owned(), Value::String(value.clone()));
        }
        if let Some(value) = &self.note {
            map.insert("note".to_owned(), Value::String(value.clone()));
        }
        map.serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn validates_and_normalizes_enums() {
        assert_eq!(
            allowed(" MEMORY ".to_owned(), "kind", OBJECT_KINDS).unwrap(),
            "memory"
        );
        assert!(allowed("report".to_owned(), "kind", OBJECT_KINDS).is_err());
    }

    #[test]
    fn provenance_rejects_unknown_keys() {
        assert!(provenance(Some(json!({"source_type": "human"}))).is_ok());
        assert!(provenance(Some(json!({"secret": "no"}))).is_err());
        assert!(provenance(Some(json!([]))).is_err());
    }

    #[test]
    fn required_text_trims_and_limits() {
        assert_eq!(
            required_text("  hello  ".to_owned(), "title", 10).unwrap(),
            "hello"
        );
        assert!(required_text("   ".to_owned(), "title", 10).is_err());
        assert!(required_text("too long".to_owned(), "title", 3).is_err());
    }
}
