#![cfg_attr(not(feature = "std"), no_std)]
// ^ enables no_std if the `std` features isn't enabled

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
#[cfg(feature = "alloc")]
extern crate alloc;

// Switches between std and core based on features. This reduces boilerplate when importing.
mod core {
    #[cfg(not(feature = "std"))]
    pub use core::*;
    #[cfg(feature = "std")]
    pub use std::*;
}

pub mod dsp;
mod gen;
mod parameters;
#[cfg(test)]
mod tests;
pub mod wrappers_core;

pub use gen::*;
pub use knaster_primitives::*;
pub use parameters::*;

/// Rate determines the speed at which something is running. Something running
/// at block rate is only calculated once per block, whereas something running
/// at audio rate is calculated for every frame.
///
/// Note that knaster supports adaptive partial blocks. This means that block
/// the `Gen::process_block` function may run more than once per global block
/// size number of frames.
#[derive(Default, Debug, Copy, Clone)]
pub enum Rate {
    /// Smoothing happens at block rate
    #[default]
    BlockRate,
    /// Smoothing happens at audio rate
    AudioRate,
}

/// Specify an action to take once this [`Gen`] is done.
///
/// Some Gens have a "done" state. This enum represents a list of standardised actions to take
/// when done.
#[derive(Default, Debug, PartialEq, Eq, Copy, Clone)]
#[repr(u8)]
pub enum Done {
    #[default]
    None = 0,
    /// Free only the current Gen node.
    FreeSelf,
    /// Free the structure that contains the node. In knaster_graph, that is the `Graph`.
    FreeParent,
}
impl From<PInteger> for Done {
    fn from(value: PInteger) -> Self {
        match value.0 {
            0 => Done::None,
            1 => Done::FreeSelf,
            2 => Done::FreeParent,
            _ => Done::None,
        }
    }
}
impl From<Done> for PInteger {
    fn from(value: Done) -> Self {
        PInteger(value as usize)
    }
}
impl PIntegerConvertible for Done {
    fn pinteger_range() -> (PInteger, PInteger) {
        (PInteger(0), PInteger(2))
    }
}

