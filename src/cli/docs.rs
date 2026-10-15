//! A v3 repo runs each verb natively; a v2 repo execs `docs.mjs`, so its streams and exit code are the tool's own.
use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{CommandFactory, Parser, Subcommand};

use crate::docs::doctor::ToolOutput;
use crate::docs::{board, check, doctor, ideas, log, project, scaffold, state};
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
    /// Scaffold the docs layout, the skill family and the convention block (`factory init`)
    Init,
    /// Allocate the next PRD number and scaffold the feature
    New {
        slug: String,
        #[arg(long)]
        title: Option<String>,
        /// Build on this idea: its body goes in the PRD, its file is deleted
        #[arg(long)]
        from_idea: Option<String>,
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
    /// A feature's retros row in the metrics store; `--set score=4` sets its score
    Retro {
        feature: String,
        #[arg(long)]
        set: Option<String>,
    },
    /// Validate the board; exits 1 on any error
    Check,
    /// Which framework version the repo is on, and what `check` would say
    Doctor,
    /// The feature's append-only log.jsonl
    Log {
        #[command(subcommand)]
        action: LogVerb,
    },
    /// The open ideas, one file each under docs/ideas/
    Idea {
        #[command(subcommand)]
        action: IdeaVerb,
    },
    /// The verify gate; `set <cmd>` declares it, `--inner` the fast subset
    Gate {
        args: Vec<String>,
        #[arg(long)]
        inner: bool,
        #[arg(long)]
        candidates: bool,
    },
    /// Can a run start on this feature? Exits 0 pass, 1 refused, 2 asks
    Preflight {
        feature: String,
        #[arg(long)]
        json: bool,
    },
    /// Mark a feature shipped once every slice is done
    Ship { feature: String },
    /// Move a feature whose slices are all done into docs/archive/
    Archive { feature: String },
    /// Print shipped features' retro lessons and the ideas they spawned; changes nothing
    Harvest { features: Vec<String> },
    /// The lanes; `set N --env K=V` writes the lane files
    Lanes {
        args: Vec<String>,
        #[arg(long)]
        env: Option<String>,
        #[arg(long)]
        setup: Option<String>,
    },
    /// Give a feature a new number, outside docs/archive/
    Renumber { from: String, to: String },
    /// Move a v2 repo's docs to v3 in one uncommitted diff; `--dry` writes nothing
    Migrate {
        #[arg(long)]
        dry: bool,
    },
    /// Install the pre-commit hook, or print it when core.hooksPath is set
    Hook {
        /// Point this repo's core.hooksPath at .git/hooks first
        #[arg(long)]
        here: bool,
    },
}

#[derive(Subcommand)]
enum IdeaVerb {
    /// Write one idea file and print its id
    Add {
        #[arg(long)]
        title: String,
        /// open, or owed when someone outside the repo owes an answer
        #[arg(long, default_value = "open")]
        state: String,
        /// Where it came from: `owner`, `PRD-NNN polish`
        #[arg(long)]
        origin: Option<String>,
        /// The event that should make someone look again
        #[arg(long)]
        retrigger: Option<String>,
        /// Only `high`; absent is normal
        #[arg(long)]
        priority: Option<String>,
        /// The markdown body: a path, or `-` for stdin
        #[arg(long)]
        body_file: String,
    },
    /// The open ideas; `--json` is what the cross-repo scanner reads
    List {
        #[arg(long)]
        json: bool,
    },
    /// Delete one idea and commit that one path, the reason as its body
    Drop {
        id: String,
        #[arg(long)]
        because: String,
    },
}

