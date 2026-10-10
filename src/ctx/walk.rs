//! The file walk every `ctx` verb starts from (PRD §14 FR-12): the `ignore`
//! crate's walker, which honours `.gitignore` and skips hidden files, plus
//! the vendor and build directories no lookup wants. `--vendor` lifts both, since a
//! vendor tree is usually gitignored too. Only files of a known language
//! come out, sorted by path so the output never depends on thread timing.
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use ignore::{WalkBuilder, WalkState};

/// Dependency directories skipped unless `--vendor`, at any depth.
pub const VENDOR_DIRS: [&str; 3] = ["vendor", "node_modules", "target"];

/// Build output and Laravel's `storage/`, skipped unless `--vendor` only
/// directly under the search root: deeper, a `build/` or `storage/` is as
/// often source (`src/build/`, `app/storage/`), AD-40004.
pub const ROOT_DIRS: [&str; 3] = ["dist", "build", "storage"];

/// A file larger than this is skipped and counted in the trailer.
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// The languages with a definition regex. Vue shares TypeScript's regexes;
/// slice 12 parses its `<script>` block apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    Php,
    Ts,
    Vue,
    Go,
    Rust,
}

impl Lang {
    pub fn from_path(path: &Path) -> Option<Lang> {
        match path.extension()?.to_str()? {
            "php" => Some(Lang::Php),
            "ts" | "tsx" | "js" | "jsx" | "mjs" => Some(Lang::Ts),
            "vue" => Some(Lang::Vue),
            "go" => Some(Lang::Go),
            "rs" => Some(Lang::Rust),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WalkOpts {
    /// Walk vendor directories and gitignored paths too.
    pub vendor: bool,
}

/// What a walk found: the source files, sorted by path, and how many were
/// left out for size.
#[derive(Debug, Default)]
pub struct Walk {
    pub files: Vec<(PathBuf, Lang)>,
    pub too_large: usize,
}

/// Walks `root` in parallel and returns its source files, sorted.
pub fn walk(root: &Path, opts: &WalkOpts) -> Walk {
    let mut builder = WalkBuilder::new(root);
    if opts.vendor {
        builder.standard_filters(false).hidden(true);
    } else {
        builder.filter_entry(|e| {
            let is_dir = e.file_type().is_some_and(|t| t.is_dir());
            let name = e.file_name().to_str().unwrap_or("");
            let skipped =
                VENDOR_DIRS.contains(&name) || (e.depth() == 1 && ROOT_DIRS.contains(&name));
            e.depth() == 0 || !(is_dir && skipped)
        });
    }
    let found = Mutex::new(Vec::new());
    let too_large = AtomicUsize::new(0);
    builder.build_parallel().run(|| {
        Box::new(|entry| {
            let Ok(entry) = entry else {
                return WalkState::Continue;
            };
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                return WalkState::Continue;
            }
            let Some(lang) = Lang::from_path(entry.path()) else {
                return WalkState::Continue;
            };
            if entry.metadata().is_ok_and(|m| m.len() > MAX_FILE_BYTES) {
                too_large.fetch_add(1, Ordering::Relaxed);
                return WalkState::Continue;
            }
            found
                .lock()
                .expect("walk results lock")
                .push((entry.into_path(), lang));
            WalkState::Continue
        })
    });
    let mut files = found.into_inner().expect("walk results lock");
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Walk {
        files,
        too_large: too_large.into_inner(),
    }
}

/// Reads every file in `files` on all cores and hands its bytes to `f`.
/// Binary files (a NUL byte anywhere) and unreadable ones are skipped; the
/// first count is returned. Results come back in `files` order.
pub fn scan<T, F>(files: &[(PathBuf, Lang)], f: F) -> (Vec<(usize, T)>, usize)
where
    T: Send,
    F: Fn(&Path, Lang, &[u8]) -> Option<T> + Sync,
{
    let next = AtomicUsize::new(0);
    let binary = AtomicUsize::new(0);
    let found = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(files.len().max(1));
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some((path, lang)) = files.get(i) else {
                        break;
                    };
                    let Ok(bytes) = std::fs::read(path) else {
                        continue;
                    };
                    if bytes.contains(&0) {
                        binary.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    if let Some(t) = f(path, *lang, &bytes) {
                        found.lock().expect("scan results lock").push((i, t));
                    }
                }
            });
        }
    });
    let mut found = found.into_inner().expect("scan results lock");
    found.sort_by_key(|(i, _)| *i);
    (found, binary.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_come_from_the_extension() {
        for (p, l) in [
            ("a/B.php", Some(Lang::Php)),
            ("x.blade.php", Some(Lang::Php)),
            ("a.ts", Some(Lang::Ts)),
            ("a.tsx", Some(Lang::Ts)),
            ("a.js", Some(Lang::Ts)),
            ("a.jsx", Some(Lang::Ts)),
            ("a.mjs", Some(Lang::Ts)),
            ("C.vue", Some(Lang::Vue)),
            ("main.go", Some(Lang::Go)),
            ("lib.rs", Some(Lang::Rust)),
            ("README.md", None),
            ("Makefile", None),
        ] {
            assert_eq!(Lang::from_path(Path::new(p)), l, "{p}");
        }
    }
}
