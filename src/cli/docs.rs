//! `factory docs <verb…>` (PRD-002 FR-15, FR-17): routes on the repo's
//! format before it parses anything. A v3 repo (`docs/project.toml` with
//! `[docs] framework = 3`) runs the verb natively; a v2 repo (`docs/index.md`)
//! hands every argument to `docs.mjs` by `exec`, so stdin, stdout, stderr and
//! the exit code are the tool's own. Later slices add one arm each to
//! [`Verb`] and [`dispatch`].
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{CommandFactory, Parser, Subcommand};

use crate::docs::{Docs, board, project, scaffold, state};
use crate::error::{io, json};
use crate::{Error, Result, paths};

/// The feature-docs board: `docs/project.toml`, one `state.toml` per feature
#[derive(Parser)]
#[command(name = "factory docs", disable_help_subcommand = true)]
pub struct DocsCli {
    #[command(subcommand)]
    verb: Verb,
}

#[derive(Subcommand)]
enum Verb {
    /// Scaffold docs/project.toml, docs/index.md, features/, ideas/, archive/
    Init,
    /// Allocate the next PRD number and scaffold the feature
    New {
        slug: String,
        #[arg(long)]
        title: Option<String>,
    },
    /// Add the next slice to a feature
    Slice {
        feature: String,
        subject: String,
        /// Slices it waits for: `1,2`
        #[arg(long)]
        depends: Option<String>,
        #[arg(long)]
        title: Option<String>,
    },
    /// The board; with a feature, its state for resuming a run
    Status {
        feature: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Write a feature's state.toml: `status=ready`, `slices.3.status=building`,
    /// `slices.3.lane=` (empty removes)
    Set {
        feature: String,
        #[arg(required = true)]
        pairs: Vec<String>,
    },
}

/// Verbs v2 had and v3 drops, each with where its job went.
const REMOVED: [(&str, &str); 3] = [
    ("render", "the board is computed, run `factory docs status`"),
    ("log merge", "the log is appended by `factory docs log add`"),
    ("vendor", "install the `factory` binary"),
];

pub fn docs(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    match route(args, out) {
        Err(Error::Msg(m)) => {
            writeln!(err, "{m}").map_err(io("write stderr"))?;
            Ok(2)
        }
        r => r,
    }
}

fn route(args: &[String], out: &mut dyn Write) -> Result<i32> {
    let root = paths::repo_root(Path::new(".")).unwrap_or_else(|_| PathBuf::from("."));
    let verb = args.first().map(String::as_str);
    if matches!(verb, None | Some("-h" | "--help")) {
        let help = DocsCli::command().render_help().to_string();
        out.write_all(help.as_bytes()).map_err(io("write stdout"))?;
        return Ok(0);
    }
    // `migrate` and `doctor` run natively in both formats (slices 03, 07).
    if matches!(verb, Some("migrate" | "doctor")) || project::is_v3(&root) {
        return native(&root, args, out);
    }
    if root.join("docs/index.md").is_file() {
        let tool = Docs::locate(&root)?.tool().to_path_buf();
        let e = Command::new("node").arg(&tool).args(args).exec();
        return Err(Error::Msg(format!(
            "could not exec node {}: {e}",
            tool.display()
        )));
    }
    // A repo with no board at all has nothing to route on; `init` makes one.
    if verb == Some("init") {
        return native(&root, args, out);
    }
    Err(Error::Msg(format!(
        "not on the framework: {} has neither docs/project.toml with [docs] framework = 3 nor docs/index.md — run `factory docs init`",
        root.display()
    )))
}

fn native(root: &Path, args: &[String], out: &mut dyn Write) -> Result<i32> {
    let joined = args
        .iter()
        .take(2)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    if let Some((verb, pointer)) = REMOVED.iter().find(|(v, _)| *v == args[0] || *v == joined) {
        return Err(Error::Msg(format!("\"{verb}\" removed in v3: {pointer}")));
    }
    let argv = std::iter::once("factory docs".to_string()).chain(args.iter().cloned());
    let cli = match DocsCli::try_parse_from(argv) {
        Ok(c) => c,
        Err(e) if e.use_stderr() => {
            return Err(Error::Msg(e.render().to_string().trim_end().into()));
        }
        Err(e) => {
            out.write_all(e.render().to_string().as_bytes())
                .map_err(io("write stdout"))?;
            return Ok(0);
        }
    };
    dispatch(root, cli.verb, out)?;
    Ok(0)
}

fn dispatch(root: &Path, verb: Verb, out: &mut dyn Write) -> Result<()> {
    match verb {
        Verb::Init => scaffold::init(root, out),
        Verb::New { slug, title } => scaffold::new(root, &slug, title.as_deref(), out),
        Verb::Slice {
            feature,
            subject,
            depends,
            title,
        } => scaffold::slice(
            root,
            &feature,
            &subject,
            depends.as_deref(),
            title.as_deref(),
            out,
        ),
        Verb::Status { feature, json } => status(root, feature.as_deref(), json, out),
        Verb::Set { feature, pairs } => set(root, &feature, &pairs, out),
    }
}

fn print(out: &mut dyn Write, text: &str) -> Result<()> {
    writeln!(out, "{text}").map_err(io("write stdout"))
}

fn status(root: &Path, feature: Option<&str>, as_json: bool, out: &mut dyn Write) -> Result<()> {
    let board = board::scan(root)?;
    let Some(feature) = feature else {
        if as_json {
            let v = board::summary_json(&board);
            return print(
                out,
                &serde_json::to_string_pretty(&v).map_err(json("serialize board"))?,
            );
        }
        return board::print_summary(&board, out);
    };
    let s = board::feature_state(&board, board.resolve(feature)?)?;
    if as_json {
        return print(
            out,
            &serde_json::to_string_pretty(&s).map_err(json("serialize status"))?,
        );
    }
    let pad2 = |n: &u32| format!("{n:02}");
    let mut lines = vec![format!(
        "PRD-{:03} {} · {}{}",
        s.number,
        s.slug,
        s.status.as_deref().unwrap_or("?"),
        s.reviewed
            .as_str()
            .map(|d| format!(" · reviewed {d}"))
            .unwrap_or_default()
    )];
    lines.push(format!(
        "qa plan   {}{}",
        if s.qa_plan { "yes" } else { "none" },
        s.harness_line
            .as_deref()
            .map(|h| format!(" · Harness: {h}"))
            .unwrap_or_default()
    ));
    let slices: Vec<String> = s
        .slices
        .iter()
        .map(|x| {
            let st = x.status.as_deref().unwrap_or("?");
            let deps = if st == "pending" && !x.depends_on.is_empty() {
                format!(
                    " (deps {})",
                    x.depends_on.iter().map(pad2).collect::<Vec<_>>().join(", ")
                )
            } else {
                String::new()
            };
            format!("{:02} {st}{deps}", x.number)
        })
        .collect();
    lines.push(format!(
        "slices    {}",
        if slices.is_empty() {
            "not sliced".into()
        } else {
            slices.join(" · ")
        }
    ));
    lines.push(format!(
        "next      {}",
        match &s.next {
            Some(n) => format!(
                "slice-{:02} ({})",
                n.number,
                n.status.as_deref().unwrap_or("?")
            ),
            None if s.all_done => "every slice done".into(),
            None if s.sliced => "nothing runnable".into(),
            None => "—".into(),
        }
    ));
    if s.lanes > 1 && s.sliced {
        let free: Vec<String> = s.free_lanes.iter().map(u32::to_string).collect();
        lines.push(format!(
            "lanes     {} · free {} · spawnable now {}",
            s.lanes,
            if free.is_empty() {
                "none".into()
            } else {
                free.join(", ")
            },
            s.spawnable
        ));
    }
    if !s.open_opens.is_empty() {
        let opens: Vec<String> = s.open_opens.iter().map(|n| format!("OPEN-{n}")).collect();
        lines.push(format!("opens     {}", opens.join(", ")));
    }
    lines.push(format!(
        "branch    {}",
        s.branch.as_deref().unwrap_or("(not a git repo)")
    ));
    print(out, &lines.join("\n"))
}

fn set(root: &Path, feature: &str, pairs: &[String], out: &mut dyn Write) -> Result<()> {
    let board = board::scan(root)?;
    let f = board.resolve(feature)?;
    if f.kind == board::Kind::Archive {
        return Err(Error::Msg(format!(
            "{} is archived, and the archive is frozen",
            f.dir_name
        )));
    }
    let mut kv = Vec::new();
    for p in pairs {
        let Some(pair) = p.split_once('=') else {
            return Err(Error::Msg(format!("\"{p}\" is not key=value")));
        };
        kv.push(pair);
    }
    state::set(&f.dir, &kv)?;
    print(
        out,
        &format!("docs/features/{}/state.toml updated", f.dir_name),
    )
}
