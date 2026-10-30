use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use super::board::{self, Kind};
use super::frontmatter;
use super::ideas;
use super::preflight;
use super::project;
use super::state;
use crate::config::PROJECT_FILE;
use crate::error::io;
use crate::skills;
use crate::{Error, Result, clock};

pub const INDEX: &str = include_str!("../../assets/docs/index.md");
pub const PRD: &str = include_str!("../../assets/docs/prd.scaffold.md");
pub const SLICE: &str = include_str!("../../assets/docs/slice.scaffold.md");
pub const IDEAS_README: &str = include_str!("../../assets/docs/ideas-README.md");
/// No backtick span in it may hold "verify": `docs gate` reads those as gate hints.
pub const CONVENTION: &str = include_str!("../../assets/docs/convention.md");

/// `docs.lock` stays out of `git status`, so the clean-tree check and folder commits ignore it;
/// `__pycache__/` keeps a script's bytecode, which holds home paths, out of the tracked docs.
const GITIGNORE_LINES: [&str; 2] = ["docs.lock", "__pycache__/"];

pub fn today() -> String {
    clock::now_rfc3339()[..10].to_string()
}

fn say(out: &mut dyn Write, text: &str) -> Result<()> {
    writeln!(out, "{text}").map_err(io("write stdout"))
}

fn refuse<T>(msg: String) -> Result<T> {
    Err(Error::Msg(msg))
}

fn write_new(path: &Path, text: &str) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io(format!("mkdir {}", dir.display())))?;
    }
    fs::write(path, text).map_err(io(format!("write {}", path.display())))
}

pub fn init(root: &Path, out: &mut dyn Write) -> Result<()> {
    // Rendered first, so a refusal leaves the repo untouched; a manifest or source checkout already has its skills.
    let rendered = if root.join(skills::MANIFEST).exists() || skills::source_checkout(root) {
        None
    } else {
        Some(skills::render(root)?)
    };
    let docs = root.join("docs");
    let mut made: Vec<String> = Vec::new();
    let toml = root.join(PROJECT_FILE);
    if !toml.exists() {
        write_new(&toml, "[docs]\nframework = 3\n")?;
        made.push(PROJECT_FILE.into());
    } else if project::load(root)?.is_none() {
        project::set(root, "framework", Some(&toml::Value::Integer(3)))?;
        made.push(format!("{PROJECT_FILE} [docs]"));
    }
    let index = docs.join("index.md");
    if !index.exists() {
        write_new(&index, INDEX)?;
        made.push("docs/index.md".into());
    }
    for d in ["features", "ideas", "archive"] {
        let p = docs.join(d);
        if !p.is_dir() {
            fs::create_dir_all(&p).map_err(io(format!("mkdir {}", p.display())))?;
            made.push(format!("docs/{d}/"));
        }
    }
    let readme = docs.join("ideas").join(ideas::README);
    if !readme.exists() {
        write_new(&readme, IDEAS_README)?;
        made.push(format!("{}/{}", ideas::DIR, ideas::README));
    }
    let ignore = docs.join(".gitignore");
    let text = fs::read_to_string(&ignore).unwrap_or_default();
    let missing: Vec<&str> = GITIGNORE_LINES
        .into_iter()
        .filter(|want| !text.lines().any(|l| l.trim() == *want))
        .collect();
    if !missing.is_empty() {
        let sep = if text.is_empty() || text.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        let add: String = missing.iter().map(|l| format!("{l}\n")).collect();
        fs::write(&ignore, format!("{text}{sep}{add}"))
            .map_err(io(format!("write {}", ignore.display())))?;
        made.push("docs/.gitignore".into());
    }
    let (convention, skipped) = convention(root)?;
    let say_skipped = |out: &mut dyn Write| -> Result<()> {
        for (name, why) in &skipped {
            say(out, &format!("convention block: skipped {name} ({why})"))?;
        }
        Ok(())
    };
    if made.is_empty() && rendered.is_none() && convention.is_empty() {
        say_skipped(out)?;
        return say(out, "already initialised");
    }
    let version = env!("CARGO_PKG_VERSION");
    let mut paths: Vec<String> = Vec::new();
    if !made.is_empty() {
        say(out, &format!("created {}", made.join(", ")))?;
        paths.push("docs".into());
    }
    if let Some(r) = &rendered {
        say(out, &r.summary())?;
        paths.extend(r.dirs().into_iter().map(String::from));
    }
    if !convention.is_empty() {
        say(out, &format!("convention block: {}", convention.join(", ")))?;
        paths.extend(convention);
    }
    say_skipped(out)?;
    let subject = match rendered {
        Some(_) => format!("chore: factory init (skills {version})"),
        None => "chore: factory init".to_string(),
    };
    say(
        out,
        &format!(
            "commit: git add {} && git commit -m \"{subject}\"",
            paths.join(" ")
        ),
    )?;
    say(
        out,
        "next, in order: the gate (factory docs gate set \"<cmd>\", or /gate), the harness (/harness), the UX rubric (/ux rubric)",
    )
}