#[derive(Subcommand)]
enum LogVerb {
    /// Append one entry under the feature's docs.lock and print its id
    Add {
        feature: String,
        #[arg(long, value_enum)]
        kind: log::Kind,
        #[arg(long)]
        title: String,
        /// The slice it belongs to; for a checkpoint, the last one it covers
        #[arg(long)]
        slice: Option<u32>,
        /// Decisions only
        #[arg(long, value_enum)]
        flag: Option<log::Flag>,
        /// The id an answer or a review points at
        #[arg(long = "ref")]
        reference: Option<String>,
        /// Reviews only
        #[arg(long, value_enum)]
        verdict: Option<log::Verdict>,
        /// preflight, slicing, checkpoint, polish, pr or ci
        #[arg(long)]
        phase: Option<String>,
        /// Checkpoints only: every slice it covered, comma-separated
        #[arg(long, value_delimiter = ',')]
        covers: Vec<u32>,
        /// The markdown body: a path, or `-` for stdin
        #[arg(long)]
        body_file: Option<String>,
    },
    /// Render the log in the agent-log.md shape
    Show {
        feature: String,
        /// Only `decide` decisions no review answers
        #[arg(long)]
        decide: bool,
        /// Only blocks no answer resolves
        #[arg(long, conflicts_with = "decide")]
        open: bool,
        /// The entries as read, a JSON array in file order
        #[arg(long)]
        json: bool,
    },
}

const REMOVED: [(&str, &str); 3] = [
    ("render", "the board is computed, run `factory docs status`"),
    ("log merge", "the log is appended by `factory docs log add`"),
    ("vendor", "install the `factory` binary"),
];

pub fn docs(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    match route(args, out, err) {
        Err(Error::Msg(m)) => {
            writeln!(err, "{m}").map_err(io("write stderr"))?;
            Ok(2)
        }
        r => r,
    }
}

fn route(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
    let root = paths::repo_root(Path::new(".")).unwrap_or_else(|_| PathBuf::from("."));
    let root = board_root(&root, args).unwrap_or(root);
    let verb = args.first().map(String::as_str);
    if matches!(verb, None | Some("-h" | "--help")) {
        let help = DocsCli::command().render_help().to_string();
        out.write_all(help.as_bytes()).map_err(io("write stdout"))?;
        return Ok(0);
    }
    if matches!(verb, Some("migrate" | "doctor")) || project::is_v3(&root) {
        return native(&root, args, out, err);
    }
    if root.join("docs/index.md").is_file() {
        let tool = legacy_tool(&root)?;
        let e = node(&tool).args(args).exec();
        return Err(Error::Msg(format!(
            "could not exec node {}: {e}",
            tool.display()
        )));
    }
    if verb == Some("init") {
        return native(&root, args, out, err);
    }
    Err(Error::Msg(format!(
        "not on the framework: {} has neither docs/project.toml with [docs] framework = 3 nor docs/index.md — run `factory docs init`",
        root.display()
    )))
}

/// The same-repo check keeps a fixture's `docs set`, run by `cargo test` inside a worker, off that worker's board.
fn board_root(root: &Path, args: &[String]) -> Option<PathBuf> {
    let verb: Vec<&str> = args.iter().take(2).map(String::as_str).collect();
    if !matches!(
        verb.as_slice(),
        ["set" | "retro" | "status", ..] | ["log", "add"]
    ) {
        return None;
    }
    let board = std::env::var_os(crate::worker::BOARD_ENV).filter(|v| !v.is_empty())?;
    let board = paths::repo_root(Path::new(&board)).ok()?;
    let same = paths::git_common_dir(&board)? == paths::git_common_dir(root)?;
    same.then_some(board)
}

