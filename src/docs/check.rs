//! A rule records a finding instead of failing, so one bad file never hides the rest of the board.
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use super::board::{self, Feature, Kind};
use super::frontmatter::Value;
use super::ideas::{self, Bad};
use super::log::{self, Kind as LogKind};
use super::project::{self, Project};
use super::state::{self, SliceState, State};
use crate::Result;
use crate::error::io;

const SECOND_BOARD_NAMES: [&str; 6] = [
    "ROADMAP.md",
    "TASKS.md",
    "PLAN.md",
    "STATUS.md",
    "BACKLOG.md",
    "TODO.md",
];
const LEGACY_DOC_DIRS: [&str; 4] = [".vibe", "plans", "ai_docs", "docs/prd"];
/// Required; other keys such as `from_idea` pass.
const PRD_KEYS: [&str; 7] = [
    "number", "slug", "title", "type", "created", "author", "summary",
];
const PRD_TYPES: [&str; 4] = ["feature", "bugfix", "enhancement", "refactor"];

/// `(code, message)` pairs.
#[derive(Debug, Default)]
pub struct Report {
    pub errors: Vec<(String, String)>,
    pub warnings: Vec<(String, String)>,
}

impl Report {
    fn err(&mut self, code: &str, msg: impl Into<String>) {
        self.errors.push((code.into(), msg.into()));
    }

    fn warn(&mut self, code: &str, msg: impl Into<String>) {
        self.warnings.push((code.into(), msg.into()));
    }

    pub fn is_clean(&self) -> bool {
        self.errors.is_empty() && self.warnings.is_empty()
    }

    pub fn counts(&self) -> String {
        let plural = |n: usize, w: &str| format!("{n} {w}{}", if n == 1 { "" } else { "s" });
        format!(
            "{}, {}",
            plural(self.errors.len(), "error"),
            plural(self.warnings.len(), "warning")
        )
    }

    pub fn lines(&self) -> Vec<String> {
        let e = self.errors.iter().map(|(c, m)| format!("  ✗ {c}  {m}"));
        let w = self.warnings.iter().map(|(c, m)| format!("  ! {c}  {m}"));
        e.chain(w).collect()
    }

    pub fn print(&self, w: &mut dyn Write) -> Result<()> {
        let mut text = self.lines().join("\n");
        text.push_str(&format!("\n\n{}\n", self.counts()));
        w.write_all(text.as_bytes()).map_err(io("write report"))
    }

    pub fn codes(&self) -> Vec<&str> {
        self.errors
            .iter()
            .chain(&self.warnings)
            .map(|(c, _)| c.as_str())
            .collect()
    }
}

pub fn run(root: &Path) -> Report {
    let mut r = Report::default();
    let rel = |p: &Path| p.strip_prefix(root).unwrap_or(p).display().to_string();
    let project = read_project(root, &mut r);
    if let Some(p) = &project
        && p.as_ref().and_then(|p| p.gate.as_ref()).is_none()
    {
        r.warn(
            "W-NO-GATE",
            "no verify gate declared in docs/project.toml — run /gate, or `factory docs gate set \"<cmd>\"` (`factory docs gate --candidates` lists likely ones)",
        );
    }
    let limits = project.flatten().unwrap_or(Project {
        framework: 3,
        gate: None,
        inner: None,
        lanes: 1,
        gate_parallel: None,
        checkpoint_every: 2,
        prd_max_kb: 120.0,
        slice_max_kb: 30.0,
    });
    let docs = root.join("docs");
    let mut folder = |name: &str, kind: Kind| match board::scan_folder(&docs.join(name), kind) {
        Ok(f) => f,
        Err(e) => {
            r.err("E-FM-INVALID", format!("docs/{name}: {e}"));
            Vec::new()
        }
    };
    let live = folder("features", Kind::Live);
    let archived = folder("archive", Kind::Archive);

    let mut by_number: HashMap<u32, &str> = HashMap::new();
    for f in live.iter().chain(&archived) {
        match by_number.get(&f.number) {
            Some(first) => r.err(
                "E-NUM-DUPLICATE",
                format!(
                    "PRD {:03} claimed by both {first} and {}",
                    f.number, f.dir_name
                ),
            ),
            None => {
                by_number.insert(f.number, &f.dir_name);
            }
        }
    }
    for f in &live {
        feature(root, &limits, f, &mut r);
    }
    for f in &archived {
        let own = f
            .state()
            .map(|s| s.status.clone())
            .or_else(|| f.prd.as_ref()?.0.get("status")?.text());
        if let Some(st) = own
            && st != "archived"
            && st != "retired"
        {
            r.err(
                "E-STATUS-PLACE",
                format!(
                    "{} is under archive/ but its status is \"{st}\"",
                    rel(&f.dir)
                ),
            );
        }
    }
    archive_frozen(root, &mut r);
    home_paths(root, &mut r);
    links(root, &mut r);
    ideas(root, &mut r);
    for dir in [root.to_path_buf(), docs] {
        for name in SECOND_BOARD_NAMES {
            let f = dir.join(name);
            if f.exists() {
                r.warn(
                    "W-SECOND-BOARD",
                    format!(
                        "{} looks like a second status board — one index only, fold it into docs/index.md or docs/ideas/",
                        rel(&f)
                    ),
                );
            }
        }
    }
    for dir in LEGACY_DOC_DIRS {
        if root.join(dir).exists() {
            r.warn(
                "W-LEGACY-DOCS",
                format!(
                    "{dir}/ still exists alongside docs/ — an older planning convention; migrate or delete it"
                ),
            );
        }
    }
    r
}

