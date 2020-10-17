// Code taken from:
// https://github.com/eminence/terminal-size

#[derive(Debug)]
pub struct Width(pub u16);
#[derive(Debug)]
pub struct Height(pub u16);

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use crate::unix::{terminal_size, terminal_size_using_fd};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::terminal_size;