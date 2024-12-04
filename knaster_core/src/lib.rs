#![no_std]
//! # Knaster Core
//!
//! Knaster Core contains everything you need to implement the Knaster traits for your own types.
//!
//! ## Re-exports
//! knaster_core re-exports all of knaster_primitives. This makes it easier to keep knaster_primitives versions consistent among implementors of the traits in knaster_core and the graph in knaster_graph.

mod gen;
mod parameters;
pub mod wrappers;

pub use gen::*;
pub use knaster_primitives::*;

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
