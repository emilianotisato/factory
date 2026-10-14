//! The row's key is the canonical repo root, as `runs.repo` holds it, and the feature number.
use std::fmt::Display;
use std::path::Path;

use crate::db::{Db, RETRO_COLUMNS, RetroRow};
use crate::docs::board;
use crate::error::io;
use crate::run::Run;
use crate::{Error, Result, paths};

struct Key {
    repo: String,
    feature: u32,
    slug: String,
}

fn key(root: &Path, feature: &str) -> Result<Key> {
    let board = board::scan(root)?;
    let f = board.resolve(feature)?;
    let repo = root
        .canonicalize()
        .map_err(io(format!("canonicalize {}", root.display())))?;
    Ok(Key {
        repo: repo.to_string_lossy().into_owned(),
        feature: f.number,
        slug: f.slug.clone(),
    })
}

fn store() -> Result<Db> {
    Db::open(&Run::db_path(&paths::data_dir()))
}

pub fn show(root: &Path, feature: &str) -> Result<Option<RetroRow>> {
    let k = key(root, feature)?;
    store()?.retro(&k.repo, k.feature)
}

pub fn set_score(root: &Path, feature: &str, value: &str) -> Result<RetroRow> {
    let score = parse_set(value)?;
    let k = key(root, feature)?;
    let db = store()?;
    db.set_retro_score(&k.repo, k.feature, &k.slug, score)?;
    db.retro(&k.repo, k.feature)?
        .ok_or_else(|| Error::Msg(format!("no retro row for PRD-{:03} after --set", k.feature)))
}

fn parse_set(pair: &str) -> Result<i64> {
    match pair.split_once('=') {
        Some(("score", v)) => v
            .trim()
            .parse()
            .map_err(|_| Error::Msg(format!("score must be an integer, not \"{v}\""))),
        _ => Err(Error::Msg(format!(
            "only `score` can be set, not \"{pair}\""
        ))),
    }
}

pub fn lines(r: &RetroRow) -> Vec<String> {
    fn opt<T: Display>(v: &Option<T>) -> String {
        v.as_ref().map_or_else(String::new, ToString::to_string)
    }
    let values = [
        r.repo.clone(),
        r.feature.to_string(),
        r.slug.clone(),
        opt(&r.shipped_at),
        opt(&r.run_id),
        opt(&r.score),
        opt(&r.gate_runs),
        opt(&r.gate_red),
        opt(&r.gate_minutes),
        opt(&r.ci_cycles),
        opt(&r.slices),
        opt(&r.checkpoints),
        opt(&r.build_minutes),
        opt(&r.polish_minutes),
        opt(&r.out_tokens),
        opt(&r.cache_read_tokens),
        opt(&r.start_context),
        opt(&r.peak_context),
        r.written_at.clone(),
    ];
    RETRO_COLUMNS
        .iter()
        .zip(values)
        .map(|(k, v)| format!("{k}: {v}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_integer_score_can_be_set() {
        assert_eq!(parse_set("score=4").unwrap(), 4);
        for (bad, says) in [
            ("score=high", "score must be an integer"),
            ("score=", "score must be an integer"),
            ("gate_runs=3", "only `score` can be set"),
            ("score", "only `score` can be set"),
        ] {
            let e = parse_set(bad).unwrap_err().to_string();
            assert!(e.contains(says), "{bad}: {e}");
        }
    }

    #[test]
    fn lines_follow_the_columns_with_empty_nulls() {
        let r = RetroRow {
            repo: "/repo".into(),
            feature: 1,
            slug: "fixture".into(),
            shipped_at: None,
            run_id: None,
            score: Some(4),
            gate_runs: None,
            gate_red: None,
            gate_minutes: Some(2.23),
            ci_cycles: None,
            slices: None,
            checkpoints: None,
            build_minutes: Some(16.0),
            polish_minutes: None,
            out_tokens: None,
            cache_read_tokens: None,
            start_context: None,
            peak_context: None,
            written_at: "2026-10-13T10:00:00Z".into(),
        };
        let l = lines(&r);
        assert_eq!(l.len(), RETRO_COLUMNS.len());
        assert_eq!(l[0], "repo: /repo");
        assert_eq!(l[3], "shipped_at: ");
        assert_eq!(l[5], "score: 4");
        assert_eq!(l[8], "gate_minutes: 2.23");
        assert_eq!(l[12], "build_minutes: 16");
        assert_eq!(l[18], "written_at: 2026-10-13T10:00:00Z");
    }
}
