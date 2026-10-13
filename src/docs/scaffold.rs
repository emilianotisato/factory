//! `init`, `new` and `slice` on v3 (PRD-002 FR-15): the folders, the PRD and
//! slice scaffolds from `assets/docs/`, and the `state.toml` entries.
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use super::board::{self, Kind};
use super::ideas;
use super::project;
use super::state;
use crate::config::PROJECT_FILE;
use crate::error::io;
use crate::{Error, Result, clock};

pub const INDEX: &str = include_str!("../../assets/docs/index.md");
pub const PRD: &str = include_str!("../../assets/docs/prd.scaffold.md");
pub const SLICE: &str = include_str!("../../assets/docs/slice.scaffold.md");
/// `docs/ideas/README.md`: the inbox rules for one file per idea (FR-20).
pub const IDEAS_README: &str = include_str!("../../assets/docs/ideas-README.md");

/// `docs/.gitignore`: the feature locks never show in `git status`, so
/// preflight's clean-tree check and the driver's folder commits ignore them.
const GITIGNORE_LINE: &str = "docs.lock";

/// The UTC day, as `docs.mjs` `today()` prints it.
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

/// `docs init`: whatever of the v3 layout is missing, never overwriting.
pub fn init(root: &Path, out: &mut dyn Write) -> Result<()> {
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
    if !text.lines().any(|l| l.trim() == GITIGNORE_LINE) {
        let sep = if text.is_empty() || text.ends_with('\n') {
            ""
        } else {
            "\n"
        };
        fs::write(&ignore, format!("{text}{sep}{GITIGNORE_LINE}\n"))
            .map_err(io(format!("write {}", ignore.display())))?;
        made.push("docs/.gitignore".into());
    }
    if made.is_empty() {
        return say(out, "already initialised");
    }
    say(out, &format!("created {}", made.join(", ")))?;
    say(
        out,
        "next: declare the verify gate under [docs] in docs/project.toml (`gate = \"<cmd>\"`), or run /gate",
    )
}

/// `allocateNumber` (`docs.mjs:1251`): the scan of `features/` and
/// `archive/`, raised to the reservation in `<git-common-dir>/docs-next-number`,
/// which it bumps, so two worktrees of one repo never claim the same number.
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

/// `my-slug` → `My Slug`, as `docs.mjs` titles a missing `--title`.
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

/// `docs new <slug> [--title T]`: `prd.md` from the v3 scaffold and
/// `state.toml` with `status = "draft"`. No log: the first append makes it.
pub fn new(root: &Path, slug: &str, title: Option<&str>, out: &mut dyn Write) -> Result<()> {
    new_from(root, slug, title, None, out)
}

/// `new` with `--from-idea ID`: the PRD carries `from_idea` and the idea's
/// body, and the idea file goes. Nothing is committed, so the PRD's first
/// commit carries the deletion too.
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

/// The scaffold with `from_idea` as its last frontmatter key and the idea's
/// body under `## From idea <id>` right after the H1, for the author to fold
/// in at the interview.
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

/// `docs slice NNN <subject> [--depends a,b] [--title T]`: the next slice
/// file, and its `[slices.N]` entry through `state::set`, under the lock.
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