fn native(root: &Path, args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Result<i32> {
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
    match cli.verb {
        Verb::Retro { feature, set } => retro(root, &feature, set.as_deref(), out, err),
        Verb::Check => {
            let report = check::run(root);
            if report.is_clean() {
                print(out, "docs check: clean")?;
            } else {
                report.print(err)?;
            }
            Ok(i32::from(!report.errors.is_empty()))
        }
        Verb::Doctor => {
            let text = doctor::run(root, || legacy_check(root));
            out.write_all(text.as_bytes()).map_err(io("write stdout"))?;
            Ok(0)
        }
        Verb::Gate {
            args,
            inner,
            candidates,
        } => crate::docs::gate::run(root, &args, inner, candidates, out, err),
        Verb::Preflight { feature, json } => crate::docs::preflight::cli(root, &feature, json, out),
        verb => {
            dispatch(root, verb, out, err)?;
            Ok(0)
        }
    }
}

fn legacy_tool(root: &Path) -> Result<PathBuf> {
    let vendored = root.join(".docs/bin/docs.mjs");
    let global = paths::home().join(".claude/skills/feature-docs/bin/docs.mjs");
    if vendored.is_file() {
        Ok(vendored)
    } else if global.is_file() {
        Ok(global)
    } else {
        Err(Error::Msg(format!(
            "no docs.mjs: neither {} nor {} exists",
            vendored.display(),
            global.display()
        )))
    }
}

fn node(tool: &Path) -> Command {
    let mut c = Command::new("node");
    c.arg(tool);
    c
}

/// Never `--fix`: doctor never writes.
fn legacy_check(root: &Path) -> Result<ToolOutput> {
    let tool = legacy_tool(root)?;
    let o = node(&tool)
        .arg("check")
        .current_dir(root)
        .output()
        .map_err(io(format!("run node {} check", tool.display())))?;
    Ok(ToolOutput {
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        code: o.status.code().unwrap_or(-1),
    })
}

fn dispatch(root: &Path, verb: Verb, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
    match verb {
        Verb::Init => scaffold::init(root, out),
        Verb::New {
            slug,
            title,
            from_idea,
        } => scaffold::new_from(root, &slug, title.as_deref(), from_idea.as_deref(), out),
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
        Verb::Retro { .. }
        | Verb::Check
        | Verb::Doctor
        | Verb::Gate { .. }
        | Verb::Preflight { .. } => {
            unreachable!("`native` runs the verbs with their own exit code")
        }
        Verb::Ship { feature } => crate::docs::ship::ship(root, &feature, out),
        Verb::Archive { feature } => crate::docs::ship::archive(root, &feature, out),
        Verb::Harvest { features } => crate::docs::harvest::harvest(root, &features, out),
        Verb::Lanes { args, env, setup } => {
            crate::docs::lanes::run(root, &args, env.as_deref(), setup.as_deref(), out)
        }
        Verb::Renumber { from, to } => crate::docs::renumber::run(root, &from, &to, out),
        Verb::Migrate { dry } => migrate(root, dry, out),
        Verb::Hook { here } => print(out, &crate::docs::hook::install(root, here)?.text()),
        Verb::Log { action } => match action {
            LogVerb::Add {
                feature,
                kind,
                title,
                slice,
                flag,
                reference,
                verdict,
                phase,
                covers,
                body_file,
            } => {
                let body = match body_file.as_deref() {
                    None => String::new(),
                    Some(p) => read_body(p)?,
                };
                let entry = log::Entry {
                    slice,
                    flag,
                    reference,
                    verdict,
                    phase,
                    covers,
                    ..log::Entry::new(kind, &title, &body)
                };
                let dir = live_dir(root, &feature)?;
                match log::add(&dir, entry)? {
                    Some(id) => print(out, &id),
                    None => Ok(()),
                }
            }
            LogVerb::Show {
                feature,
                decide,
                open,
                json: as_json,
            } => {
                let board = board::scan(root)?;
                let f = board.resolve(&feature)?;
                let (entries, skipped) = log::entries(&f.dir)?;
                for (n, e) in &skipped {
                    writeln!(err, "{}/{}:{n}: skipped, {e}", f.dir_name, log::FILE)
                        .map_err(io("write stderr"))?;
                }
                let filter = if decide {
                    log::Filter::Decide
                } else if open {
                    log::Filter::Open
                } else {
                    log::Filter::All
                };
                if as_json {
                    let v = log::select(&entries, filter);
                    return print(
                        out,
                        &serde_json::to_string_pretty(&v).map_err(json("serialize log"))?,
                    );
                }
                out.write_all(log::render(&entries, filter, f.number).as_bytes())
                    .map_err(io("write stdout"))
            }
        },
        Verb::Idea { action } => idea(root, action, out, err),
    }
}

/// Plan first so "already migrated" beats "not clean"; `--dry` never opens the store, which would create it.
fn migrate(root: &Path, dry: bool, out: &mut dyn Write) -> Result<()> {
    use crate::docs::migrate;
    let plan = migrate::plan(root)?;
    if !dry {
        migrate::clean(root)?;
    }
    for row in &plan.rows {
        print(out, &row.to_string())?;
    }
    if dry {
        return print(out, "dry run: nothing written");
    }
    let db = crate::db::Db::open(&crate::run::Run::db_path(&paths::data_dir()))?;
    migrate::run(root, &plan, &db)?;
    print(
        out,
        &format!(
            "migrated: {} files, {} rows; review with git diff -- docs; undo with git checkout -- docs && git clean -fd docs",
            plan.files.len(),
            plan.retros.len()
        ),
    )
}

fn read_body(spec: &str) -> Result<String> {
    if spec == "-" {
        let mut b = String::new();
        std::io::stdin()
            .read_to_string(&mut b)
            .map_err(io("read stdin"))?;
        return Ok(b);
    }
    std::fs::read_to_string(spec).map_err(io(format!("read {spec}")))
}

fn idea(root: &Path, action: IdeaVerb, out: &mut dyn Write, err: &mut dyn Write) -> Result<()> {
    match action {
        IdeaVerb::Add {
            title,
            state,
            origin,
            retrigger,
            priority,
            body_file,
        } => {
            let idea = ideas::Idea {
                title,
                state,
                added: scaffold::today(),
                origin,
                retrigger,
                priority,
                body: read_body(&body_file)?,
            };
            print(out, &ideas::add(root, &idea)?)
        }
        IdeaVerb::List { json: as_json } => {
            let (rows, skipped) = ideas::list(root);
            for (path, why) in &skipped {
                writeln!(err, "! {path}: {why}").map_err(io("write stderr"))?;
            }
            if as_json {
                return print(
                    out,
                    &serde_json::to_string_pretty(&rows).map_err(json("serialize ideas"))?,
                );
            }
            if rows.is_empty() {
                return print(out, "no open ideas");
            }
            let lines: Vec<String> = rows
                .iter()
                .map(|r| {
                    let high = if r.priority.is_some() { "  [high]" } else { "" };
                    format!(
                        "{}  {}  {}{high}\n    {}",
                        r.id, r.state, r.title, r.first_sentence
                    )
                })
                .collect();
            print(out, &lines.join("\n"))
        }
        IdeaVerb::Drop { id, because } => {
            let id = ideas::drop(root, &id, &because)?;
            print(
                out,
                &format!("dropped {id}: docs/ideas/{id}.md deleted and committed"),
            )
        }
    }
}

fn retro(
    root: &Path,
    feature: &str,
    set: Option<&str>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<i32> {
    let row = match set {
        Some(pair) => Some(crate::docs::retro::set_score(root, feature, pair)?),
        None => crate::docs::retro::show(root, feature)?,
    };
    match row {
        Some(r) => {
            print(out, &crate::docs::retro::lines(&r).join("\n"))?;
            Ok(0)
        }
        None => {
            let n = board::scan(root)?.resolve(feature)?.number;
            writeln!(err, "no retro row for PRD-{n:03}").map_err(io("write stderr"))?;
            Ok(1)
        }
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

fn live_dir(root: &Path, feature: &str) -> Result<PathBuf> {
    let board = board::scan(root)?;
    let f = board.resolve(feature)?;
    if f.kind == board::Kind::Archive {
        return Err(Error::Msg(format!(
            "{} is archived, and the archive is frozen",
            f.dir_name
        )));
    }
    Ok(f.dir.clone())
}

fn set(root: &Path, feature: &str, pairs: &[String], out: &mut dyn Write) -> Result<()> {
    let dir = live_dir(root, feature)?;
    let mut kv = Vec::new();
    for p in pairs {
        let Some(pair) = p.split_once('=') else {
            return Err(Error::Msg(format!("\"{p}\" is not key=value")));
        };
        kv.push(pair);
    }
    state::set(&dir, &kv)?;
    let name = dir.file_name().unwrap_or_default().to_string_lossy();
    print(out, &format!("docs/features/{name}/state.toml updated"))
}
