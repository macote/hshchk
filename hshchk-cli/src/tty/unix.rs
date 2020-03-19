extern crate libc;
use super::{Height, Width};

// We need to convert from c_int to c_ulong at least on DragonFly and FreeBSD.
#[cfg(any(target_os = "dragonfly", target_os = "freebsd"))]
fn ioctl_conv<T: Into<libc::c_ulong>>(v: T) -> libc::c_ulong {
    v.into()
}

// No-op on any other operating system.
#[cfg(not(any(target_os = "dragonfly", target_os = "freebsd")))]
fn ioctl_conv<T: Copy>(v: T) -> T {
    v
}

/// Returns the size of the terminal, if available.
///
/// If STDOUT is not a tty, returns `None`
pub fn terminal_size() -> Option<(Width, Height)> {
    use self::libc::{ioctl, isatty, winsize, STDOUT_FILENO, TIOCGWINSZ};
    let is_tty: bool = unsafe { isatty(STDOUT_FILENO) == 1 };

    if !is_tty {
        return None;
    }

    let (rows, cols) = unsafe {
        let mut winsize = winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        ioctl(STDOUT_FILENO, ioctl_conv(TIOCGWINSZ), &mut winsize);
        let rows = if winsize.ws_row > 0 {
            winsize.ws_row
        } else {
            0
        };
        let cols = if winsize.ws_col > 0 {
            winsize.ws_col
        } else {
            0
        };
        (rows as u16, cols as u16)
    };

    if rows > 0 && cols > 0 {
        Some((Width(cols), Height(rows)))
    } else {
        None
    }
}
