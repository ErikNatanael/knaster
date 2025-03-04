#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

// Switches between std and core based on features. This reduces boilerplate when importing.
mod core {
    #[cfg(not(feature = "std"))]
    pub use alloc::*;
    #[cfg(not(feature = "std"))]
    pub use core::*;
    #[cfg(feature = "std")]
    pub use std::*;
}

pub mod math_ugens;
pub mod node_ops;
pub mod prelude;
pub mod preludef32;
pub(crate) mod subprelude_fundamental_types;
pub use knaster_graph::*;
