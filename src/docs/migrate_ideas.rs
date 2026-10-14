use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::ideas::{self, Idea};
use super::migrate::{self, Plan, PlanRow};
use super::scaffold::IDEAS_README;
use super::state::is_date;
use crate::{Error, Result};

pub const FILE: &str = "docs/ideas.md";

#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    /// `guess` names the rule when the placement is not certain.
    Open {
        owed: bool,
        high: bool,
        guess: Option<String>,
    },
    NotMigrated(&'static str),
    Dropped(&'static str),
}

/// A `- ` bullet with its continuation lines, or a stray line under a heading (then `Dropped`).
#[derive(Debug, Clone, PartialEq)]
pub struct Bullet {
    /// 1-based, the bullet's first line.
    pub line: usize,
    pub title: String,
    /// Continuation lines as written, ending in one `\n`, or empty.
    pub body: String,
    pub h2: Option<String>,
    pub h3: Option<String>,
    pub place: Place,
}

/// The fields a bullet's phrases give, and which rule gave `added`.
#[derive(Debug, Clone, PartialEq)]
pub struct Fields {
    pub added: String,
    pub origin: Option<String>,
    pub retrigger: Option<String>,
    pub from: &'static str,
}

fn is_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let h = line.len() - line.trim_start_matches('#').len();
    let text = line[h..].strip_prefix(' ')?;
    (1..=6).contains(&h).then_some((h, text.trim()))
}

fn bold(text: &str) -> Option<(String, &str)> {
    let inner = text.strip_prefix("**")?;
    let (end, _) = inner.match_indices("**").find(|(i, _)| *i > 0)?;
    let title = inner[..end].trim();
    let title = title.strip_suffix('.').unwrap_or(title).trim_end();
    let rest = &inner[end + 2..];
    let rest = [" — ", " – ", " - ", ": "]
        .iter()
        .find_map(|sep| rest.strip_prefix(sep))
        .or_else(|| rest.strip_prefix(' '))
        .unwrap_or(rest);
    Some((title.to_string(), rest))
}

fn placement(h2: &str, h3: Option<&str>) -> Place {
    let lower = h2.to_lowercase();
    let parked = h3.is_some_and(|h| h.to_lowercase().starts_with("parked"));
    let open = |owed, guess| Place::Open {
        owed,
        high: !owed && lower.contains("high priority"),
        guess,
    };
    if lower.starts_with("open") {
        open(false, None)
    } else if lower.starts_with("owed") {
        open(true, None)
    } else if lower.starts_with("graduated") {
        if parked {
            open(false, Some("Parked below Graduated".into()))
        } else {
            Place::NotMigrated("under Graduated")
        }
    } else {
        open(false, Some(format!("unknown heading ## {h2}")))
    }
}

fn bullet(line: usize, first: &str, cont: &[&str], h2: Option<&str>, h3: Option<&str>) -> Bullet {
    let whole = first.trim();
    let (title, rest, has_bold) = match bold(first) {
        Some((t, rest)) => (t, rest, true),
        None => {
            let t = whole.find(". ").map_or(whole, |i| &whole[..i]);
            (t.strip_suffix('.').unwrap_or(t).to_string(), first, false)
        }
    };
    let parts: Vec<&str> = std::iter::once(rest)
        .filter(|r| !r.is_empty())
        .chain(cont.iter().copied())
        .collect();
    let body = if parts.is_empty() {
        String::new()
    } else {
        parts.join("\n") + "\n"
    };
    let place = match h2 {
        None => Place::Dropped("before any ## heading"),
        Some(_) if whole.starts_with("_(") && whole.ends_with(")_") => {
            Place::Dropped("placeholder")
        }
        Some(h2) => match placement(h2, h3) {
            Place::Open { owed, high, guess } if !has_bold => Place::Open {
                owed,
                high,
                guess: Some(match guess {
                    Some(g) => format!("{g}; no bold title"),
                    None => "no bold title".into(),
                }),
            },
            p => p,
        },
    };
    Bullet {
        line,
        title,
        body,
        h2: h2.map(str::to_string),
        h3: h3.map(str::to_string),
        place,
    }
}

