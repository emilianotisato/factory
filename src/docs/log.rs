//! A feature's append-only `log.jsonl` (PRD-002 FR-14): one JSON object per
//! line, appended only by [`add`], which allocates the id under the feature's
//! `docs.lock` (DEC-3). [`entries`] reads it the way `events::scan` reads
//! `events.jsonl`: a line that does not parse is skipped and returned, never a
//! reason to fail. [`render`] prints today's `agent-log.md` shape on demand;
//! no rendered copy lives in the repo.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::state;
use crate::clock;
use crate::error::{io, json};
use crate::{Error, Result};

pub const FILE: &str = "log.jsonl";

/// The `phase` vocabulary (prd §4, added at slicing).
pub const PHASES: [&str; 6] = ["preflight", "slicing", "checkpoint", "polish", "pr", "ci"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Decision,
    Block,
    Answer,
    Checkpoint,
    Ask,
    Review,
    Note,
}

impl Kind {
    /// The id prefix of the kind's sequence; `None` for an id-less kind.
    /// `decision` and `ask` share `AD`, so `log show` keeps one AD list.
    pub fn sequence(self) -> Option<&'static str> {
        match self {
            Kind::Decision | Kind::Ask => Some("AD"),
            Kind::Block => Some("BLOCK"),
            Kind::Checkpoint => Some("CP"),
            Kind::Answer | Kind::Review | Kind::Note => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Kind::Decision => "decision",
            Kind::Block => "block",
            Kind::Answer => "answer",
            Kind::Checkpoint => "checkpoint",
            Kind::Ask => "ask",
            Kind::Review => "review",
            Kind::Note => "note",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Flag {
    Fyi,
    Decide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Kept,
    Changed,
}

/// One line of `log.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub kind: Kind,
    #[serde(default)]
    pub ts: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slice: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flag: Option<Flag>,
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verdict: Option<Verdict>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
}

impl Entry {
    /// An entry with only its kind, title and body set.
    pub fn new(kind: Kind, title: &str, body: &str) -> Entry {
        Entry {
            id: None,
            kind,
            ts: String::new(),
            slice: None,
            phase: None,
            flag: None,
            reference: None,
            verdict: None,
            title: title.to_string(),
            body: body.to_string(),
        }
    }

    /// The `n` of `AD-n`, `BLOCK-n` or `CP-n`.
    pub fn n(&self) -> Option<u32> {
        id_number(self.id.as_deref()?).map(|(_, n)| n)
    }

    fn date(&self) -> &str {
        self.ts.get(..10).unwrap_or(&self.ts)
    }
}

/// A line of `log.jsonl` that does not parse: `(line number, error)`, 1-based.
pub type Skipped = (usize, String);

fn id_number(id: &str) -> Option<(&str, u32)> {
    let (prefix, n) = id.rsplit_once('-')?;
    Some((prefix, n.parse().ok()?))
}

pub fn path(dir: &Path) -> PathBuf {
    dir.join(FILE)
}

fn parse(bytes: &[u8]) -> (Vec<Entry>, Vec<Skipped>) {
    let mut out = Vec::new();
    let mut bad = Vec::new();
    for (i, l) in String::from_utf8_lossy(bytes).lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        match serde_json::from_str(l) {
            Ok(e) => out.push(e),
            Err(e) => bad.push((i + 1, e.to_string())),
        }
    }
    (out, bad)
}

fn read(p: &Path) -> Result<Vec<u8>> {
    match fs::read(p) {
        Ok(b) => Ok(b),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(io(format!("read {}", p.display()))(e)),
    }
}

/// Every entry of `<dir>/log.jsonl` in file order, and each line that does not
/// parse. A feature with no log reads as an empty one.
pub fn entries(dir: &Path) -> Result<(Vec<Entry>, Vec<Skipped>)> {
    Ok(parse(&read(&path(dir))?))
}

fn refuse<T>(msg: String) -> Result<T> {
    Err(Error::Msg(msg))
}

