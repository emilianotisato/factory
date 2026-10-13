//! `factory docs retro NNN [--set score=N]` (PRD-002 FR-19, slice 06): the
//! feature's `retros` row in the store under `FACTORY_DATA_DIR`.
#[path = "harness/mod.rs"]
mod harness;

use std::path::PathBuf;
use std::process::Output;

use factory::db::Db;
use harness::Fixture;

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn store(fx: &Fixture) -> PathBuf {
    PathBuf::from(&fx.env()["FACTORY_DATA_DIR"]).join("factory.db")
}

fn run(fx: &Fixture, args: &[&str]) -> (Option<i32>, String, String) {
    let o: Output = fx.docs_output(args);
    (o.status.code(), text(&o.stdout), text(&o.stderr))
}

#[test]
fn retro_prints_the_row_and_sets_only_an_integer_score() {
    let fx = Fixture::new("docs-retro");
    let repo = fx.path().canonicalize().unwrap();
    let repo = repo.to_str().unwrap();

    let (code, out, err) = run(&fx, &["retro", "001"]);
    assert_eq!(code, Some(1), "{out}{err}");
    assert_eq!(out, "");
    assert_eq!(err.trim(), "no retro row for PRD-001");
    assert_eq!(Db::open(&store(&fx)).unwrap().retro(repo, 1).unwrap(), None);

    let (code, set, err) = run(&fx, &["retro", "001", "--set", "score=4"]);
    assert_eq!(code, Some(0), "{err}");
    let lines: Vec<&str> = set.lines().collect();
    assert_eq!(lines.len(), 19, "{set}");
    assert_eq!(lines[0], format!("repo: {repo}"));
    assert!(lines.contains(&"feature: 1"), "{set}");
    assert!(lines.contains(&"slug: fixture"), "{set}");
    assert!(lines.contains(&"score: 4"), "{set}");
    assert!(lines.contains(&"gate_runs: "), "{set}");
    assert!(lines[18].starts_with("written_at: 20"), "{set}");

    let (code, shown, _) = run(&fx, &["retro", "fixture"]);
    assert_eq!(code, Some(0));
    assert_eq!(shown, set);

    let (code, _, err) = run(&fx, &["retro", "001", "--set", "score=high"]);
    assert_eq!(code, Some(2));
    assert!(err.contains("score must be an integer"), "{err}");
    let row = Db::open(&store(&fx))
        .unwrap()
        .retro(repo, 1)
        .unwrap()
        .unwrap();
    assert_eq!(row.score, Some(4));

    let (code, _, err) = run(&fx, &["retro", "001", "--set", "gate_runs=3"]);
    assert_eq!(code, Some(2));
    assert!(err.contains("only `score` can be set"), "{err}");

    let (code, _, err) = run(&fx, &["retro", "999"]);
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains("no feature matching"), "{err}");

    assert!(store(&fx).is_file());
}