/// A blank line ends a bullet, not a section; stray lines before the first `##` drop silently.
pub fn parse(text: &str) -> Vec<Bullet> {
    struct Open<'a> {
        line: usize,
        first: &'a str,
        cont: Vec<&'a str>,
    }
    let mut out = Vec::new();
    let mut h2: Option<&str> = None;
    let mut h3: Option<&str> = None;
    let mut fenced = false;
    let mut cur: Option<Open> = None;
    fn flush(out: &mut Vec<Bullet>, cur: &mut Option<Open>, h2: Option<&str>, h3: Option<&str>) {
        if let Some(o) = cur.take() {
            out.push(bullet(o.line, o.first, &o.cont, h2, h3));
        }
    }
    let stray =
        |out: &mut Vec<Bullet>, i: usize, line: &str, h2: Option<&str>, h3: Option<&str>| {
            if h2.is_some() && !line.trim().is_empty() {
                out.push(Bullet {
                    line: i + 1,
                    title: line.trim().to_string(),
                    body: String::new(),
                    h2: h2.map(str::to_string),
                    h3: h3.map(str::to_string),
                    place: Place::Dropped("in a fence under no bullet"),
                });
            }
        };
    for (i, line) in text.lines().enumerate() {
        if is_fence(line) {
            // An indented fence belongs to the bullet above; one at the margin ends it.
            match cur.as_mut() {
                Some(o) if line.starts_with(char::is_whitespace) => o.cont.push(line),
                _ => {
                    flush(&mut out, &mut cur, h2, h3);
                    stray(&mut out, i, line, h2, h3);
                }
            }
            fenced = !fenced;
            continue;
        }
        if fenced {
            match cur.as_mut() {
                Some(o) => o.cont.push(line),
                None => stray(&mut out, i, line, h2, h3),
            }
            continue;
        }
        if let Some((level, text)) = heading(line) {
            flush(&mut out, &mut cur, h2, h3);
            match level {
                1 => (h2, h3) = (None, None),
                2 => (h2, h3) = (Some(text), None),
                3 => h3 = Some(text),
                _ => {}
            }
            continue;
        }
        if line.trim().is_empty() {
            flush(&mut out, &mut cur, h2, h3);
        } else if let Some(first) = line.strip_prefix("- ") {
            flush(&mut out, &mut cur, h2, h3);
            cur = Some(Open {
                line: i + 1,
                first,
                cont: Vec::new(),
            });
        } else if let Some(o) = cur.as_mut() {
            o.cont.push(line);
        } else if h2.is_some() {
            out.push(Bullet {
                line: i + 1,
                title: line.trim().to_string(),
                body: String::new(),
                h2: h2.map(str::to_string),
                h3: h3.map(str::to_string),
                place: Place::Dropped("not a bullet"),
            });
        }
    }
    flush(&mut out, &mut cur, h2, h3);
    out
}

/// `Parked from (.+?) \((\d{4}-\d{2}-\d{2})\)`, within one line.
fn parked(text: &str) -> Option<(String, String)> {
    text.match_indices("Parked from ").find_map(|(i, p)| {
        let rest = &text[i + p.len()..];
        let rest = &rest[..rest.find('\n').unwrap_or(rest.len())];
        rest.match_indices(" (").find_map(|(j, _)| {
            let d = rest.get(j + 2..j + 12)?;
            (j > 0 && is_date(d) && rest[j + 12..].starts_with(')')).then(|| {
                let origin = &rest[..j];
                let origin = origin.strip_prefix("the ").unwrap_or(origin);
                (origin.to_string(), d.to_string())
            })
        })
    })
}