/// `ideas::read` is the reader `idea list` uses, so what passes here lists.
fn ideas(root: &Path, r: &mut Report) {
    for (rel, path) in ideas::files(root) {
        match ideas::read(&path) {
            Ok(_) => {}
            Err(Bad::Name(m)) => r.err("E-IDEA-NAME", format!("{rel}: {m}")),
            Err(Bad::Parse(m)) => r.err("E-IDEA-PARSE", format!("{rel}: {m}")),
            Err(Bad::Field(m)) => r.err("E-IDEA-FIELD", format!("{rel}: {m}")),
        }
    }
    if root.join("docs/ideas.md").exists() {
        r.warn(
            "W-IDEAS-V2",
            "docs/ideas.md is the v2 inbox, and an old skill may still write to it — move each line with `factory docs idea add`, then delete the file",
        );
    }
}

/// Outer `None`: the file does not parse (recorded); inner: nothing for `project::load` to read.
fn read_project(root: &Path, r: &mut Report) -> Option<Option<Project>> {
    let p = project::path(root);
    let Ok(text) = fs::read_to_string(&p) else {
        return Some(None);
    };
    let table: toml::Table = match text.parse() {
        Ok(t) => t,
        Err(e) => {
            let e: toml::de::Error = e;
            r.err(
                "E-PROJECT-INVALID",
                format!("docs/project.toml does not parse: {}", e.message()),
            );
            return None;
        }
    };
    if let Some(toml::Value::Table(docs)) = table.get("docs") {
        for (k, v) in docs {
            use toml::Value as T;
            let ok = match k.as_str() {
                "framework" | "lanes" | "checkpoint_every" => matches!(v, T::Integer(_)),
                "gate" | "inner" => matches!(v, T::String(_)),
                "gate_parallel" => {
                    matches!(v, T::Boolean(_))
                        || matches!(v, T::String(s) if s == "true" || s == "false")
                }
                "prd_max_kb" | "slice_max_kb" => matches!(v, T::Integer(_) | T::Float(_)),
                _ => true,
            };
            if !ok {
                r.err(
                    "E-PROJECT-INVALID",
                    format!("docs/project.toml [docs] {k} = {v} is not a {}", kind_of(k)),
                );
            }
        }
    }
    Some(project::load(root).ok().flatten())
}

fn kind_of(key: &str) -> &'static str {
    match key {
        "gate" | "inner" => "string",
        "gate_parallel" => "boolean",
        "prd_max_kb" | "slice_max_kb" => "number",
        _ => "integer",
    }
}

fn feature(root: &Path, limits: &Project, f: &Feature, r: &mut Report) {
    let dir = format!("docs/features/{}", f.dir_name);
    let nnn = format!("{:03}", f.number);
    let prd = format!("{dir}/prd.md");
    let state_rel = format!("{dir}/{}", state::FILE);
    let kb = |text: &str| text.len() as f64 / 1024.0;

    let st = match &f.state {
        Err(e) => {
            let e = e.replace(&format!("{}/", root.display()), "");
            r.err("E-STATE-INVALID", e);
            None
        }
        Ok(None) => {
            r.err(
                "E-STATE-INVALID",
                format!(
                    "{dir} has no state.toml — `factory docs set {nnn} status=draft` writes one"
                ),
            );
            None
        }
        Ok(Some(s)) => Some(s),
    };
    let status = st.map(|s| s.status.as_str());

    match &f.prd {
        None => r.err("E-NO-PRD", format!("{dir} has no prd.md")),
        Some((fm, text)) => {
            if kb(text) > limits.prd_max_kb {
                r.warn(
                    "W-SIZE",
                    format!(
                        "{prd} is {}KB / {} lines — slice, don't fatten (ceiling {}KB)",
                        kb(text).round(),
                        text.split('\n').count(),
                        limits.prd_max_kb
                    ),
                );
            }
            if let Some(e) = &fm.error {
                r.err("E-FM-INVALID", format!("{prd}: {e}"));
            } else if fm.data.is_none() {
                r.err(
                    "E-FM-MISSING",
                    format!("{prd} has no frontmatter (needs {})", PRD_KEYS.join(", ")),
                );
            } else {
                prd_keys(f, &prd, r);
            }
            if matches!(status, Some("ready" | "in-progress")) && !board::has_qa_plan(text) {
                r.warn(
                    "W-NO-QA-PLAN",
                    format!(
                        "{prd} is {} but has no \"QA plan\" section — every FR needs a proof row before it is sliced (see /prd)",
                        status.unwrap_or_default()
                    ),
                );
            }
            if status == Some("ready") && fm.get("summary").is_some() && f.summary().is_none() {
                r.warn(
                    "W-SUMMARY-EMPTY",
                    format!(
                        "{prd} is ready but its summary is empty — `factory docs status` prints it; write two or three lines"
                    ),
                );
            }
        }
    }

    let mut seen = HashSet::new();
    for s in &f.slices {
        if !seen.insert(s.number) {
            r.err(
                "E-SLICE-DUPLICATE",
                format!("{dir} has two slice-{:02} files", s.number),
            );
        }
        let Ok(text) = fs::read_to_string(f.dir.join(&s.file)) else {
            continue;
        };
        let spec = text.len() - progress_len(&text);
        if spec as f64 / 1024.0 > limits.slice_max_kb {
            r.warn(
                "W-SIZE",
                format!(
                    "{dir}/{} spec is {}KB ({}KB with Progress) — split the slice (ceiling {}KB)",
                    s.file,
                    (spec as f64 / 1024.0).round(),
                    kb(&text).round(),
                    limits.slice_max_kb
                ),
            );
        }
    }

    if let Some(s) = st {
        state_rules(&dir, &state_rel, &nnn, f, s, r);
    }
    log_rules(&dir, &nnn, f, st, r);
}

