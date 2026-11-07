//! One run's live detail, read from its files without opening `factory.db` or writing anything.
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::{Value, json};

use crate::docs::{Docs, log};
use crate::events::{self, Event, Skipped};
use crate::run::{self, RunState};
use crate::{Result, clock, metrics};

pub mod evidence;
pub mod gate;
pub mod now;
pub mod redact;
pub mod timeline;
pub mod workers;

/// The `schema` of `live --json`; bumped when a field changes meaning or goes away.
pub const SCHEMA: u32 = 1;

/// The printed object never exceeds this many bytes.
pub const CAP: usize = 512 * 1024;

/// The envelope's keys, in the order they print.
pub const KEYS: [&str; 19] = [
    "schema",
    "factory",
    "id",
    "feature",
    "phase",
    "repo",
    "branch",
    "seq",
    "rev",
    "at",
    "now",
    "attention",
    "workers",
    "gate",
    "ci",
    "timeline",
    "evidence",
    "decisions",
    "truncated",
];

/// Everything one read of a run sees, built once and shared by every section.
pub struct Ctx {
    pub run_dir: PathBuf,
    pub state: RunState,
    pub events: Vec<Event>,
    pub skipped: Vec<Skipped>,
    /// The feature's docs folder in the run's repo, live before archived.
    pub feature_dir: Option<PathBuf>,
    /// Epoch seconds.
    pub now: u64,
}

impl Ctx {
    pub fn read(run_dir: &Path) -> Result<Ctx> {
        let state = run::load_state(run_dir)?;
        let (events, skipped) = events::scan(run_dir)?;
        let docs = Docs::open(&state.repo)
            .and_then(|d| d.status(&state.feature))
            .ok();
        let feature_dir =
            crate::cli::status::feature_folder(&state.repo, &state.feature, docs.as_ref())
                .map(|(_, dir)| dir);
        Ok(Ctx {
            run_dir: run_dir.to_path_buf(),
            state,
            events,
            skipped,
            feature_dir,
            now: clock::unix_now(),
        })
    }
}

pub struct Opts {
    /// Only events after this `seq` reach the timeline; none keeps the last [`timeline::DEFAULT`].
    pub since: Option<u64>,
    pub evidence_limit: usize,
}

/// Redacted, then hashed into `rev`, then cut to [`CAP`].
pub fn snapshot(ctx: &Ctx, opts: &Opts) -> Value {
    let s = &ctx.state;
    let mut v = json!({
        "schema": SCHEMA,
        "factory": env!("CARGO_PKG_VERSION"),
        "id": s.id.to_string(),
        "feature": s.feature,
        "phase": s.phase.as_str(),
        "repo": s.repo.display().to_string(),
        "branch": s.branch,
        "seq": ctx.events.last().map_or(0, |e| e.seq),
        "rev": "",
        "at": clock::rfc3339(ctx.now),
        "now": now::now(ctx),
        "attention": now::attention(ctx),
        "workers": workers::workers(ctx),
        "gate": gate::gate(ctx),
        "ci": gate::ci(ctx),
        "timeline": timeline::timeline(ctx, opts.since),
        "evidence": evidence::evidence(ctx, opts.evidence_limit),
        "decisions": { "count": decisions(ctx) },
        "truncated": [],
    });
    redact::all(&mut v);
    v["rev"] = Value::String(rev(&v));
    cap(&mut v);
    v
}

/// One compact line, the envelope's keys in [`KEYS`] order.
pub fn render(v: &Value) -> String {
    let mut out = String::from("{");
    for k in KEYS {
        let Some(x) = v.get(k) else { continue };
        if out.len() > 1 {
            out.push(',');
        }
        let _ = write!(out, "{}:{}", Value::from(k), x);
    }
    out.push('}');
    out
}

/// Blind to the fields that move while nothing visible happens.
pub fn rev(v: &Value) -> String {
    let mut v = v.clone();
    if let Some(o) = v.as_object_mut() {
        o.remove("rev");
        o.remove("at");
        o.remove("truncated");
    }
    if let Some(o) = v["now"].as_object_mut() {
        o.remove("idleSeconds");
    }
    if let Some(ws) = v["workers"].as_array_mut() {
        for w in ws {
            if let Some(o) = w.as_object_mut() {
                o.remove("lastWriteAt");
            }
            if let Some(o) = w["running"].as_object_mut() {
                o.remove("elapsedSeconds");
            }
        }
    }
    metrics::sha256_hex(render(&v).as_bytes())[..16].to_string()
}

/// Drops evidence files from the oldest, then timeline events from the oldest, until the line fits.
fn cap(v: &mut Value) {
    let mut cut: Vec<&str> = Vec::new();
    let mut len = render(v).len();
    while len > CAP {
        let over = len - CAP;
        let section = if cut_evidence(v, over) {
            "evidence"
        } else if cut_timeline(v, over) {
            "timeline"
        } else {
            break;
        };
        if !cut.contains(&section) {
            cut.push(section);
        }
        v["truncated"] = json!(cut);
        len = render(v).len();
    }
}

