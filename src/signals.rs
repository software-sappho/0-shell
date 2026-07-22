//! SIGINT (Ctrl+C) handling for the REPL.
//!
//! Installs a handler via `sigaction` that only sets a flag — it must never
//! unwind, exit, or call into Rust code that isn't async-signal-safe. The REPL
//! notices the flag / `EINTR` and reprints the prompt.

use std::sync::atomic::{AtomicBool, Ordering};

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

/// True after SIGINT until cleared by the REPL.
pub fn take_interrupted() -> bool {
    INTERRUPTED.swap(false, Ordering::SeqCst)
}

pub fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

/// Install a non-terminating SIGINT handler. Safe to call once at startup.
pub fn install_sigint_handler() {
    #[cfg(unix)]
    unix::install();
}

#[cfg(unix)]
mod unix {
    use super::INTERRUPTED;
    use std::sync::atomic::Ordering;

    const SIGINT: i32 = 2;

    unsafe extern "C" fn on_sigint(_: i32) {
        // Async-signal-safe: only an atomic store.
        INTERRUPTED.store(true, Ordering::SeqCst);
    }

    pub fn install() {
        // Prefer sigaction without SA_RESTART so blocking reads return EINTR
        // and the REPL can cancel the current line.
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            #[repr(C)]
            struct SigSet {
                val: [u64; 16],
            }

            #[repr(C)]
            struct SigAction {
                sa_handler: usize,
                sa_mask: SigSet,
                sa_flags: i32,
                sa_restorer: usize,
            }

            unsafe extern "C" {
                fn sigaction(sig: i32, act: *const SigAction, old: *mut SigAction) -> i32;
                fn sigemptyset(set: *mut SigSet) -> i32;
            }

            unsafe {
                let mut act = SigAction {
                    sa_handler: on_sigint as *const () as usize,
                    sa_mask: SigSet { val: [0; 16] },
                    sa_flags: 0, // no SA_RESTART
                    sa_restorer: 0,
                };
                let _ = sigemptyset(&mut act.sa_mask);
                let rc = sigaction(SIGINT, &act, std::ptr::null_mut());
                if rc != 0 {
                    // Fall back to signal(2) if sigaction failed for any reason.
                    install_with_signal();
                }
            }
            return;
        }

        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        install_with_signal();
    }

    fn install_with_signal() {
        unsafe extern "C" {
            fn signal(sig: i32, handler: usize) -> usize;
        }
        unsafe {
            let _ = signal(SIGINT, on_sigint as *const () as usize);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_interrupted_clears_the_flag() {
        INTERRUPTED.store(true, Ordering::SeqCst);
        assert!(take_interrupted());
        assert!(!interrupted());
        assert!(!take_interrupted());
    }
}