/// A convention file left alone, and why.
type Skipped = (&'static str, &'static str);

/// Each file is re-read on its turn, so a `CLAUDE.md` linked to `AGENTS.md` gets the block once.
fn convention(root: &Path) -> Result<(Vec<String>, Vec<Skipped>)> {
    let heading = CONVENTION.lines().next().unwrap_or_default();
    let canon_root = fs::canonicalize(root).map_err(io(format!("resolve {}", root.display())))?;
    let has_heading = |text: &str| text.lines().any(|l| l.trim_end() == heading);
    let mut written: Vec<String> = Vec::new();
    let mut skipped = Vec::new();
    for name in ["CLAUDE.md", "AGENTS.md"] {
        let path = root.join(name);
        if fs::symlink_metadata(&path).is_ok() {
            match fs::canonicalize(&path) {
                Ok(p) if p.starts_with(&canon_root) => {}
                Ok(_) => {
                    skipped.push((name, "links outside the repo"));
                    continue;
                }
                Err(_) => {
                    skipped.push((name, "a dangling link"));
                    continue;
                }
            }
        }
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(io(format!("read {}", path.display()))(e)),
        };
        if has_heading(&text) {
            continue;
        }
        let sep = match text.as_str() {
            "" => "",
            t if t.ends_with('\n') => "\n",
            _ => "\n\n",
        };
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(io(format!("open {}", path.display())))?;
        f.write_all(format!("{sep}{CONVENTION}").as_bytes())
            .map_err(io(format!("write {}", path.display())))?;
        let shown = fs::canonicalize(&path)
            .ok()
            .and_then(|p| {
                p.strip_prefix(&canon_root)
                    .ok()
                    .map(|r| r.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| name.to_string());
        if !written.contains(&shown) {
            written.push(shown);
        }
    }
    // A `CLAUDE.md` link that dangled until `AGENTS.md` was written got the block: no skip.
    skipped.retain(|(name, _)| {
        !fs::canonicalize(root.join(name)).is_ok_and(|p| {
            p.starts_with(&canon_root) && fs::read_to_string(&p).is_ok_and(|t| has_heading(&t))
        })
    });
    Ok((written, skipped))
}

/// The reservation in `<git-common-dir>/docs-next-number` keeps two worktrees from claiming one number.
fn allocate(root: &Path, board: &board::Board) -> Result<(u32, bool)> {
    let computed = board.all().map(|f| f.number).max().unwrap_or(0) + 1;
    let common = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let Some(common) = common.filter(|c| !c.is_empty()) else {
        return Ok((computed, false));
    };
    let common = Path::new(&common);
    let file = if common.is_absolute() {
        common.join("docs-next-number")
    } else {
        root.join(common).join("docs-next-number")
    };
    let reserved = fs::read_to_string(&file)
        .ok()
        .and_then(|t| t.trim().parse::<u32>().ok())
        .unwrap_or(0);
    let number = computed.max(reserved);
    fs::write(&file, (number + 1).to_string()).map_err(io(format!("write {}", file.display())))?;
    Ok((number, true))
}

fn title_case(s: &str) -> String {
    let mut out = String::new();
    let mut prev_word = false;
    for c in s.replace('-', " ").chars() {
        let word = c.is_ascii_alphanumeric() || c == '_';
        out.push(if word && !prev_word {
            c.to_ascii_uppercase()
        } else {
            c
        });
        prev_word = word;
    }
    out
}

fn name_ok(what: &str, s: &str) -> Result<()> {
    if s.is_empty()
        || s.contains(['/', '\\'])
        || s.contains(char::is_whitespace)
        || s.starts_with('-')
    {
        return refuse(format!("{what} \"{s}\" must be one kebab-case word"));
    }
    Ok(())
}

/// `git config user.name` as a frontmatter value that reads back as the name: bare, else quoted.
fn author(root: &Path) -> Option<String> {
    let name = preflight::git_line(root, &["config", "user.name"]).filter(|n| !n.is_empty())?;
    [name.clone(), format!("\"{name}\"")].into_iter().find(|v| {
        frontmatter::parse(&format!("---\nauthor: {v}\n---\n")).str("author") == Some(name.as_str())
    })
}

/// No `log.jsonl`: the first append makes it.
pub fn new(root: &Path, slug: &str, title: Option<&str>, out: &mut dyn Write) -> Result<()> {
    new_from(root, slug, title, None, out)
}

/// The idea file is deleted but not committed, so the PRD's first commit carries the deletion.
pub fn new_from(
    root: &Path,
    slug: &str,
    title: Option<&str>,
    from_idea: Option<&str>,
    out: &mut dyn Write,
) -> Result<()> {
    name_ok("slug", slug)?;
    // Before the number is allocated: a bad id leaves nothing behind.
    let idea = from_idea.map(|id| ideas::open(root, id)).transpose()?;
    let Some(project) = project::load(root)? else {
        return refuse(format!(
            "no [docs] in {PROJECT_FILE} — run `factory docs init` first"
        ));
    };
    let board = board::scan(root)?;
    let (number, shared) = allocate(root, &board)?;
    let name = format!("{number:03}-{slug}");
    let dir = root.join("docs/features").join(&name);
    if dir.exists() {
        return refuse(format!("docs/features/{name} already exists"));
    }
    let title = match (title, &idea) {
        (Some(t), _) => t.to_string(),
        (None, Some((_, _, i))) => i.title.clone(),
        (None, None) => title_case(slug),
    };
    let gate = match &project.gate {
        Some(g) => format!("`{g}` *(declared in project.toml)*"),
        None => "_(none declared — run /gate before slicing)_".into(),
    };
    let mut body = PRD
        .replace("{{NNN}}", &format!("{number:03}"))
        .replace("{{SLUG}}", slug)
        .replace("{{TITLE}}", &title)
        .replace("{{DATE}}", &today())
        .replace("{{GATE}}", &gate);
    if let Some(author) = author(root) {
        body = body.replacen("\nauthor: _(name)_\n", &format!("\nauthor: {author}\n"), 1);
    }
    if let Some((id, _, i)) = &idea {
        body = with_idea(&body, id, &i.body);
    }
    write_new(&dir.join("prd.md"), &body)?;
    state::set(&dir, &[("status", "draft")])?;
    say(
        out,
        &format!("created docs/features/{name}/prd.md, state.toml"),
    )?;
    if let Some((_, rel, _)) = &idea {
        let p = root.join(rel);
        fs::remove_file(&p).map_err(io(format!("delete {}", p.display())))?;
        say(out, &format!("deleted {rel}: commit it with the PRD"))?;
    }
    say(
        out,
        &format!(
            "PRD-{number:03}{}",
            if shared {
                ""
            } else {
                " (no git — number not reserved across worktrees)"
            }
        ),
    )?;
    say(
        out,
        &match &project.gate {
            Some(g) => format!("gate: {g}"),
            None => "⚠ no verify gate declared — mention /gate in the hand-off".into(),
        },
    )
}

fn with_idea(prd: &str, id: &str, body: &str) -> String {
    let mut out = String::with_capacity(prd.len() + body.len() + 64);
    let mut fences = 0;
    let mut placed = false;
    for line in prd.split_inclusive('\n') {
        if line == "---\n" {
            fences += 1;
            if fences == 2 {
                out.push_str(&format!("from_idea: \"{id}\"\n"));
            }
        }
        out.push_str(line);
        if fences >= 2 && !placed && line.starts_with("# ") {
            placed = true;
            out.push_str(&format!("\n## From idea {id}\n\n{body}"));
            if !body.is_empty() && !body.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    out
}

pub fn slice(
    root: &Path,
    feature: &str,
    subject: &str,
    depends: Option<&str>,
    title: Option<&str>,
    out: &mut dyn Write,
) -> Result<()> {
    name_ok("subject", subject)?;
    let board = board::scan(root)?;
    let f = board.resolve(feature)?;
    if f.kind == Kind::Archive {
        return refuse(format!(
            "{} is archived, and the archive is frozen",
            f.dir_name
        ));
    }
    let st = match &f.state {
        Ok(Some(s)) => s,
        Ok(None) => return refuse(format!("{} has no state.toml", f.dir_name)),
        Err(e) => return refuse(e.clone()),
    };
    let exists = |n: u32| st.slices.contains_key(&n) || f.slices.iter().any(|s| s.number == n);
    let number = st
        .slices
        .keys()
        .copied()
        .chain(f.slices.iter().map(|s| s.number))
        .max()
        .unwrap_or(0)
        + 1;
    let mut deps: Vec<u32> = Vec::new();
    for t in depends
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        match t.parse::<u32>() {
            Ok(d) if exists(d) => deps.push(d),
            _ => return refuse(format!("slice {t} does not exist in {}", f.dir_name)),
        }
    }
    let file = f.dir.join(format!("slice-{number:02}-{subject}.md"));
    if file.exists() {
        return refuse(format!("{} already exists", file.display()));
    }
    let project = board.project.clone();
    let (gate, inner) = project.map_or((None, None), |p| (p.gate, p.inner));
    let verify = match (&inner, &gate) {
        (Some(c), _) | (None, Some(c)) => format!("`{c}`"),
        (None, None) => "_(no gate declared — run /gate, or declare `gate` under [docs] in docs/project.toml, then name it here)_".into(),
    };
    let prose = if deps.is_empty() {
        "—".to_string()
    } else {
        deps.iter()
            .map(|d| format!("{d:02}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let body = SLICE
        .replace("{{NN}}", &format!("{number:02}"))
        .replace(
            "{{TITLE}}",
            &title.map_or_else(|| title_case(subject), str::to_string),
        )
        .replace("{{DATE}}", &today())
        .replace("{{DEPENDS_PROSE}}", &prose)
        .replace("{{GATE}}", &verify);
    write_new(&file, &body)?;
    let deps_value = deps
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let (k_status, k_deps) = (
        format!("slices.{number}.status"),
        format!("slices.{number}.depends_on"),
    );
    if let Err(e) = state::set(&f.dir, &[(&k_status, "pending"), (&k_deps, &deps_value)]) {
        let _ = fs::remove_file(&file);
        return Err(e);
    }
    say(
        out,
        &format!(
            "created docs/features/{}/slice-{number:02}-{subject}.md",
            f.dir_name
        ),
    )?;
    if gate.is_none() {
        say(
            out,
            "⚠ no verify gate declared — the Verify task holds a placeholder",
        )?;
    } else if inner.is_none() {
        say(
            out,
            "⚠ no inner loop declared — the Verify task runs the full gate on every slice",
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_title_cased_slugs() {
        assert_eq!(title_case("docs-port"), "Docs Port");
        assert_eq!(title_case("a"), "A");
    }

    #[test]
    fn the_prd_scaffold_frontmatter_has_the_seven_keys() {
        let fm = super::super::frontmatter::parse(
            &PRD.replace("{{NNN}}", "001")
                .replace("{{SLUG}}", "x")
                .replace("{{TITLE}}", "X: y")
                .replace("{{DATE}}", "2026-10-13"),
        );
        let keys: Vec<&str> = fm
            .data
            .as_ref()
            .unwrap()
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(
            keys,
            [
                "number", "slug", "title", "type", "created", "author", "summary"
            ]
        );
        assert_eq!(fm.str("title"), Some("X: y"));
    }
}