fn prd_keys(f: &Feature, prd: &str, r: &mut Report) {
    let Some((fm, _)) = &f.prd else { return };
    let missing: Vec<&str> = PRD_KEYS
        .iter()
        .copied()
        .filter(|k| fm.get(k).is_none())
        .collect();
    if !missing.is_empty() {
        r.err(
            "E-FM-MISSING",
            format!("{prd} frontmatter is missing {}", missing.join(", ")),
        );
    }
    match fm.get("number") {
        Some(Value::Num(n)) if *n != i64::from(f.number) => r.err(
            "E-NUM-MISMATCH",
            format!("{prd} frontmatter number {n} ≠ folder {:03}", f.number),
        ),
        Some(Value::Num(_)) | None => {}
        Some(v) => invalid(r, prd, "number", format!("{v:?} is not an integer")),
    }
    let text = |k: &str| {
        fm.get(k)
            .and_then(Value::text)
            .filter(|s| !s.trim().is_empty())
    };
    for k in ["slug", "title", "author"] {
        if fm.get(k).is_some() && text(k).is_none() {
            invalid(r, prd, k, "is empty".into());
        }
    }
    if let Some(slug) = text("slug")
        && slug != f.slug
    {
        r.err(
            "E-SLUG-MISMATCH",
            format!("{prd} frontmatter slug \"{slug}\" ≠ folder \"{}\"", f.slug),
        );
    }
    if fm.get("type").is_some() {
        let t = text("type").unwrap_or_default();
        if !PRD_TYPES.contains(&t.as_str()) {
            invalid(
                r,
                prd,
                "type",
                format!("\"{t}\" is not one of {}", PRD_TYPES.join(" | ")),
            );
        }
    }
    if fm.get("created").is_some() {
        let c = text("created").unwrap_or_default();
        if !state::is_date(&c) {
            invalid(
                r,
                prd,
                "created",
                format!("\"{c}\" is not a date (YYYY-MM-DD)"),
            );
        }
    }
    if let Some(Value::List(_)) = fm.get("summary") {
        invalid(r, prd, "summary", "is a list, not text".into());
    }
}

fn invalid(r: &mut Report, prd: &str, key: &str, why: String) {
    r.err("E-FM-INVALID", format!("{prd}: {key} {why}"));
}

