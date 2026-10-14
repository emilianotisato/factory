//! `factory docs preflight NNN` (PRD-002 FR-15): can a run start, or resume,
//! on this feature unattended? A port of `docs.mjs:1671-1771` reading
//! `project.toml`, `state.toml` and the log instead of frontmatter. Four
//! buckets: `refuse` (the run cannot start), `ask` (a one-time fix the owner
//! answers first), `judge` (the agent decides alone), `ok`. Exit 0 pass,
//! 1 refused, 2 asks pending.
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use super::board::{self, Feature};
use super::{Branch, FeatureRef, Preflight, Slice, check, gate, log, project};
use crate::error::{io, json};
use crate::{Result, skills};

/// `git <args>` in `root`, stdout untrimmed; `None` on any failure.
pub fn git(root: &Path, args: &[&str]) -> Option<String> {
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

fn git_line(root: &Path, args: &[&str]) -> Option<String> {
    git(root, args).map(|s| s.trim().to_string())
}

/// `defaultBranch` (`docs.mjs:1649-1654`): `origin/HEAD`, else `main` or
/// `master` when the branch exists.
pub fn default_branch(root: &Path) -> Option<String> {
    if let Some(h) = git_line(
        root,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    ) {
        return Some(h.strip_prefix("origin/").unwrap_or(&h).to_string());
    }
    ["main", "master"]
        .into_iter()
        .find(|b| {
            git(
                root,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("refs/heads/{b}"),
                ],
            )
            .is_some()
        })
        .map(str::to_string)
}

/// `parallelWidth` (`docs.mjs:1093-1104`): the widest wave of a greedy
/// schedule over `depends_on`. A dependency on a slice that is not listed
/// counts as met; a cycle stops the count.
pub fn parallel_width(slices: &[Slice]) -> usize {
    let mut pending: Vec<(u32, &[u32])> = slices
        .iter()
        .map(|s| (s.number, s.depends_on.as_slice()))
        .collect();
    let mut done: Vec<u32> = Vec::new();
    let mut widest = 0;
    while !pending.is_empty() {
        let listed: Vec<u32> = pending.iter().map(|(n, _)| *n).collect();
        let ready: Vec<u32> = pending
            .iter()
            .filter(|(_, deps)| deps.iter().all(|d| done.contains(d) || !listed.contains(d)))
            .map(|(n, _)| *n)
            .collect();
        if ready.is_empty() {
            break;
        }
        widest = widest.max(ready.len());
        pending.retain(|(n, _)| !ready.contains(n));
        done.extend(ready);
    }
    widest
}

fn find<'a>(features: &'a [Feature], reference: &str) -> Option<&'a Feature> {
    let n = (!reference.is_empty() && reference.bytes().all(|b| b.is_ascii_digit()))
        .then(|| reference.parse::<u32>().ok())
        .flatten();
    features.iter().find(|f| match n {
        Some(n) => f.number == n,
        None => f.slug == reference || f.dir_name == reference,
    })
}

