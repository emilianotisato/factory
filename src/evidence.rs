//! Checkpoint row evidence directories under a run's `evidence/` tree.

use std::fs;
use std::path::{Path, PathBuf};

use crate::Result;
use crate::error::io;

/// If `cp-{n}/` exists under `evidence`, renames it to the first unused `cp-{n}.sent-back-{k}/`.
pub fn rotate_stale_evidence(evidence: &Path, n: u32) -> Result<Option<PathBuf>> {
    let src = evidence.join(format!("cp-{n}"));
    if !src.exists() {
        return Ok(None);
    }
    let mut k = 1u32;
    loop {
        let dest = evidence.join(format!("cp-{n}.sent-back-{k}"));
        if !dest.exists() {
            fs::rename(&src, &dest).map_err(io(format!(
                "rename {} -> {}",
                src.display(),
                dest.display()
            )))?;
            return Ok(Some(dest));
        }
        k += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_stale_evidence_uses_the_first_free_sent_back_slot() {
        let tmp = tempfile::tempdir().unwrap();
        let evidence = tmp.path().join("evidence");
        let cp = evidence.join("cp-1");
        fs::create_dir_all(cp.join("FR-1")).unwrap();
        fs::write(cp.join("FR-1/proof.txt"), "x").unwrap();
        let first = rotate_stale_evidence(&evidence, 1).unwrap().unwrap();
        assert_eq!(first, evidence.join("cp-1.sent-back-1"));
        assert!(first.join("FR-1/proof.txt").is_file());
        assert!(!cp.exists());

        fs::create_dir_all(&cp).unwrap();
        let second = rotate_stale_evidence(&evidence, 1).unwrap().unwrap();
        assert_eq!(second, evidence.join("cp-1.sent-back-2"));
        assert!(!cp.exists());
        assert!(evidence.join("cp-1.sent-back-1").is_dir());
    }
}