/// What can be refused without reading the log.
fn validate(e: &Entry) -> Result<()> {
    let kind = e.kind.name();
    if e.flag.is_some() && e.kind != Kind::Decision {
        return refuse(format!("--flag is for a decision, not a {kind}"));
    }
    match (e.kind, e.verdict) {
        (Kind::Review, None) => return refuse("a review needs --verdict kept|changed".into()),
        (Kind::Review, Some(_)) | (_, None) => {}
        (_, Some(_)) => return refuse(format!("--verdict is for a review, not a {kind}")),
    }
    let refers = matches!(e.kind, Kind::Answer | Kind::Review);
    if refers && e.reference.is_none() {
        return refuse(format!("a {kind} needs --ref, the id it points at"));
    }
    if !refers && e.reference.is_some() {
        return refuse(format!("--ref is for an answer or a review, not a {kind}"));
    }
    if let Some(p) = &e.phase
        && !PHASES.contains(&p.as_str())
    {
        return refuse(format!("phase \"{p}\" is not one of {}", PHASES.join(", ")));
    }
    if e.kind == Kind::Checkpoint && e.slice.is_none() {
        return refuse("a checkpoint needs --slice, the last slice it covers".into());
    }
    Ok(())
}

/// Append `entry` to `<dir>/log.jsonl` with the next id of its sequence (the
/// highest `n` read plus one) and a fresh `ts`, under the feature's
/// `docs.lock`; returns the id, `None` for an `answer`, `review` or `note`.
/// The line goes out in one write, on a line of its own after a torn one.
pub fn add(dir: &Path, mut entry: Entry) -> Result<Option<String>> {
    validate(&entry)?;
    let _lock = state::lock(dir)?;
    let p = path(dir);
    let bytes = read(&p)?;
    let (have, _) = parse(&bytes);
    // Inside the lock: a ref read before it could miss a BLOCK being written.
    if let Some(r) = &entry.reference
        && !have.iter().any(|e| e.id.as_deref() == Some(r))
    {
        return refuse(format!("--ref {r}: the log holds no such id"));
    }
    entry.id = entry.kind.sequence().map(|seq| {
        let last = have
            .iter()
            .filter_map(|e| id_number(e.id.as_deref()?))
            .filter(|(prefix, _)| *prefix == seq)
            .map(|(_, n)| n)
            .max()
            .unwrap_or(0);
        format!("{seq}-{}", last + 1)
    });
    entry.ts = clock::now_rfc3339();
    let line = serde_json::to_string(&entry).map_err(json("serialize log entry"))? + "\n";
    let torn = bytes.last().is_some_and(|b| *b != b'\n');
    let line = if torn { format!("\n{line}") } else { line };
    let mut f = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&p)
        .map_err(io(format!("open {}", p.display())))?;
    f.write_all(line.as_bytes())
        .map_err(io(format!("append {}", p.display())))?;
    Ok(entry.id)
}

/// Which part of the log `log show` prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    /// `decide` decisions no `review` refs: what `/factory-deliver` §3 reads.
    Decide,
    /// Blocks no `answer` refs.
    Open,
}

fn refs<'a>(all: &'a [Entry], kind: Kind, id: Option<&str>) -> Vec<&'a Entry> {
    all.iter()
        .filter(|e| e.kind == kind && id.is_some() && e.reference.as_deref() == id)
        .collect()
}

/// The entries of `kinds`, in id order.
fn by_n<'a>(all: &'a [Entry], kinds: &[Kind]) -> Vec<&'a Entry> {
    let mut v: Vec<&Entry> = all.iter().filter(|e| kinds.contains(&e.kind)).collect();
    v.sort_by_key(|e| e.n().unwrap_or(u32::MAX));
    v
}

