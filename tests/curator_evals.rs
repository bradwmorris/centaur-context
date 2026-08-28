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
        let valid = validate_plan(&mut case.plan).is_ok();
        assert_eq!(valid, case.expected_valid, "eval case: {}", case.name);
    }
}
