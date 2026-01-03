#![no_std]
#![cfg_attr(feature = "unstable", feature(portable_simd))]

//! # Knaster Core DSP
//!
//! Knaster Core DSP contains DSP code: UGens, wrappers, etc.
//!
//! ## Re-exports
//! knaster_core_dsp depends on, but doesn't re-export anything from knaster_core.
//!
//! # Features
//!
//! - `alloc`: Enables alloc in `knaster_core` and heap based data structures
//! - `std`: Enables the standard library. `alloc` does nothing if `std` is enabled.
//! - `unstable`: Enables unstable Rust features that may speed up execution
#![deny(rustdoc::broken_intra_doc_links)] // error if there are broken intra-doc links
#![warn(missing_docs)]

extern crate no_std_compat as std;

// Switches between std and core based on features. This reduces boilerplate when importing.
mod core {
    pub use no_std_compat::*;
    // #[cfg(all(feature = "alloc", not(feature = "std")))]
    // extern crate alloc as __alloc;
    // #[cfg(all(feature = "alloc", not(feature = "std")))]
    // pub use __alloc::*;
    // #[cfg(not(feature = "std"))]
    // pub use core::*;
    // #[cfg(feature = "std")]
    // pub use std::*;
}

mod ugens;
pub use ugens::*;
pub mod dsp;
pub mod wrappers_core;

#[cfg(test)]
pub(crate) mod test_utils;