fn state_rules(dir: &str, state_rel: &str, nnn: &str, f: &Feature, s: &State, r: &mut Report) {
    if !state::STATUSES.contains(&s.status.as_str()) {
        r.err(
            "E-STATE-INVALID",
            format!(
                "{state_rel} status \"{}\" is not one of {}",
                s.status,
                state::STATUSES.join(" | ")
            ),
        );
    }
    if matches!(s.status.as_str(), "archived" | "retired") {
        r.err(
            "E-STATUS-PLACE",
            format!(
                "{state_rel} says {} but the folder is still under features/ — run `factory docs archive {nnn}`",
                s.status
            ),
        );
    }
    for (n, sl) in &s.slices {
        if !state::SLICE_STATUSES.contains(&sl.status.as_str()) {
            r.err(
                "E-STATE-INVALID",
                format!(
                    "{state_rel} slices.{n}.status \"{}\" is not one of {}",
                    sl.status,
                    state::SLICE_STATUSES.join(" | ")
                ),
            );
        }
    }
    let files: Vec<u32> = f.slices.iter().map(|s| s.number).collect();
    for sf in &f.slices {
        if !s.slices.contains_key(&sf.number) {
            r.err(
                "E-SLICE-NO-STATE",
                format!(
                    "{dir}/{} has no [slices.{}] in state.toml — `factory docs set {nnn} slices.{}.status=pending` adds it",
                    sf.file, sf.number, sf.number
                ),
            );
        }
    }
    for (n, sl) in &s.slices {
        if !files.contains(n) {
            r.err(
                "E-STATE-NO-SLICE",
                format!("{state_rel} has [slices.{n}] but there is no slice-{n:02}-*.md"),
            );
        }
        for d in &sl.depends_on {
            if !s.slices.contains_key(d) && !files.contains(d) {
                r.err(
                    "E-DEP-MISSING",
                    format!("{state_rel} slices.{n} depends_on slice {d}, which does not exist"),
                );
            }
        }
    }
    if let Some(cycle) = find_cycle(&s.slices) {
        let path: Vec<String> = cycle.iter().map(|n| format!("{n:02}")).collect();
        r.err(
            "E-DEP-CYCLE",
            format!("{dir} slice dependency cycle: {}", path.join(" → ")),
        );
    }

    let st = s.status.as_str();
    let open: Vec<String> = s
        .slices
        .iter()
        .filter(|(_, sl)| sl.status != "done")
        .map(|(n, _)| format!("{n:02}"))
        .collect();
    if matches!(st, "shipped" | "archived") && !open.is_empty() {
        r.err(
            "E-STATUS-INCONSISTENT",
            format!(
                "PRD-{nnn} is {st} but {} slice(s) are not done: {}",
                open.len(),
                open.join(", ")
            ),
        );
    }
    if matches!(st, "draft" | "ready") && s.slices.values().any(|sl| sl.status != "pending") {
        r.warn(
            "W-STATUS-BEHIND",
            format!(
                "PRD-{nnn} is {st} but a slice is already building/done — should it be in-progress? (`factory docs set {nnn} status=in-progress`)"
            ),
        );
    }
    if !s.slices.is_empty() && open.is_empty() && matches!(st, "draft" | "ready" | "in-progress") {
        match &s.reviewed {
            None => r.warn(
                "W-NOT-REVIEWED",
                format!(
                    "PRD-{nnn}: every slice is done but it has not been through /polish — review, then `factory docs set {nnn} reviewed=<date>`"
                ),
            ),
            Some(d) => r.warn(
                "W-READY-TO-SHIP",
                format!(
                    "PRD-{nnn}: reviewed {d}, every slice done, status still \"{st}\" — run `factory docs ship {nnn}` before you merge"
                ),
            ),
        }
    }
}

fn find_cycle(slices: &BTreeMap<u32, SliceState>) -> Option<Vec<u32>> {
    fn visit(
        n: u32,
        trail: &mut Vec<u32>,
        slices: &BTreeMap<u32, SliceState>,
        marks: &mut HashMap<u32, u8>,
    ) -> Option<Vec<u32>> {
        match marks.get(&n) {
            Some(1) => {
                let i = trail.iter().position(|t| *t == n).unwrap_or(0);
                let mut c = trail[i..].to_vec();
                c.push(n);
                return Some(c);
            }
            Some(_) => return None,
            None => {}
        }
        marks.insert(n, 1);
        trail.push(n);
        for d in &slices[&n].depends_on {
            if slices.contains_key(d)
                && let Some(c) = visit(*d, trail, slices, marks)
            {
                return Some(c);
            }
        }
        trail.pop();
        marks.insert(n, 2);
        None
    }
    let mut marks = HashMap::new();
    slices
        .keys()
        .find_map(|n| visit(*n, &mut Vec::new(), slices, &mut marks))
}

/// Progress is record, not spec: a done slice that trips W-SIZE on its evidence cannot be split.
fn progress_len(text: &str) -> usize {
    let start = if text.starts_with("**Progress**:") {
        Some(0)
    } else {
        text.find("\n**Progress**:").map(|i| i + 1)
    };
    start
        .and_then(|s| text[s..].find("\n**Slice**:"))
        .map_or(0, |j| j + 1)
}

fn id_number(id: &str) -> Option<(&str, u32)> {
    let (prefix, n) = id.rsplit_once('-')?;
    Some((prefix, n.parse().ok()?))
}

