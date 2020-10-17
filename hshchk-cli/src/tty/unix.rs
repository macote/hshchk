use super::{Height, Width};
use std::os::unix::io::RawFd;

/// Returns the size of the terminal defaulting to STDOUT, if available.
///
/// If STDOUT is not a tty, returns `None`
pub fn terminal_size() -> Option<(Width, Height)> {
    terminal_size_using_fd(libc::STDOUT_FILENO)
}

/// Returns the size of the terminal using the given file descriptor, if available.
///
/// If the given file descriptor is not a tty, returns `None`
pub fn terminal_size_using_fd(fd: RawFd) -> Option<(Width, Height)> {
    use libc::ioctl;
    use libc::isatty;
    use libc::{winsize as WinSize, TIOCGWINSZ};
    let is_tty: bool = unsafe { isatty(fd) == 1 };

    if !is_tty {
        return None;
    }

    let mut winsize = WinSize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    if unsafe { ioctl(fd, TIOCGWINSZ.into(), &mut winsize) } == -1 {
        return None;
    }

    let rows = winsize.ws_row;
    let cols = winsize.ws_col;

    if rows > 0 && cols > 0 {
        Some((Width(cols), Height(rows)))
    } else {
        None
    }
}
