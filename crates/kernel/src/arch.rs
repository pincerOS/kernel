#[cfg(target_arch = "aarch64")]
pub mod aarch64;
#[cfg(target_arch = "aarch64")]
pub use aarch64::*;

#[cfg(not(target_arch = "aarch64"))]
pub mod stubs;
#[cfg(not(target_arch = "aarch64"))]
pub use stubs::*;