fn added(text: &str) -> Option<String> {
    text.match_indices("Added ").find_map(|(i, p)| {
        text.get(i + p.len()..i + p.len() + 10)
            .filter(|d| is_date(d))
            .map(str::to_string)
    })
}

fn retrigger(text: &str) -> Option<String> {
    let i = text.find("Re-trigger:")?;
    let rest = &text[i + "Re-trigger:".len()..];
    let rest = rest[..rest.find('\n').unwrap_or(rest.len())].trim();
    let s = match [". ", "? ", "! "].iter().filter_map(|p| rest.find(p)).min() {
        Some(j) => &rest[..=j],
        None => rest,
    };
    (!s.is_empty()).then(|| s.to_string())
}

/// `blame` dates the bullet's first line; the body keeps every phrase a field was read from.
pub fn fields(b: &Bullet, blame: &str) -> Fields {
    let text = format!("{}\n{}", b.title, b.body);
    let own = parked(&text);
    let head = b.h3.as_deref().and_then(parked);
    let (added, from) = if let Some((_, d)) = &own {
        (d.clone(), "Parked")
    } else if let Some(d) = added(&text) {
        (d, "Added")
    } else if let Some((_, d)) = &head {
        (d.clone(), "the heading")
    } else {
        (blame.to_string(), "git blame")
    };
    Fields {
        added,
        origin: own.or(head).map(|(o, _)| o),
        retrigger: retrigger(&text),
        from,
    }
}

fn clip(s: &str) -> String {
    match s.char_indices().nth(60) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}

fn refuse<T>(line: usize, title: &str, why: impl std::fmt::Display) -> Result<T> {
    Err(Error::Msg(format!(
        "{FILE}:{line} \"{title}\": {why} — edit the line, commit, and run migrate again"
    )))
}

