//! What leaves the run before it prints.
use serde_json::Value;

/// Runs over the whole snapshot, before `rev` and the cap.
pub fn all(_v: &mut Value) {}
