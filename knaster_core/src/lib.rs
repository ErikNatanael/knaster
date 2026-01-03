#![no_std]
#![cfg_attr(feature = "unstable", feature(portable_simd))]

//! # Knaster Core
//!
//! Knaster Core contains everything you need to implement the Knaster traits for your own types.
//!
//! ## Re-exports
//! knaster_core re-exports all of knaster_primitives. This makes it easier to keep knaster_primitives versions consistent among implementors of the traits in knaster_core and the graph in knaster_graph.
//!
//! # Features
//!
//! - `alloc`: Enables alloc in `knaster_primitives` and heap based data structures
// #[cfg(feature = "alloc")]
// extern crate alloc;
// #[cfg(feature = "std")]
// extern crate std;
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

pub mod log;
mod parameters;
mod ugen;

pub use knaster_macros::*;
use knaster_primitives::num_derive::*;
pub use knaster_primitives::*;
pub use parameters::*;
use std::prelude::v1::*;
pub use ugen::*;

/// Rate determines the speed at which something is running. Something running
/// at block rate is only calculated once per block, whereas something running
/// at audio rate is calculated for every frame.
///
/// Note that knaster supports adaptive partial blocks. This means that block
/// the `UGen::process_block` function may run more than once per global block
/// size number of frames.
#[derive(Default, Debug, Copy, Clone)]
pub enum Rate {
    /// Smoothing happens at block rate
    #[default]
    BlockRate,
    /// Smoothing happens at audio rate
    AudioRate,
}

/// Specify an action to take once this [`UGen`] is done.
///
/// Some UGens have a "done" state. This enum represents a list of standardised actions to take
/// when done.
#[derive(
    Default, Debug, PartialEq, Eq, Copy, Clone, FromPrimitive, ToPrimitive, KnasterIntegerParameter,
)]
#[num_traits = "num_traits"]
#[repr(u8)]
pub enum Done {
    /// Don't do anything when done
    #[default]
    None = 0,
    /// Free only the current UGen node.
    FreeSelf,
    /// Free the structure that contains the node. In knaster_graph, that is the `Graph`.
    FreeParent,
}
