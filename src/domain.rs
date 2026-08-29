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
            actor_id: "local-human".to_owned(),
            centaur_thread_key: None,
            centaur_execution_id: None,
            is_agent: false,
        }
    }

    pub fn system(actor_id: impl Into<String>) -> Self {
        Self {
            actor_type: "system",
            actor_id: actor_id.into(),
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
    #[error("description must {0}")]
    WeakDescription(&'static str),
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

pub fn object_description(title: &str, value: String) -> Result<String, ValidationError> {
    let value = required_text(value, "description", 1000)?;
    validate_object_description(title, &value)?;
    Ok(value)
}

pub fn validate_object_description(title: &str, description: &str) -> Result<(), ValidationError> {
    let comparable_title = comparable_text(title);
    let comparable_description = comparable_text(description);
    if !comparable_title.is_empty() && comparable_description == comparable_title {
        return Err(ValidationError::WeakDescription(
            "add concrete context instead of repeating the title",
        ));
    }
    if matches!(
        comparable_description.as_str(),
        "tbd"
            | "todo"
            | "placeholder"
            | "description"
            | "no description"
            | "none"
            | "n a"
            | "na"
            | "unknown"
            | "lorem ipsum"
            | "this is about the project"
            | "this is about this project"
    ) {
        return Err(ValidationError::WeakDescription(
            "replace placeholder or vague text with a specific statement",
        ));
    }
    let lower = description.trim().to_lowercase();
    if ["user:", "human:", "assistant:", "agent:"]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
    {
        return Err(ValidationError::WeakDescription(
            "summarize the represented Object instead of copying a transcript fragment",
        ));
    }
    let starts_with_process_commentary = [
        "as an ai",
        "i was asked to",
        "i have generated",
        "here is a description",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix));
    let contains_generation_commentary = ["the model generated", "generated description"]
        .iter()
        .any(|marker| lower.contains(marker));
    if starts_with_process_commentary || contains_generation_commentary {
        return Err(ValidationError::WeakDescription(
            "describe the Object directly without process or model commentary",
        ));
    }
    Ok(())
}

fn comparable_text(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() {
                character.to_lowercase().collect::<String>()
            } else {
                " ".to_owned()
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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

pub const OBJECT_KINDS: &[&str] = &["task", "chat", "user", "entity", "memory", "source", "note"];
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
pub const SOURCE_KINDS: &[&str] = &[
    "article", "paper", "podcast", "video", "book", "report", "document", "dataset", "web_page",
    "other",
];
pub const SOURCE_CONTENT_KINDS: &[&str] = &[
    "article_text",
    "transcript",
    "paper_text",
    "document_text",
    "dataset_description",
    "other",
];
pub const NOTE_CONTENT_FORMATS: &[&str] = &["plain_text", "markdown"];

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

    #[test]
    fn object_descriptions_reject_mechanical_weaknesses() {
        for description in [
            "Quarterly plan",
            "TBD",
            "This is about the project.",
            "User: please update the launch plan",
            "As an AI, I have generated this description.",
        ] {
            assert!(
                object_description("Quarterly plan", description.to_owned()).is_err(),
                "weak description was accepted: {description}"
            );
        }
        assert!(object_description("Long", "x".repeat(1001)).is_err());
    }

    #[test]
    fn object_descriptions_accept_concrete_examples_for_every_kind() {
        for (title, description) in [
            (
                "Publish launch notes",
                "Prepare and publish the approved launch notes for customers.",
            ),
            (
                "Launch review",
                "A Slack conversation where the release team approved the launch checklist.",
            ),
            (
                "Taylor Morgan",
                "A human product lead responsible for the customer migration program.",
            ),
            (
                "Northwind",
                "A customer organization participating in the August migration pilot.",
            ),
            (
                "Migration approved",
                "The product team approved the customer migration during the August review.",
            ),
        ] {
            assert_eq!(
                object_description(title, format!("  {description}  ")).unwrap(),
                description
            );
        }
    }
}