/// The verdict and its exit code.
pub fn run(root: &Path, reference: &str) -> Result<(Preflight, i32)> {
    let mut refuse: Vec<String> = Vec::new();
    let mut ask: Vec<String> = Vec::new();
    let mut judge: Vec<String> = Vec::new();
    let mut ok: Vec<String> = Vec::new();

    let p = project::load(root).ok().flatten();
    match &p {
        Some(p) if p.framework == 3 => {
            let report = check::run(root);
            if report.errors.is_empty() {
                ok.push("docs check: no errors".into());
            } else {
                let mut codes: Vec<&str> = Vec::new();
                for (c, _) in &report.errors {
                    if !codes.contains(&c.as_str()) {
                        codes.push(c);
                    }
                }
                refuse.push(format!(
                    "docs check has {} error(s) ({}) — fix drift first",
                    report.errors.len(),
                    codes.join(", ")
                ));
            }
            // FR-3: a stale skill is the owner's to judge, never a refusal.
            let v = skills::verdict(root);
            let into = if v.is_ok() { &mut ok } else { &mut judge };
            into.extend(v.lines().into_iter().map(|l| format!("skills {l}")));
        }
        _ => refuse.push(
            "not on the framework: docs/project.toml has no [docs] framework = 3 — `factory docs doctor`".into(),
        ),
    }

    let b = board::scan(root)?;
    let f = find(&b.live, reference);
    let mut slices: Vec<Slice> = Vec::new();
    match f {
        None => refuse.push(match find(&b.archived, reference) {
            Some(a) => format!("PRD-{:03} is archived — nothing to deliver", a.number),
            None => format!("no live feature matching \"{reference}\""),
        }),
        Some(f) => {
            slices = f.board_slices();
            feature(
                root,
                f,
                p.as_ref(),
                &slices,
                &mut refuse,
                &mut ask,
                &mut judge,
                &mut ok,
            )?;
        }
    }

    // Either naming style counts: projects use both, and refusing the run's
    // own branch cost a real run.
    let mut branch = Branch {
        feature: f.map(|f| format!("feat/{}", f.slug)),
        alt: f.map(|f| format!("feat/{:03}-{}", f.number, f.slug)),
        ..Branch::default()
    };
    if git_line(root, &["rev-parse", "--is-inside-work-tree"]).as_deref() != Some("true") {
        refuse.push("not a git repository".into());
    } else {
        let dirty = git(root, &["status", "--porcelain"]).unwrap_or_default();
        let count = dirty.split('\n').filter(|l| !l.is_empty()).count();
        let building: Vec<String> = slices
            .iter()
            .filter(|s| s.status.as_deref() == Some("building"))
            .map(|s| format!("{:02}", s.number))
            .collect();
        // A builder that died mid-slice leaves its work uncommitted; refusing
        // the tree would make the run unresumable, so the run judges whether
        // the diff is that builder's.
        if count > 0 && !building.is_empty() {
            judge.push(format!(
                "working tree not clean ({count} path(s)) while slice {} is building — likely a builder that died mid-slice; hand its diff to a fresh builder, and stop if any path is not that slice's",
                building.join(", ")
            ));
        } else if count > 0 {
            refuse.push(format!(
                "working tree not clean ({count} path(s)) — commit or stash first"
            ));
        } else {
            ok.push("working tree clean".into());
        }
        match git_line(root, &["remote", "get-url", "origin"]).filter(|o| !o.is_empty()) {
            Some(o) => ok.push(format!("origin {o}")),
            None => refuse.push("no origin remote — the run ends in a pull request".into()),
        }
        branch.current = git_line(root, &["rev-parse", "--abbrev-ref", "HEAD"]);
        branch.default = default_branch(root);
        for name in [branch.feature.clone(), branch.alt.clone()]
            .into_iter()
            .flatten()
        {
            let r = format!("refs/heads/{name}");
            if git(root, &["rev-parse", "--verify", "--quiet", &r]).is_some() {
                branch.feature = Some(name);
                branch.feature_exists = true;
                break;
            }
        }
        let on_feature = branch.current.is_some()
            && (branch.current == branch.feature || branch.current == branch.alt);
        if branch.feature.is_some() && on_feature {
            branch.feature = branch.current.clone();
            ok.push(format!(
                "on {} — resuming",
                branch.current.as_deref().unwrap_or("")
            ));
        } else if branch.current == branch.default {
            let d = branch.default.as_deref().unwrap_or("");
            let fb = branch.feature.as_deref().unwrap_or("");
            judge.push(if branch.feature_exists {
                format!("on {d} — switch to existing {fb}")
            } else {
                format!("on {d} — create {fb}")
            });
        } else if let Some(fb) = &branch.feature {
            refuse.push(format!(
                "on {}, which is neither {} nor {fb} (or {}) — the owner decides which branch this feature lives on",
                branch.current.as_deref().unwrap_or("?"),
                branch.default.as_deref().unwrap_or("the default branch"),
                branch.alt.as_deref().unwrap_or("")
            ));
        }
    }

    // `pass` is "nothing mechanical refuses this run"; asks exit 2, which the
    // driver puts to the owner.
    let code = if !refuse.is_empty() {
        1
    } else if !ask.is_empty() {
        2
    } else {
        0
    };
    let pf = Preflight {
        feature: f.map(|f| FeatureRef {
            number: f.number,
            slug: f.slug.clone(),
            status: f.status(),
        }),
        pass: refuse.is_empty(),
        refuse,
        ask,
        judge,
        ok,
        branch,
    };
    Ok((pf, code))
}