/// Guess rows go to the front of the whole plan, so the owner reads the risky rows first.
pub fn plan(root: &Path, plan: &mut Plan) -> Result<()> {
    let path = root.join(FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(crate::error::io(format!("read {}", path.display())))?;
    let blame = migrate::blame_lines(root, &path)?;
    let mut taken: HashSet<String> = ideas::files(root)
        .into_iter()
        .filter_map(|(rel, _)| {
            rel.strip_suffix(".md")
                .map(|s| s[ideas::DIR.len() + 1..].to_string())
        })
        .collect();
    let mut guesses = Vec::new();
    let mut rows = Vec::new();
    for b in parse(&text) {
        let (owed, high, guess) = match b.place {
            Place::Dropped(why) => {
                let line = text.lines().nth(b.line - 1).unwrap_or_default();
                rows.push(PlanRow::new(
                    "drop",
                    format!("{FILE}:{} {} ({why})", b.line, clip(line.trim())),
                ));
                continue;
            }
            Place::NotMigrated(rule) => {
                rows.push(PlanRow::new(
                    "idea",
                    format!("not migrated \"{}\" line {}: {rule}", b.title, b.line),
                ));
                continue;
            }
            Place::Open {
                owed,
                high,
                ref guess,
            } => (owed, high, guess.clone()),
        };
        let (date, committed) = blame
            .get(b.line - 1)
            .cloned()
            .unwrap_or_else(|| (crate::clock::today(), false));
        let f = fields(&b, &date);
        if f.from == "git blame" && !committed {
            rows.push(PlanRow::new(
                "warn",
                format!("{FILE}: added is today: line {} is not committed", b.line),
            ));
        }
        let slug = ideas::slug(&b.title).or_else(|e| refuse(b.line, &b.title, e))?;
        let base = format!("{}-{slug}", f.added);
        let id = (1..)
            .map(|n| match n {
                1 => base.clone(),
                n => format!("{base}-{n}"),
            })
            .find(|id| !taken.contains(id))
            .unwrap_or(base);
        taken.insert(id.clone());
        let state = if owed { "owed" } else { "open" };
        let idea = Idea {
            title: b.title.clone(),
            state: state.into(),
            added: f.added,
            origin: f.origin,
            retrigger: f.retrigger,
            priority: high.then(|| "high".to_string()),
            body: b.body.clone(),
        };
        let rendered = ideas::render(&idea).or_else(|e| refuse(b.line, &b.title, e))?;
        plan.files.push((
            PathBuf::from(ideas::DIR).join(format!("{id}.md")),
            Some(rendered),
        ));
        let rule = guess
            .clone()
            .unwrap_or_else(|| format!("under ## {}", b.h2.as_deref().unwrap_or_default()));
        let what = format!(
            "{id} {state}{} \"{}\" line {}: {rule}, added from {}",
            if high { " high" } else { "" },
            b.title,
            b.line,
            f.from
        );
        match guess {
            Some(_) => guesses.push(PlanRow::new("guess", what)),
            None => rows.push(PlanRow::new("idea", what)),
        }
    }
    let readme = PathBuf::from(ideas::DIR).join(ideas::README);
    if !root.join(&readme).exists() {
        rows.push(PlanRow::new(
            "create",
            format!("{} (the inbox rules)", readme.display()),
        ));
        plan.files.push((readme, Some(IDEAS_README.to_string())));
    }
    rows.push(PlanRow::new("delete", FILE));
    plan.files.push((FILE.into(), None));
    plan.rows.extend(rows);
    plan.rows.splice(0..0, guesses);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    use super::*;

    fn one(text: &str) -> Bullet {
        let mut b = parse(text);
        assert_eq!(b.len(), 1, "{b:?}");
        b.remove(0)
    }

    fn open(owed: bool, high: bool) -> Place {
        Place::Open {
            owed,
            high,
            guess: None,
        }
    }

    fn guess_of(b: &Bullet) -> Option<&str> {
        match &b.place {
            Place::Open { guess, .. } => guess.as_deref(),
            p => panic!("not migrated: {p:?}"),
        }
    }

    const GRADUATED: &str = "## Graduated → became a feature\n\n- **Delta** — became PRD-001\n\n### Parked from the PRD-001 polish review (2026-09-22)\n\n- **Echo.** worth a look\n";

    #[test]
    fn titles_bodies_and_placement() {
        let a = one("## Open\n\n- **A.** x\n");
        assert_eq!((a.title.as_str(), a.body.as_str()), ("A", "x\n"));
        assert_eq!(a.place, open(false, false));
        let b = one("## Open — High Priority\n- **B** — y\n  more\n");
        assert_eq!((b.title.as_str(), b.body.as_str()), ("B", "y\n  more\n"));
        assert_eq!(b.place, open(false, true));
        let c = one("## Owed by someone else — pending confirmations\n- **C:** z\n");
        assert_eq!(c.place, open(true, false));
        let g = parse(GRADUATED);
        assert_eq!(g[0].place, Place::NotMigrated("under Graduated"));
        assert_eq!(guess_of(&g[1]), Some("Parked below Graduated"));
        let s = one("## Someday\n- **S.** s\n");
        assert_eq!(guess_of(&s), Some("unknown heading ## Someday"));
        let p = one("## Open\n### Parked from x (2026-01-01)\n- **P.** p\n");
        assert_eq!(p.place, open(false, false));
        let n = one("## Open\n- no bold. here\n");
        assert_eq!(
            (n.title.as_str(), n.body.as_str()),
            ("no bold", "no bold. here\n")
        );
        assert_eq!(guess_of(&n), Some("no bold title"));
    }

    #[test]
    fn bounds_fences_and_drops() {
        let b = parse("## Open\n- **A.** a\n- **B.** b\n  b2\n\n  not b\n```\n- **C.** c\n```\n");
        assert_eq!(b.len(), 6, "{b:?}");
        assert_eq!(b[1].body, "b\n  b2\n");
        assert_eq!(b[2].place, Place::Dropped("not a bullet"));
        assert_eq!(
            b[3..]
                .iter()
                .map(|x| (x.line, x.title.as_str()))
                .collect::<Vec<_>>(),
            [(7, "```"), (8, "- **C.** c"), (9, "```")]
        );
        assert!(
            b[3..]
                .iter()
                .all(|x| x.place == Place::Dropped("in a fence under no bullet"))
        );
        let b = parse("## Open\n- **A.** a\n```\nx\n\n```\n");
        assert_eq!(b.len(), 4, "{b:?}");
        assert_eq!(b[0].body, "a\n");
        assert_eq!(
            b[1..].iter().map(|x| x.line).collect::<Vec<_>>(),
            [3, 4, 6],
            "the blank line is not listed"
        );
        let template = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixture/v2/.docs/templates/ideas.md"),
        )
        .unwrap();
        let t = parse(&template);
        assert_eq!(t.len(), 7, "{t:?}");
        assert!(
            t[..5]
                .iter()
                .all(|x| x.place == Place::Dropped("before any ## heading"))
        );
        assert!(
            t[5..]
                .iter()
                .all(|x| x.place == Place::Dropped("placeholder"))
        );
    }

    #[test]
    fn an_inbox_parses_bullet_by_bullet() {
        let text = "# Ideas Inbox\n\nLoose one-liners.\n\n**Rules**\n- One line per idea.\n- No design here.\n\n## Open\n\n- **First idea.** Plain body, no dash.\n- **Second idea.** Another.\n\n- **`tool pull | list | drop`: errors as a list.** After the blank line.\n\n## Graduated → became a feature\n\n- _(move lines here with their `{NNN}`)_\n";
        let all = parse(text);
        let open: Vec<&Bullet> = all
            .iter()
            .filter(|b| {
                matches!(
                    b.place,
                    Place::Open {
                        guess: None,
                        owed: false,
                        ..
                    }
                )
            })
            .collect();
        assert_eq!(
            open.iter().map(|b| b.title.as_str()).collect::<Vec<_>>(),
            [
                "First idea",
                "Second idea",
                "`tool pull | list | drop`: errors as a list"
            ]
        );
        assert_eq!(open[0].body, "Plain body, no dash.\n");
        assert_eq!(open[2].body, "After the blank line.\n");
        let drops: Vec<(usize, &Place)> = all
            .iter()
            .filter(|b| matches!(b.place, Place::Dropped(_)))
            .map(|b| (b.line, &b.place))
            .collect();
        assert_eq!(
            drops,
            [
                (6, &Place::Dropped("before any ## heading")),
                (7, &Place::Dropped("before any ## heading")),
                (18, &Place::Dropped("placeholder")),
            ],
            "{all:#?}"
        );
        assert_eq!(all.len(), open.len() + drops.len(), "{all:#?}");

        let tmp = tree(text, &[]);
        let mut p = empty();
        plan(tmp.path(), &mut p).unwrap();
        let (f, t) = p
            .files
            .iter()
            .find(|(f, _)| f.to_string_lossy().contains("tool-pull"))
            .unwrap_or_else(|| panic!("{:?}", p.files));
        let f = tmp.path().join(f);
        fs::write(&f, t.as_deref().unwrap()).unwrap();
        assert_eq!(ideas::read(&f).unwrap().title, open[2].title);
    }

    fn fields_of(text: &str) -> Fields {
        fields(&one(text), "2026-09-30")
    }

    #[test]
    fn fields_come_from_the_phrases_then_the_heading_then_blame() {
        let f = fields_of("## Open\n- **A.** Parked from PRD-001 polish (2026-10-10): slow.\n");
        assert_eq!(
            (f.added.as_str(), f.origin.as_deref(), f.from),
            ("2026-10-10", Some("PRD-001 polish"), "Parked")
        );
        let f = fields_of("## Open\n- **A.** Added 2026-10-11, not assessed.\n");
        assert_eq!(
            (f.added.as_str(), f.origin, f.from),
            ("2026-10-11", None, "Added")
        );
        let h = "## Graduated\n### Parked from the PRD-001 polish review (2026-09-22)\n";
        let f = fields_of(&format!("{h}- **E.** worth a look\n"));
        assert_eq!(
            (f.added.as_str(), f.origin.as_deref(), f.from),
            ("2026-09-22", Some("PRD-001 polish review"), "the heading")
        );
        let f = fields_of(&format!(
            "{h}- **E.** Parked from the owner review (2026-10-11).\n"
        ));
        assert_eq!(
            (f.added.as_str(), f.origin.as_deref()),
            ("2026-10-11", Some("owner review"))
        );
        let f = fields_of("## Open\n- **R.** x. Re-trigger: when PRD-004 ships. Then look.\n");
        assert_eq!(f.retrigger.as_deref(), Some("when PRD-004 ships."));
        let f = fields_of("## Open\n- **N.** nothing here\n");
        assert_eq!(
            (f.added.as_str(), f.origin, f.retrigger, f.from),
            ("2026-09-30", None, None, "git blame")
        );
    }

    fn git(root: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .current_dir(root)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_DATE", "2026-09-30T12:00:00+00:00")
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?}");
    }

    fn tree(ideas_md: &str, existing: &[&str]) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(ideas::DIR)).unwrap();
        fs::write(root.join(FILE), ideas_md).unwrap();
        for id in existing {
            fs::write(root.join(ideas::DIR).join(format!("{id}.md")), "x\n").unwrap();
        }
        git(root, &["init", "-q"]);
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "v2"]);
        tmp
    }

    fn empty() -> Plan {
        Plan {
            rows: Vec::new(),
            files: Vec::new(),
            retros: Vec::new(),
        }
    }

    fn ids(p: &Plan) -> Vec<String> {
        p.files
            .iter()
            .filter(|(f, t)| t.is_some() && !f.ends_with(ideas::README))
            .map(|(f, _)| f.file_stem().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn ids_clash_in_file_order_and_against_the_folder() {
        let md = "## Open\n- **Same.** one\n- **Same.** two\n- **Other.** three\n";
        let tmp = tree(md, &["2026-09-30-other"]);
        let mut a = empty();
        plan(tmp.path(), &mut a).unwrap();
        assert_eq!(
            ids(&a),
            ["2026-09-30-same", "2026-09-30-same-2", "2026-09-30-other-2"]
        );
        let mut b = empty();
        plan(tmp.path(), &mut b).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.files.last().unwrap(), &(PathBuf::from(FILE), None));
    }

    #[test]
    fn a_quote_renders_single_quoted_and_both_refuse() {
        let tmp = tree("## Open\n- **Say \"hi\".** body\n", &[]);
        let mut p = empty();
        plan(tmp.path(), &mut p).unwrap();
        let text = p.files[0].1.clone().unwrap();
        assert!(text.contains("title: 'Say \"hi\"'\n"), "{text}");
        let f = tmp.path().join(ideas::DIR).join("2026-09-30-say-hi.md");
        fs::write(&f, &text).unwrap();
        assert_eq!(ideas::read(&f).unwrap().title, "Say \"hi\"");

        let tmp = tree("## Open\n\n- **It's \"x\".** body\n", &[]);
        let e = plan(tmp.path(), &mut empty()).unwrap_err().to_string();
        assert!(e.contains("docs/ideas.md:3"), "{e}");
    }

    #[test]
    fn guess_rows_go_first() {
        let tmp = tree(&format!("## Open\n- **A.** a\n\n{GRADUATED}"), &[]);
        let mut p = empty();
        p.rows.push(PlanRow::new("create", "docs/project.toml"));
        plan(tmp.path(), &mut p).unwrap();
        assert_eq!(p.rows[0].verb, "guess");
        assert!(p.rows[0].what.contains("\"Echo\""), "{:?}", p.rows);
        assert!(
            p.rows
                .iter()
                .any(|r| r.what.starts_with("not migrated \"Delta\""))
        );
    }
}
