//! Canonical, versioned Context contract embedded in the service binary.

use std::sync::LazyLock;

use serde_json::Value;
use sha2::{Digest, Sha256};

const CONTRACT_JSON: &str = include_str!("../contract/context-contract.json");

static CONTRACT: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(CONTRACT_JSON).expect("embedded Context contract must be valid JSON")
});

static CONTRACT_HASH: LazyLock<String> =
    LazyLock::new(|| format!("{:x}", Sha256::digest(CONTRACT_JSON.as_bytes())));

pub fn document() -> &'static Value {
    &CONTRACT
}

pub fn hash() -> &'static str {
    &CONTRACT_HASH
}

pub fn version() -> &'static str {
    CONTRACT["contract_version"]
        .as_str()
        .expect("contract_version must be a string")
}

pub fn tool_version() -> &'static str {
    CONTRACT["tool_version"]
        .as_str()
        .expect("tool_version must be a string")
}

pub fn object_description_max_characters() -> usize {
    CONTRACT["description_policy"]["max_characters"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .expect("description_policy.max_characters must be a positive integer")
}

pub fn interactive_writable(kind: &str) -> bool {
    CONTRACT["object_types"][kind]["interactive_write"]
        .as_bool()
        .unwrap_or(false)
}

pub fn object_fields(kind: &str) -> Option<Vec<&'static str>> {
    CONTRACT["object_types"][kind]["fields"]
        .as_array()
        .map(|values| values.iter().filter_map(Value::as_str).collect())
}

pub fn standalone_note_intent(intent: &str) -> bool {
    CONTRACT["rules"]["standalone_note_intents"]
        .as_array()
        .is_some_and(|values| values.iter().any(|value| value.as_str() == Some(intent)))
}

pub fn connection_kind(kind: &str) -> bool {
    CONTRACT["connection_kinds"]
        .as_array()
        .is_some_and(|values| values.iter().any(|value| value.as_str() == Some(kind)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_contract_has_the_three_tools_and_expected_write_boundary() {
        assert_eq!(version(), "1.1.0");
        assert_eq!(tool_version(), "1.1.0");
        assert_eq!(object_description_max_characters(), 600);
        assert!(interactive_writable("task"));
        assert!(interactive_writable("source"));
        assert!(!interactive_writable("memory"));
        assert!(connection_kind("related_to"));
        assert_eq!(
            document()["tools"].as_object().expect("tools object").len(),
            3
        );
    }
}