/// The entries `filter` keeps, in the order `log show` prints them.
pub fn select(all: &[Entry], filter: Filter) -> Vec<&Entry> {
    match filter {
        Filter::All => all.iter().collect(),
        Filter::Decide => by_n(all, &[Kind::Decision])
            .into_iter()
            .filter(|e| e.flag == Some(Flag::Decide))
            .filter(|e| refs(all, Kind::Review, e.id.as_deref()).is_empty())
            .collect(),
        Filter::Open => by_n(all, &[Kind::Block])
            .into_iter()
            .filter(|e| refs(all, Kind::Answer, e.id.as_deref()).is_empty())
            .collect(),
    }
}

/// `slice-NN`, the phase, or both, as the heading's middle field.
fn place(e: &Entry) -> String {
    let mut parts = Vec::new();
    if let Some(s) = e.slice {
        parts.push(format!("slice-{s:02}"));
    }
    if let Some(p) = &e.phase {
        parts.push(p.clone());
    }
    parts.iter().map(|p| format!(" · {p}")).collect()
}

fn push_body(out: &mut String, body: &str) {
    if body.is_empty() {
        return;
    }
    out.push('\n');
    out.push_str(body);
    if !body.ends_with('\n') {
        out.push('\n');
    }
}

fn block(out: &mut String, all: &[Entry], e: &Entry) {
    let answers = refs(all, Kind::Answer, e.id.as_deref());
    let state = if answers.is_empty() {
        "open"
    } else {
        "resolved"
    };
    out.push_str(&format!(
        "\n### {} — {}{} · {state}\n",
        e.id.as_deref().unwrap_or("BLOCK-?"),
        e.title,
        place(e)
    ));
    push_body(out, &e.body);
    for a in answers {
        out.push_str(&format!(
            "\n**Resolution ({}):** {}\n",
            a.date(),
            a.body.trim_end()
        ));
    }
}

fn decision(out: &mut String, all: &[Entry], e: &Entry) {
    out.push_str(&format!(
        "\n### {} — {}{} · {}\n",
        e.id.as_deref().unwrap_or("AD-?"),
        e.title,
        place(e),
        e.date()
    ));
    push_body(out, &e.body);
    // A migrated entry keeps a Review line that carries a reason in its body.
    let reviewed = e.body.lines().any(|l| l.starts_with("**Review:**"));
    if let Some(f) = e.flag.filter(|_| !reviewed) {
        let f = if f == Flag::Decide { "decide" } else { "fyi" };
        out.push_str(&format!("\n**Review:** {f}\n"));
    }
    for r in refs(all, Kind::Review, e.id.as_deref()) {
        let verdict = match r.verdict {
            Some(Verdict::Changed) => "changed",
            _ => "kept",
        };
        let body = r.body.trim_end();
        let tail = if body.is_empty() {
            String::new()
        } else {
            format!(" — {body}")
        };
        out.push_str(&format!("\n**Owner ({}):** {verdict}{tail}\n", r.date()));
    }
}

fn checkpoint(out: &mut String, e: &Entry) {
    out.push_str(&format!(
        "\n### {} — after slice-{:02} · {}\n",
        e.id.as_deref().unwrap_or("CP-?"),
        e.slice.unwrap_or(0),
        e.date()
    ));
    push_body(out, &e.body);
}

fn section(out: &mut String, heading: &str, items: &[&Entry], each: impl Fn(&mut String, &Entry)) {
    out.push_str(&format!("\n## {heading}\n"));
    if items.is_empty() {
        out.push_str("\n_(none)_\n");
    }
    for e in items {
        each(out, e);
    }
}

