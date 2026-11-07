//! `evidence`: the files checkpoint workers left under the run's evidence dir.
use serde_json::{Value, json};

use super::Ctx;

/// `checkpoints` runs newest first, and each one's `files` newest first.
pub fn evidence(_ctx: &Ctx, _limit: usize) -> Value {
    json!({
        "root": null,
        "checkpoints": [],
        "skipped": 0,
        "truncated": false,
    })
}
