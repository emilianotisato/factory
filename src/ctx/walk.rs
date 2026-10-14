//! `--vendor` also lifts `.gitignore`, since a vendor tree is usually gitignored; files sort by path so threads never reorder output.
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use ignore::{WalkBuilder, WalkState};

pub const VENDOR_DIRS: [&str; 3] = ["vendor", "node_modules", "target"];

/// Only directly under the root: deeper, a `build/` or `storage/` is as often source.
pub const ROOT_DIRS: [&str; 3] = ["dist", "build", "storage"];

pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

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
    pub vendor: bool,
}

#[derive(Debug, Default)]
pub struct Walk {
    pub files: Vec<(PathBuf, Lang)>,
    pub too_large: usize,
}

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
