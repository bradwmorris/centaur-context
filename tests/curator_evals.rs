use centaur_os::curator::{ReconciliationPlan, validate_plan};
use serde::Deserialize;

#[derive(Deserialize)]
struct EvalSuite {
    cases: Vec<EvalCase>,
}

#[derive(Deserialize)]
struct EvalCase {
    name: String,
    expected_valid: bool,
    plan: ReconciliationPlan,
}

#[test]
fn context_curator_mvp_policy_evals() {
    let suite: EvalSuite =
        serde_json::from_str(include_str!("../evals/context_curator_mvp.json")).unwrap();
    for mut case in suite.cases {
        let original_descriptions = case
            .plan
            .create_objects
            .iter()
            .map(|object| object.description.clone())
            .collect::<Vec<_>>();
        let valid = validate_plan(&mut case.plan).is_ok();
        assert_eq!(valid, case.expected_valid, "eval case: {}", case.name);
        if valid {
            assert_eq!(
                case.plan
                    .create_objects
                    .iter()
                    .map(|object| object.description.clone())
                    .collect::<Vec<_>>(),
                original_descriptions,
                "clear descriptions should not be rewritten: {}",
                case.name
            );
        }
    }
}