fn log_rules(dir: &str, nnn: &str, f: &Feature, st: Option<&State>, r: &mut Report) {
    let log_rel = format!("{dir}/{}", log::FILE);
    let (entries, skipped) = match log::entries(&f.dir) {
        Ok(x) => x,
        Err(e) => {
            r.err("E-LOG-PARSE", format!("{log_rel}: {e}"));
            return;
        }
    };
    for (n, e) in &skipped {
        r.err("E-LOG-PARSE", format!("{log_rel}:{n}: {e}"));
    }
    let mut ids = HashSet::new();
    let mut last: HashMap<&str, u32> = HashMap::new();
    for e in &entries {
        let Some(id) = e.id.as_deref() else { continue };
        if !ids.insert(id) {
            r.err(
                "E-LOG-DUPLICATE",
                format!("{log_rel} has two {id} entries — ids are never reused"),
            );
        }
        if let Some((prefix, n)) = id_number(id) {
            if let Some(prev) = last.get(prefix)
                && n < *prev
            {
                r.err(
                    "E-LOG-ORDER",
                    format!("{log_rel}: {id} comes after {prefix}-{prev} — ids only increase"),
                );
            }
            last.insert(prefix, n);
        }
    }
    for e in entries
        .iter()
        .filter(|e| matches!(e.kind, LogKind::Answer | LogKind::Review))
    {
        let kind = if e.kind == LogKind::Answer {
            "an answer"
        } else {
            "a review"
        };
        match e.reference.as_deref() {
            None => r.err("E-LOG-REF", format!("{log_rel}: {kind} with no ref")),
            Some(id) if !ids.contains(id) => r.err(
                "E-LOG-REF",
                format!("{log_rel}: {kind} points at {id}, which the log does not hold"),
            ),
            Some(_) => {}
        }
    }

    let Some(s) = st else { return };
    let is = |n: u32, status: &str| s.slices.get(&n).is_some_and(|sl| sl.status == status);
    let open = log::select(&entries, log::Filter::Open);
    for (n, _) in s.slices.iter().filter(|(_, sl)| sl.status == "blocked") {
        if !open.iter().any(|b| b.slice == Some(*n)) {
            r.warn(
                "W-BLOCKED-NO-ENTRY",
                format!(
                    "PRD-{nnn} slice-{n:02} is blocked but {log_rel} has no open block naming it — record why with `factory docs log add {nnn} --kind block --slice {n}`, or `factory docs set {nnn} slices.{n}.status=building`"
                ),
            );
        }
    }
    for c in entries.iter().filter(|e| e.kind == LogKind::Checkpoint) {
        if let Some(n) = c.slice
            && !is(n, "done")
        {
            r.warn(
                "W-CP-STALE",
                format!(
                    "{log_rel} {} says it verified through slice-{n:02}, which is not done",
                    c.id.as_deref().unwrap_or("a checkpoint")
                ),
            );
        }
    }
    // A phase-only block (polish, pr, ci) names no slice and never trips this.
    for b in &open {
        if let Some(n) = b.slice
            && !is(n, "blocked")
        {
            let id = b.id.as_deref().unwrap_or("a block");
            r.warn(
                "W-BLOCK-STALE",
                format!(
                    "{log_rel} {id} is open but slice-{n:02} is not blocked — answer it (`factory docs log add {nnn} --kind answer --ref {id}`), or `factory docs set {nnn} slices.{n}.status=blocked`"
                ),
            );
        }
    }
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Bytes, not text: a tracked `.pyc` holds home paths too. `docs/archive/` is frozen, so a hit there cannot be fixed.
fn home_paths(root: &Path, r: &mut Report) {
    if git(root, &["rev-parse", "--is-inside-work-tree"])
        .as_deref()
        .map(str::trim)
        != Some("true")
    {
        return;
    }
    let Some(tracked) = git(root, &["ls-files", "-z", "--", "docs"]) else {
        return;
    };
    for rel in tracked.split('\0').filter(|p| !p.is_empty()) {
        if rel.starts_with("docs/archive/") {
            continue;
        }
        let Ok(bytes) = fs::read(root.join(rel)) else {
            continue;
        };
        if let Some(at) = crate::paths::find_home_path(&bytes) {
            let line = bytes[..at].iter().filter(|b| **b == b'\n').count() + 1;
            r.warn(
                "W-HOME-PATH",
                format!("{rel}:{line} holds a home path — write it as `~/…`"),
            );
        }
    }
}

/// A move in (`R`, `A`, `??`) is how archiving works; only an `M` in either column is an error.
fn archive_frozen(root: &Path, r: &mut Report) {
    if git(root, &["rev-parse", "--is-inside-work-tree"])
        .as_deref()
        .map(str::trim)
        != Some("true")
    {
        return;
    }
    // Untrimmed: porcelain's first column is a significant space.
    let Some(dirty) = git(root, &["status", "--porcelain", "--", "docs/archive"]) else {
        return;
    };
    for line in dirty.split('\n').filter(|l| l.len() > 3) {
        let b = line.as_bytes();
        if b[0] != b'M' && b[1] != b'M' {
            continue;
        }
        r.err(
            "E-ARCHIVE-MODIFIED",
            format!(
                "{} is modified — docs/archive/** is frozen; move folders, never edit inside",
                &line[3..]
            ),
        );
    }
}

fn walk_md(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            walk_md(&p, out);
        } else if p.extension().is_some_and(|e| e == "md") {
            out.push(p);
        }
    }
}

/// The archive is skipped: a link that went stale when a folder moved in is the rule working.
fn links(root: &Path, r: &mut Report) {
    let docs = root.join("docs");
    let archive = docs.join("archive");
    let mut files = Vec::new();
    walk_md(&docs, &mut files);
    for file in files.iter().filter(|f| !f.starts_with(&archive)) {
        let Ok(text) = fs::read_to_string(file) else {
            continue;
        };
        let base = file.parent().unwrap_or(root);
        for href in hrefs(&text) {
            if href.starts_with("http:")
                || href.starts_with("https:")
                || href.starts_with("mailto:")
                || href.starts_with('#')
            {
                continue;
            }
            let target = href.split('#').next().unwrap_or_default();
            if target.is_empty() || normalize(&base.join(target)).exists() {
                continue;
            }
            r.err(
                "E-LINK-BROKEN",
                format!(
                    "{} → {href} does not resolve",
                    file.strip_prefix(root).unwrap_or(file).display()
                ),
            );
        }
    }
}

