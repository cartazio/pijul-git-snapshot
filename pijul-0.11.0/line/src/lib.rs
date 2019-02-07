#[cfg(unix)]
extern crate libc;
#[cfg(unix)]
extern crate utf8parse;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::*;
