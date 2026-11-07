//! `workers[]`: every worker the run spawned, from `state.json` and the mtime of its stream.
use serde_json::{Value, json};

use super::{Ctx, mtime};
use crate::worker;

pub fn workers(ctx: &Ctx) -> Value {
    ctx.state
        .workers
        .iter()
        .map(|w| {
            json!({
                "sessionId": w.session_id,
                "role": w.role,
                "slice": w.slice,
                "checkpoint": w.checkpoint,
                "lane": w.lane,
                "spawnedAt": w.spawned_at,
                "alive": ctx.state.worker_alive(w),
                "lastWriteAt": mtime(&worker::stream_path(&ctx.run_dir, &w.session_id)),
                "job": null,
                "doing": null,
                "narration": null,
                "running": null,
                "outcome": null,
            })
        })
        .collect()
}
