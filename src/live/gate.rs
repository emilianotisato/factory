//! `gate`, the running or last gate, and `ci`, the pull request's checks.
use serde_json::Value;

use super::Ctx;

pub fn gate(_ctx: &Ctx) -> Value {
    Value::Null
}

pub fn ci(_ctx: &Ctx) -> Value {
    Value::Null
}