/// The hrefs of `\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)`, in order.
fn hrefs(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = text[at..].find('[').map(|i| at + i) {
        match link_at(text, i) {
            Some((href, end)) => {
                out.push(href);
                at = end;
            }
            None => at = i + 1,
        }
    }
    out
}

fn link_at(text: &str, i: usize) -> Option<(&str, usize)> {
    let close = i + text[i..].find(']')?;
    let rest = text[close + 1..].strip_prefix('(')?;
    let start = close + 2;
    let len = rest
        .find(|c: char| c == ')' || c.is_whitespace())
        .unwrap_or(rest.len());
    if len == 0 {
        return None;
    }
    let href = &rest[..len];
    let after = &rest[len..];
    if after.starts_with(')') {
        return Some((href, start + len + 1));
    }
    let trimmed = after.trim_start();
    let title = trimmed.strip_prefix('"')?;
    let q = title.find('"')?;
    let tail = title[q + 1..].strip_prefix(')')?;
    Some((href, text.len() - tail.len()))
}

/// Lexical, so `a/missing/../b` is `a/b` without touching the filesystem.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::scaffold;

    fn git_ok(root: &Path, args: &[&str]) {
        let out = Command::new("git")
            .args(["-c", "user.name=T", "-c", "user.email=t@example.invalid"])
            .args(args)
            .current_dir(root)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn tree() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        git_ok(root, &["init", "-q", "-b", "main"]);
        let mut sink = Vec::new();
        scaffold::init(root, &mut sink).unwrap();
        fs::write(
            root.join("docs/project.toml"),
            "[docs]\nframework = 3\ngate = \"./gate.sh\"\n",
        )
        .unwrap();
        scaffold::new(root, "fixture", Some("Fixture"), &mut sink).unwrap();
        for (s, dep) in [("a", None), ("b", None), ("c", Some("2"))] {
            scaffold::slice(root, "001", s, dep, None, &mut sink).unwrap();
        }
        state::set(&feat(root), &[("status", "ready")]).unwrap();
        d
    }

    fn feat(root: &Path) -> PathBuf {
        root.join("docs/features/001-fixture")
    }

    fn edit_state(root: &Path, f: impl FnOnce(&mut State)) {
        let dir = feat(root);
        let mut s = state::read(&dir).unwrap().unwrap();
        f(&mut s);
        fs::write(state::path(&dir), state::render(&s)).unwrap();
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    fn append(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        let old = fs::read_to_string(&p).unwrap_or_default();
        fs::write(p, old + text).unwrap();
    }

    fn codes(root: &Path) -> Vec<String> {
        run(root).codes().into_iter().map(String::from).collect()
    }

    fn drop_summary(dir: &Path) {
        let p = dir.join("prd.md");
        let text = fs::read_to_string(&p).unwrap();
        let mut out = Vec::new();
        let mut skipping = false;
        for l in text.lines() {
            if l == "summary: |" {
                skipping = true;
                continue;
            }
            if skipping && l.starts_with("  ") {
                continue;
            }
            skipping = false;
            out.push(l);
        }
        fs::write(p, out.join("\n") + "\n").unwrap();
    }

    #[test]
    fn the_fixture_is_clean() {
        let d = tree();
        let r = run(d.path());
        assert!(r.is_clean(), "{:?}", r.lines());
    }

    #[test]
    fn each_broken_file_yields_exactly_its_code() {
        type Break = fn(&Path);
        let cases: [(&str, Break); 8] = [
            ("E-FM-MISSING", |root| drop_summary(&feat(root))),
            ("E-STATE-INVALID", |root| {
                edit_state(root, |s| s.status = "later".into())
            }),
            ("E-SLICE-NO-STATE", |root| {
                write(
                    root,
                    "docs/features/001-fixture/slice-04-d.md",
                    "# Slice 04 — D\n",
                )
            }),
            ("E-STATE-NO-SLICE", |root| {
                edit_state(root, |s| {
                    s.slices.insert(5, SliceState::default());
                })
            }),
            ("E-DEP-MISSING", |root| {
                edit_state(root, |s| s.slices.get_mut(&1).unwrap().depends_on = vec![9])
            }),
            ("E-DEP-CYCLE", |root| {
                edit_state(root, |s| s.slices.get_mut(&2).unwrap().depends_on = vec![3])
            }),
            ("W-SIZE", |root| {
                append(root, "docs/project.toml", "prd_max_kb = 1\n")
            }),
            ("E-PROJECT-INVALID", |root| {
                append(root, "docs/project.toml", "lanes = \"two\"\n")
            }),
        ];
        for (code, f) in cases {
            let d = tree();
            f(d.path());
            assert_eq!(
                codes(d.path()),
                [code],
                "{code}: {:?}",
                run(d.path()).lines()
            );
        }
    }

    #[test]
    fn a_cycle_names_the_slices_walked() {
        let d = tree();
        edit_state(d.path(), |s| {
            s.slices.get_mut(&2).unwrap().depends_on = vec![3]
        });
        let r = run(d.path());
        assert!(
            r.errors[0].1.ends_with("cycle: 02 → 03 → 02"),
            "{:?}",
            r.errors
        );
    }

    #[test]
    fn the_progress_region_does_not_count_toward_the_slice_ceiling() {
        let d = tree();
        let p = feat(d.path()).join("slice-01-a.md");
        let text = fs::read_to_string(&p).unwrap();
        let filler = "x".repeat(40 * 1024);
        let text = text.replacen("**Progress**: ", &format!("**Progress**: {filler} "), 1);
        fs::write(&p, &text).unwrap();
        assert_eq!(codes(d.path()), Vec::<String>::new());
        fs::write(&p, text.replace("**Slice**:", "**Spec**:")).unwrap();
        assert_eq!(codes(d.path()), ["W-SIZE"]);
    }

    #[test]
    fn one_bad_file_does_not_hide_another_features_findings() {
        let d = tree();
        let root = d.path();
        scaffold::new(root, "second", None, &mut Vec::new()).unwrap();
        fs::write(feat(root).join("state.toml"), "status = \n").unwrap();
        drop_summary(&root.join("docs/features/002-second"));
        let mut c = codes(root);
        c.sort();
        assert_eq!(c, ["E-FM-MISSING", "E-STATE-INVALID"]);
    }

    #[test]
    fn links_skip_urls_fragments_titles_and_the_archive() {
        let d = tree();
        let root = d.path();
        let prd = "docs/features/001-fixture/prd.md";
        append(
            root,
            prd,
            "\n[x](https://e.com) [y](#qa-plan) [z](slice-01-a.md \"the first\") [w](slice-02-b.md#tasks)\n",
        );
        assert_eq!(codes(root), Vec::<String>::new());
        write(
            root,
            "docs/archive/005-old/prd.md",
            "---\nnumber: 5\nslug: old\nstatus: archived\n---\n[x](nowhere.md)\n",
        );
        assert_eq!(codes(root), Vec::<String>::new());
        append(root, prd, "[gone](slice-09-missing.md)\n");
        let r = run(root);
        assert_eq!(r.codes(), ["E-LINK-BROKEN"]);
        assert!(
            r.errors[0].1.contains(prd) && r.errors[0].1.contains("slice-09-missing.md"),
            "{:?}",
            r.errors
        );
    }

    #[test]
    fn the_archive_is_frozen_in_the_index_and_the_working_tree() {
        let d = tree();
        let root = d.path();
        let old = "docs/archive/005-old/prd.md";
        write(
            root,
            old,
            "---\nnumber: 5\nslug: old\nstatus: archived\n---\nold\n",
        );
        git_ok(root, &["add", "-A"]);
        git_ok(root, &["commit", "-q", "-m", "fixture"]);
        assert_eq!(codes(root), Vec::<String>::new());

        append(root, old, "edited\n");
        assert_eq!(codes(root), ["E-ARCHIVE-MODIFIED"], "unstaged");
        git_ok(root, &["add", old]);
        assert_eq!(codes(root), ["E-ARCHIVE-MODIFIED"], "staged");
        git_ok(root, &["checkout", "HEAD", "--", old]);

        write(
            root,
            "docs/features/007-moved/prd.md",
            "---\nnumber: 7\nslug: moved\n---\n",
        );
        write(
            root,
            "docs/features/007-moved/state.toml",
            "status = \"archived\"\n",
        );
        git_ok(root, &["add", "-A"]);
        git_ok(root, &["commit", "-q", "-m", "moved"]);
        git_ok(
            root,
            &["mv", "docs/features/007-moved", "docs/archive/007-moved"],
        );
        assert_eq!(codes(root), Vec::<String>::new(), "a move in");
    }

    #[test]
    fn a_tracked_doc_holding_a_home_path_warns_once_with_its_first_line() {
        let d = tree();
        let root = d.path();
        let slice = "docs/features/001-fixture/slice-01-a.md";
        let line = fs::read_to_string(root.join(slice))
            .unwrap()
            .lines()
            .count()
            + 1;
        append(root, slice, "the run dir: /home/x/a\nagain /home/x/b\n");
        append(
            root,
            "docs/features/001-fixture/slice-02-b.md",
            "scratch: /tmp/home/x/a\n",
        );
        write(
            root,
            "docs/archive/005-old/prd.md",
            "---\nnumber: 5\nslug: old\nstatus: archived\n---\n/home/x/a\n",
        );
        let home_paths = |root: &Path| -> Vec<String> {
            run(root)
                .warnings
                .into_iter()
                .filter(|(c, _)| c == "W-HOME-PATH")
                .map(|(_, m)| m)
                .collect()
        };
        assert_eq!(home_paths(root), Vec::<String>::new(), "untracked");
        git_ok(root, &["add", "-A"]);
        let hits = home_paths(root);
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert!(hits[0].starts_with(&format!("{slice}:{line} ")), "{hits:?}");
    }

    #[test]
    fn second_boards_and_legacy_dirs_warn() {
        let d = tree();
        let root = d.path();
        write(root, "docs/TODO.md", "- a\n");
        fs::create_dir(root.join("plans")).unwrap();
        let r = run(root);
        assert_eq!(r.codes(), ["W-SECOND-BOARD", "W-LEGACY-DOCS"]);
        assert!(r.warnings[0].1.contains("docs/ideas/"), "{:?}", r.warnings);
    }

    fn log(root: &Path, lines: &[&str]) {
        fs::write(feat(root).join(log::FILE), lines.join("\n") + "\n").unwrap();
    }

    const FIRST_DECISION: &str =
        r#"{"id":"AD-1","kind":"decision","ts":"2026-10-13T00:00:00Z","title":"a"}"#;
    const SECOND_DECISION: &str =
        r#"{"id":"AD-2","kind":"decision","ts":"2026-10-13T00:00:00Z","title":"b"}"#;

    #[test]
    fn the_log_rules() {
        let d = tree();
        let root = d.path();
        log(
            root,
            &[FIRST_DECISION, SECOND_DECISION, r#"{"id":"AD-3","ki"#],
        );
        let r = run(root);
        assert_eq!(r.codes(), ["E-LOG-PARSE"]);
        assert!(r.errors[0].1.contains("log.jsonl:3:"), "{:?}", r.errors);

        log(root, &[FIRST_DECISION, FIRST_DECISION]);
        assert_eq!(codes(root), ["E-LOG-DUPLICATE"]);

        let ask_out_of_order =
            r#"{"id":"AD-1","kind":"ask","ts":"2026-10-13T00:00:00Z","title":"q"}"#;
        log(root, &[SECOND_DECISION, ask_out_of_order]);
        assert_eq!(codes(root), ["E-LOG-ORDER"]);

        let block = r#"{"id":"BLOCK-1","kind":"block","ts":"2026-10-13T00:00:00Z","phase":"polish","title":"x"}"#;
        let answer = r#"{"kind":"answer","ts":"2026-10-13T00:00:00Z","ref":"BLOCK-1","title":"y"}"#;
        log(root, &[FIRST_DECISION, block, SECOND_DECISION, answer]);
        assert_eq!(codes(root), Vec::<String>::new());

        let stray_answer =
            r#"{"kind":"answer","ts":"2026-10-13T00:00:00Z","ref":"BLOCK-4","title":"y"}"#;
        log(root, &[FIRST_DECISION, stray_answer]);
        assert_eq!(codes(root), ["E-LOG-REF"]);
    }

    #[test]
    fn an_empty_summary_warns_only_when_ready() {
        let d = tree();
        let root = d.path();
        let p = feat(root).join("prd.md");
        drop_summary(&feat(root));
        let text = fs::read_to_string(&p).unwrap();
        fs::write(&p, text.replacen("author:", "summary: |\nauthor:", 1)).unwrap();
        assert_eq!(codes(root), ["W-SUMMARY-EMPTY"]);
        state::set(&feat(root), &[("status", "draft")]).unwrap();
        assert_eq!(codes(root), Vec::<String>::new());
    }

    #[test]
    fn the_lifecycle_warnings_read_state_and_log() {
        let d = tree();
        let root = d.path();
        let dir = feat(root);
        state::set(
            &dir,
            &[("status", "in-progress"), ("slices.2.status", "blocked")],
        )
        .unwrap();
        assert_eq!(codes(root), ["W-BLOCKED-NO-ENTRY"]);

        state::set(&dir, &[("slices.2.status", "pending")]).unwrap();
        let polish = r#"{"id":"BLOCK-1","kind":"block","ts":"2026-10-13T00:00:00Z","phase":"polish","title":"x"}"#;
        log(root, &[polish]);
        assert_eq!(codes(root), Vec::<String>::new());

        let stale =
            r#"{"id":"BLOCK-2","kind":"block","ts":"2026-10-13T00:00:00Z","slice":3,"title":"x"}"#;
        let cp = r#"{"id":"CP-1","kind":"checkpoint","ts":"2026-10-13T00:00:00Z","slice":1,"title":"cp"}"#;
        log(root, &[polish, stale, cp]);
        assert_eq!(codes(root), ["W-CP-STALE", "W-BLOCK-STALE"]);

        log(root, &[]);
        let all = ["slices.1.status", "slices.2.status", "slices.3.status"];
        let done: Vec<(&str, &str)> = all.iter().map(|k| (*k, "done")).collect();
        state::set(&dir, &done).unwrap();
        assert_eq!(codes(root), ["W-NOT-REVIEWED"]);
        state::set(&dir, &[("reviewed", "2026-10-13")]).unwrap();
        assert_eq!(codes(root), ["W-READY-TO-SHIP"]);
        state::set(
            &dir,
            &[("status", "shipped"), ("slices.1.status", "building")],
        )
        .unwrap();
        assert_eq!(codes(root), ["E-STATUS-INCONSISTENT"]);
        state::set(&dir, &[("status", "ready")]).unwrap();
        assert_eq!(codes(root), ["W-STATUS-BEHIND"]);
    }

    #[test]
    fn no_gate_warns() {
        let d = tree();
        fs::write(
            d.path().join("docs/project.toml"),
            "[docs]\nframework = 3\n",
        )
        .unwrap();
        assert_eq!(codes(d.path()), ["W-NO-GATE"]);
    }
}
