//! One handler forwards terminal signals to every registered child process group.
use std::sync::Once;
use std::sync::atomic::{AtomicI32, Ordering};

use crate::Result;
use crate::error::Error;

const SLOTS: usize = 256;
static CHILD_GROUPS: [AtomicI32; SLOTS] = [const { AtomicI32::new(0) }; SLOTS];

/// Holds a slot from before the spawn (as -1, which the handler skips) until the child is done.
pub struct Guard(&'static AtomicI32);

impl Guard {
    pub fn claim() -> Result<Guard> {
        CHILD_GROUPS
            .iter()
            .find(|s| {
                s.compare_exchange(0, -1, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
            })
            .map(Guard)
            .ok_or_else(|| Error::Msg(format!("more than {SLOTS} child groups at once")))
    }

    pub fn set(&self, pgid: i32) {
        self.0.store(pgid, Ordering::SeqCst);
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.0.store(0, Ordering::SeqCst);
    }
}

#[cfg(unix)]
extern "C" fn forward(sig: libc::c_int) {
    // SAFETY: atomics and async-signal-safe calls only; the re-raise under SIG_DFL ends factory as the signal would.
    unsafe {
        for slot in &CHILD_GROUPS {
            let group = slot.load(Ordering::SeqCst);
            if group > 0 {
                libc::kill(-group, libc::SIGKILL);
            }
        }
        libc::signal(sig, libc::SIG_DFL);
        libc::raise(sig);
    }
}

/// A signal factory started out ignoring (`nohup`) stays ignored.
#[cfg(unix)]
pub fn install() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        for sig in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            // SAFETY: a zeroed `sigaction` is valid; the handler is async-signal-safe.
            unsafe {
                let mut old: libc::sigaction = std::mem::zeroed();
                libc::sigaction(sig, std::ptr::null(), &mut old);
                if old.sa_sigaction == libc::SIG_IGN {
                    continue;
                }
                let mut sa: libc::sigaction = std::mem::zeroed();
                sa.sa_sigaction = forward as extern "C" fn(libc::c_int) as libc::sighandler_t;
                libc::sigemptyset(&mut sa.sa_mask);
                libc::sigaction(sig, &sa, std::ptr::null_mut());
            }
        }
    });
}

#[cfg(not(unix))]
pub fn install() {}