/// The feature half (`docs.mjs:1692-1727`).
#[allow(clippy::too_many_arguments)]
fn feature(
    root: &Path,
    f: &Feature,
    p: Option<&project::Project>,
    slices: &[Slice],
    refuse: &mut Vec<String>,
    ask: &mut Vec<String>,
    judge: &mut Vec<String>,
    ok: &mut Vec<String>,
) -> Result<()> {
    let nnn = format!("{:03}", f.number);
    let st = f.status().unwrap_or_default();
    let (entries, _) = log::entries(&f.dir)?;
    // A run ships in its closeout commit and still has the PR, CI and the
    // owner's review ahead: shipped with a written log is a run to resume.
    if st == "shipped" && !entries.is_empty() {
        ok.push("status shipped by the run — resumes at the PR, CI or owner review".into());
    } else if !matches!(st.as_str(), "draft" | "ready" | "in-progress") {
        refuse.push(format!("PRD-{nnn} is {st} — nothing to deliver"));
    } else {
        ok.push(format!(
            "status {st}{}",
            if st == "draft" {
                " — becomes ready when this preflight passes"
            } else {
                ""
            }
        ));
    }
    // `reviewed` records /polish over the code. Set before any code exists,
    // it makes a resumed run skip polish.
    if let Some(r) = f.state().and_then(|s| s.reviewed.as_deref())
        && matches!(st.as_str(), "draft" | "ready")
    {
        refuse.push(format!(
            "reviewed {r} is set but nothing is built — it marks the code's /polish pass, not a PRD review; clear it with `factory docs set {nnn} reviewed=`"
        ));
    }
    let text = f.prd.as_ref().map(|(_, t)| t.as_str()).unwrap_or("");
    if board::has_qa_plan(text) {
        ok.push("QA plan present".into());
    } else {
        refuse.push("no QA plan section — every FR needs a proof row first (/prd)".into());
    }
    let g = gate::status(root, p.and_then(|p| p.gate.as_deref()));
    match g.status.as_str() {
        "none" => refuse.push(
            "no verify gate declared — /gate builds one (an interview, never unattended)".into(),
        ),
        "missing" => refuse.push(format!("gate {}", gate::line(&g))),
        _ => ok.push(format!("gate {}", gate::line(&g))),
    }
    match project::seed(root) {
        Ok(Some(_)) => {}
        Ok(None) => refuse.push(crate::lanes::NO_SEED.into()),
        Err(e) => refuse.push(format!("[lanes] seed unreadable: {e}")),
    }
    let open: Vec<String> = log::select(&entries, log::Filter::Open)
        .iter()
        .filter_map(|e| e.id.clone())
        .collect();
    if !open.is_empty() {
        refuse.push(format!(
            "open blocker(s): {} — answer each in the log (`factory docs log add {nnn} --kind answer --ref BLOCK-n`) before resuming",
            open.join(", ")
        ));
    }
    let opens = board::open_opens(text);
    if !opens.is_empty() {
        let tags: Vec<String> = opens.iter().map(|n| format!("OPEN-{n}")).collect();
        judge.push(format!(
            "unstruck {} — a load-bearing one blocks delivery; a deliberately parked one may stay",
            tags.join(", ")
        ));
    }
    let h = gate::harness(root);
    judge.push(format!(
        "harness on disk: {} · QA plan says: {}",
        if h.present {
            h.detail.as_deref().unwrap_or("")
        } else {
            "none"
        },
        board::harness_line(text)
            .as_deref()
            .unwrap_or("(no Harness line)")
    ));
    if !gate::rubric(root) {
        judge.push("no docs/ux-rubric.md — a UI diff gets the mechanical UX checks only".into());
    }
    let inner = p.and_then(|p| p.inner.as_deref());
    if g.status != "none" && inner.is_none() {
        ask.push("no inner loop declared — every slice runs the full gate. Declare the fast subset once (`factory docs gate set --inner \"<cmd>\"`) and every slice of every future run skips the slow tiers.".into());
    }
    // Lanes let two agents run the suite at once, each with its own
    // database, ports and scratch paths.
    let lanes = p.map_or(1, |p| p.lanes);
    let width = parallel_width(slices);
    if width >= 2 && lanes < 2 {
        ask.push(format!(
            "{} slices, up to {width} of them runnable at once by depends_on, but `lanes = {lanes}` in docs/project.toml — they will build one after another. Defining lanes is a one-time interview per project (`/lanes`).",
            slices.len()
        ));
    } else if width >= 2 {
        ok.push(format!("lanes {lanes} · graph allows {width} at once"));
    }
    // Only a measured false asks. Absent means nobody has looked.
    match p.and_then(|p| p.gate_parallel) {
        Some(false) => ask.push("the gate cannot run parallel workers (`gate_parallel = false`) — every gate run pays full serial wall-clock, and a run pays it dozens of times. One pass at making the suite worker-safe cuts every future run.".into()),
        None if g.status != "none" => judge.push("gate_parallel not measured — `/gate audit` times the suite serial and parallel and writes the answer to docs/project.toml".into()),
        _ => {}
    }
    Ok(())
}