/// The log as markdown in today's `agent-log.md` shape: `## Notes`,
/// `## Blockers`, `## Decisions`, `## Checkpoints`, each entry with the
/// answers and reviews that ref it underneath. `number` titles it `PRD-NNN`.
pub fn render(all: &[Entry], filter: Filter, number: u32) -> String {
    let mut out = String::new();
    match filter {
        Filter::Decide => {
            section(&mut out, "Decisions", &select(all, filter), |o, e| {
                decision(o, all, e)
            });
        }
        Filter::Open => {
            section(&mut out, "Blockers", &select(all, filter), |o, e| {
                block(o, all, e)
            });
        }
        Filter::All => {
            out.push_str(&format!("# Agent log — PRD-{number:03}\n"));
            let notes: Vec<&Entry> = all.iter().filter(|e| e.kind == Kind::Note).collect();
            section(&mut out, "Notes", &notes, |o, e| push_body(o, &e.body));
            section(&mut out, "Blockers", &by_n(all, &[Kind::Block]), |o, e| {
                block(o, all, e)
            });
            section(
                &mut out,
                "Decisions",
                &by_n(all, &[Kind::Decision, Kind::Ask]),
                |o, e| decision(o, all, e),
            );
            section(
                &mut out,
                "Checkpoints",
                &by_n(all, &[Kind::Checkpoint]),
                checkpoint,
            );
        }
    }
    out.trim_start_matches('\n').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: Kind) -> Entry {
        Entry::new(kind, "t", "")
    }

    fn lines(d: &tempfile::TempDir) -> Vec<String> {
        fs::read_to_string(path(d.path()))
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect()
    }

    #[test]
    fn ids_follow_their_sequence_and_decision_and_ask_share_ad() {
        let d = tempfile::tempdir().unwrap();
        let mut cp = entry(Kind::Checkpoint);
        cp.slice = Some(2);
        let mut ids = Vec::new();
        for e in [
            entry(Kind::Decision),
            entry(Kind::Ask),
            entry(Kind::Block),
            cp,
            entry(Kind::Decision),
        ] {
            ids.push(add(d.path(), e).unwrap().unwrap());
        }
        assert_eq!(ids, ["AD-1", "AD-2", "BLOCK-1", "CP-1", "AD-3"]);
        let l = lines(&d);
        assert_eq!(l.len(), 5);
        for line in &l {
            serde_json::from_str::<Entry>(line).unwrap();
        }

        let mut a = entry(Kind::Answer);
        a.reference = Some("BLOCK-1".into());
        assert_eq!(add(d.path(), a).unwrap(), None);
        let (all, bad) = entries(d.path()).unwrap();
        assert!(bad.is_empty());
        assert_eq!(all[5].reference.as_deref(), Some("BLOCK-1"));
        assert!(lines(&d)[5].contains(r#""ref":"BLOCK-1""#));
        assert!(
            !lines(&d)[0].contains("flag"),
            "absent options are not written"
        );
    }

    #[test]
    fn refusals_leave_the_file_unchanged() {
        let d = tempfile::tempdir().unwrap();
        add(d.path(), entry(Kind::Block)).unwrap();
        let before = fs::read(path(d.path())).unwrap();

        let mut dangling = entry(Kind::Answer);
        dangling.reference = Some("BLOCK-9".into());
        let mut verdict = entry(Kind::Decision);
        verdict.verdict = Some(Verdict::Kept);
        let mut flag = entry(Kind::Block);
        flag.flag = Some(Flag::Fyi);
        let no_slice = entry(Kind::Checkpoint);
        let mut phase = entry(Kind::Block);
        phase.phase = Some("lunch".into());
        let mut no_verdict = entry(Kind::Review);
        no_verdict.reference = Some("BLOCK-1".into());
        for e in [dangling, verdict, flag, no_slice, phase, no_verdict] {
            let err = add(d.path(), e.clone()).unwrap_err();
            assert!(matches!(err, Error::Msg(_)), "{e:?} refused with {err}");
        }
        assert_eq!(fs::read(path(d.path())).unwrap(), before);
    }

    #[test]
    fn a_torn_last_line_keeps_its_own_line() {
        let d = tempfile::tempdir().unwrap();
        add(d.path(), entry(Kind::Decision)).unwrap();
        let mut f = OpenOptions::new()
            .append(true)
            .open(path(d.path()))
            .unwrap();
        f.write_all(br#"{"id":"AD-2","kind":"deci"#).unwrap();
        assert_eq!(
            add(d.path(), entry(Kind::Decision)).unwrap().as_deref(),
            Some("AD-2"),
            "the torn line holds no id"
        );
        let (all, bad) = entries(d.path()).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(bad.len(), 1);
        assert_eq!(bad[0].0, 2);
        assert_eq!(lines(&d).len(), 3);
    }

    /// The fixed log of the slice's acceptance, review and answer last so a
    /// prefix of it is the log before them.
    fn fixed() -> Vec<Entry> {
        let ts = "2026-10-13T10:00:00Z";
        let mk = |id: Option<&str>, kind: Kind, title: &str, body: &str| Entry {
            id: id.map(String::from),
            ts: ts.into(),
            ..Entry::new(kind, title, body)
        };
        let mut ad1 = mk(
            Some("AD-1"),
            Kind::Decision,
            "Which lock?",
            "**Chose:** flock\n",
        );
        ad1.slice = Some(2);
        ad1.flag = Some(Flag::Decide);
        let mut ad2 = mk(
            Some("AD-2"),
            Kind::Decision,
            "Helper?",
            "**Chose:** extract\n",
        );
        ad2.slice = Some(3);
        ad2.flag = Some(Flag::Fyi);
        let mut ad3 = mk(
            Some("AD-3"),
            Kind::Ask,
            "Use the cache?",
            "Yes, per the owner.\n",
        );
        ad3.phase = Some("preflight".into());
        let mut b1 = mk(
            Some("BLOCK-1"),
            Kind::Block,
            "Gate red",
            "**Happened:** red\n",
        );
        b1.phase = Some("checkpoint".into());
        let mut cp = mk(
            Some("CP-1"),
            Kind::Checkpoint,
            "after 2",
            "**Covers:** slices 01, 02\n",
        );
        cp.slice = Some(2);
        let note = mk(
            None,
            Kind::Note,
            "Traps",
            "- `cargo test` needs no network.\n",
        );
        let mut answer = mk(None, Kind::Answer, "answer", "Rerun after the fix.\n");
        answer.reference = Some("BLOCK-1".into());
        let mut review = mk(None, Kind::Review, "review", "");
        review.reference = Some("AD-1".into());
        review.verdict = Some(Verdict::Kept);
        vec![ad1, ad2, ad3, b1, cp, note, answer, review]
    }

    #[test]
    fn render_matches_the_snapshot() {
        let all = fixed();
        let got = render(&all, Filter::All, 1);
        let snap = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/docs-log/show.md");
        if std::env::var_os("FACTORY_BLESS").is_some() {
            fs::create_dir_all(snap.parent().unwrap()).unwrap();
            fs::write(&snap, &got).unwrap();
        }
        assert_eq!(
            got,
            fs::read_to_string(&snap).unwrap(),
            "render printed:\n{got}"
        );
        assert!(got.contains("### BLOCK-1 — Gate red · checkpoint · resolved"));
    }

    #[test]
    fn decide_and_open_drop_what_was_reviewed_or_answered() {
        let all = fixed();
        let ids =
            |v: Vec<&Entry>| -> Vec<String> { v.iter().filter_map(|e| e.id.clone()).collect() };
        let before_answer = &all[..6];
        let before_review = &all[..7];
        assert_eq!(ids(select(before_review, Filter::Decide)), ["AD-1"]);
        assert!(select(&all, Filter::Decide).is_empty());
        assert_eq!(ids(select(before_answer, Filter::Open)), ["BLOCK-1"]);
        assert!(select(before_review, Filter::Open).is_empty());
        assert!(render(before_review, Filter::Decide, 1).contains("### AD-1 — Which lock?"));
        assert_eq!(
            render(&all, Filter::Decide, 1),
            "## Decisions\n\n_(none)_\n"
        );
        assert_eq!(
            render(before_review, Filter::Open, 1),
            "## Blockers\n\n_(none)_\n"
        );
    }
}
