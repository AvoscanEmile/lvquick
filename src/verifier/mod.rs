use crate::core::Draft;
mod provision;
use provision::verify_provision;

pub fn verify(draft: Draft) -> Result<Draft, String> {
    match draft.draft_type.as_str() {
        "provision" => verify_provision(draft),
        _ => Err(format!(
            "Architectural Error: No verification ruleset exists for draft type '{}'", 
            draft.draft_type
        )),
    }
}

