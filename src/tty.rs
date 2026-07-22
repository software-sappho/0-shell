//! Terminal raw-mode helpers for interactive line editing (Unix).

#[cfg(unix)]
mod unix {
    use std::io;

    unsafe extern "C" {
        fn isatty(fd: i32) -> i32;
    }

    pub fn stdin_is_tty() -> bool {
        unsafe { isatty(0) == 1 }
    }

    /// Enter non-canonical, no-echo mode on stdin. Restored on drop.
    ///
    /// Implemented for Linux termios layout; other Unix platforms return an
    /// error so the REPL falls back to cooked input.
    pub struct RawMode {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        fd: i32,
        #[cfg(any(target_os = "linux", target_os = "android"))]
        original: Termios,
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[repr(C)]
    #[derive(Clone)]
    struct Termios {
        c_iflag: u32,
        c_oflag: u32,
        c_cflag: u32,
        c_lflag: u32,
        c_line: u8,
        c_cc: [u8; 32],
        c_ispeed: u32,
        c_ospeed: u32,
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    const ICANON: u32 = 0o0000002;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const ECHO: u32 = 0o0000010;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const TCSANOW: i32 = 0;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const VMIN: usize = 6;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const VTIME: usize = 5;

    impl RawMode {
        pub fn enter() -> io::Result<Self> {
            #[cfg(any(target_os = "linux", target_os = "android"))]
            {
                use std::mem::MaybeUninit;

                unsafe extern "C" {
                    fn tcgetattr(fd: i32, termios_p: *mut Termios) -> i32;
                    fn tcsetattr(fd: i32, optional_actions: i32, termios_p: *const Termios) -> i32;
                }

                let fd = 0;
                let mut original = MaybeUninit::<Termios>::uninit();
                let rc = unsafe { tcgetattr(fd, original.as_mut_ptr()) };
                if rc != 0 {
                    return Err(io::Error::last_os_error());
                }
                let original = unsafe { original.assume_init() };
                let mut raw = original.clone();
                raw.c_lflag &= !(ICANON | ECHO);
                raw.c_cc[VMIN] = 1;
                raw.c_cc[VTIME] = 0;
                let rc = unsafe { tcsetattr(fd, TCSANOW, &raw) };
                if rc != 0 {
                    return Err(io::Error::last_os_error());
                }
                return Ok(Self { fd, original });
            }
            #[cfg(not(any(target_os = "linux", target_os = "android")))]
            {
                Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "raw mode not implemented on this OS",
                ))
            }
        }
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    impl Drop for RawMode {
        fn drop(&mut self) {
            unsafe extern "C" {
                fn tcsetattr(fd: i32, optional_actions: i32, termios_p: *const Termios) -> i32;
            }
            unsafe {
                let _ = tcsetattr(self.fd, TCSANOW, &self.original);
            }
        }
    }
}

#[cfg(unix)]
pub use unix::{stdin_is_tty, RawMode};

#[cfg(not(unix))]
pub fn stdin_is_tty() -> bool {
    false
}
