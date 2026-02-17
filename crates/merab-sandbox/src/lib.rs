#[cfg(windows)]
pub mod windows;

#[cfg(windows)]
pub use windows::JobObject;

#[cfg(not(windows))]
pub mod unix;

#[cfg(not(windows))]
pub use unix::JobObject;
