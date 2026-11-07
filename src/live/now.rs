//! `now`, what the run waits on, and `attention`, what the owner is asked.
use serde_json::{Value, json};

use super::Ctx;

pub fn now(_ctx: &Ctx) -> Value {
    Value::Null
}

pub fn attention(_ctx: &Ctx) -> Value {
    json!([])
}