/// Newest checkpoint first, newest file first: the oldest file is the last one.
fn cut_evidence(v: &mut Value, over: usize) -> bool {
    let Some(cps) = v["evidence"]["checkpoints"].as_array_mut() else {
        return false;
    };
    let mut freed = 0;
    while freed < over {
        let Some(cp) = cps.last_mut() else { break };
        match cp["files"].as_array_mut().and_then(|f| f.pop()) {
            Some(f) => freed += f.to_string().len() + 1,
            None => freed += cps.pop().map_or(0, |c| c.to_string().len() + 1),
        }
    }
    freed > 0
}

fn cut_timeline(v: &mut Value, over: usize) -> bool {
    let Some(items) = v["timeline"].as_array_mut() else {
        return false;
    };
    let mut freed = 0;
    let mut drop = 0;
    while freed < over && drop < items.len() {
        freed += items[drop].to_string().len() + 1;
        drop += 1;
    }
    items.drain(..drop);
    drop > 0
}

/// The decision entries in the feature's `log.jsonl`; 0 when there is no folder or no log.
fn decisions(ctx: &Ctx) -> usize {
    ctx.feature_dir
        .as_deref()
        .and_then(|dir| log::entries(dir).ok())
        .map_or(0, |(entries, _)| {
            entries
                .iter()
                .filter(|e| e.kind == log::Kind::Decision)
                .count()
        })
}

/// A file's mtime as UTC RFC3339, or `None` when it cannot be read.
pub fn mtime(path: &Path) -> Option<String> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(clock::rfc3339(t.duration_since(UNIX_EPOCH).ok()?.as_secs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bare() -> Value {
        let mut v = json!({});
        for k in KEYS.iter().rev() {
            v[*k] = Value::Null;
        }
        v["truncated"] = json!([]);
        v
    }

    #[test]
    fn the_envelope_prints_in_its_own_order() {
        let mut v = bare();
        v["schema"] = json!(1);
        v["now"] = json!({"b": 1, "a": 2});
        let line = render(&v);
        let want = KEYS
            .iter()
            .map(|k| match *k {
                "schema" => "\"schema\":1".to_string(),
                "now" => "\"now\":{\"a\":2,\"b\":1}".to_string(),
                "truncated" => "\"truncated\":[]".to_string(),
                k => format!("\"{k}\":null"),
            })
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(line, format!("{{{want}}}"));
    }

    #[test]
    fn rev_skips_the_fields_that_move_on_their_own() {
        let mut v = bare();
        v["now"] = json!({"waitingOn": "worker", "idleSeconds": 3});
        v["workers"] = json!([{"sessionId": "s", "lastWriteAt": "a", "running": {"elapsedSeconds": 1, "cmd": "x"}}]);
        let first = rev(&v);
        v["at"] = json!("later");
        v["rev"] = json!(first.clone());
        v["now"]["idleSeconds"] = json!(9);
        v["workers"][0]["lastWriteAt"] = json!("b");
        v["workers"][0]["running"]["elapsedSeconds"] = json!(7);
        assert_eq!(rev(&v), first);
        v["workers"][0]["running"]["cmd"] = json!("y");
        assert_ne!(rev(&v), first);
    }

    #[test]
    fn the_cap_cuts_evidence_before_the_timeline() {
        let mut v = bare();
        let file = |i: usize| json!({"path": format!("f{i}"), "pad": "e".repeat(1000)});
        v["evidence"] = json!({
            "checkpoints": [
                {"n": 2, "files": (0..50).map(file).collect::<Vec<_>>()},
                {"n": 1, "files": (50..100).map(file).collect::<Vec<_>>()},
            ]
        });
        v["timeline"] = (1..=700)
            .map(|seq| json!({"seq": seq, "label": "t".repeat(1000)}))
            .collect();
        cap(&mut v);
        assert!(render(&v).len() <= CAP);
        assert_eq!(v["truncated"], json!(["evidence", "timeline"]));
        assert_eq!(v["evidence"]["checkpoints"], json!([]));
        assert_eq!(
            v["timeline"].as_array().unwrap().last().unwrap()["seq"],
            700
        );

        let mut v = bare();
        v["evidence"] = json!({
            "checkpoints": [
                {"n": 2, "files": (0..300).map(file).collect::<Vec<_>>()},
                {"n": 1, "files": (300..600).map(file).collect::<Vec<_>>()},
            ]
        });
        cap(&mut v);
        assert_eq!(v["truncated"], json!(["evidence"]));
        let cps = v["evidence"]["checkpoints"].as_array().unwrap();
        assert_eq!(cps[0]["files"][0]["path"], "f0", "the newest file stays");
    }
}