/// The verb: the JSON with `--json`, else the `✓ ✗ ! ?` lines.
pub fn cli(root: &Path, reference: &str, as_json: bool, out: &mut dyn Write) -> Result<i32> {
    let (pf, code) = run(root, reference)?;
    let mut text = String::new();
    if as_json {
        text = serde_json::to_string_pretty(&pf).map_err(json("serialize preflight"))?;
        text.push('\n');
    } else {
        text.push_str(&match &pf.feature {
            Some(f) => format!("preflight PRD-{:03} {}\n", f.number, f.slug),
            None => format!("preflight {reference}\n"),
        });
        for (mark, lines) in [
            ("✓", &pf.ok),
            ("✗", &pf.refuse),
            ("!", &pf.ask),
            ("?", &pf.judge),
        ] {
            for l in lines {
                text.push_str(&format!("  {mark} {l}\n"));
            }
        }
        text.push_str(&if !pf.pass {
            format!("\nrefused: {} blocker(s)\n", pf.refuse.len())
        } else if !pf.ask.is_empty() {
            format!(
                "\n{} ask(s) for the owner — answer them before starting; each is a one-time fix the run would otherwise pay for repeatedly\n",
                pf.ask.len()
            )
        } else {
            "\nready — the ? lines are yours to judge before starting\n".into()
        });
    }
    out.write_all(text.as_bytes()).map_err(io("write stdout"))?;
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slice(number: u32, deps: &[u32]) -> Slice {
        Slice {
            number,
            depends_on: deps.to_vec(),
            ..Slice::default()
        }
    }

    #[test]
    fn the_width_is_the_widest_wave() {
        assert_eq!(parallel_width(&[]), 0);
        let fixture = [slice(1, &[]), slice(2, &[]), slice(3, &[2])];
        assert_eq!(parallel_width(&fixture), 2);
        let chain = [slice(1, &[]), slice(2, &[1]), slice(3, &[2])];
        assert_eq!(parallel_width(&chain), 1);
        let cycle = [slice(1, &[2]), slice(2, &[1])];
        assert_eq!(parallel_width(&cycle), 0);
        // A dependency on a slice nobody lists counts as met.
        assert_eq!(parallel_width(&[slice(1, &[9]), slice(2, &[])]), 2);
    }
}
