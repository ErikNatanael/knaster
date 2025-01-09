#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::single_match)]
#![allow(clippy::new_without_default)]
//! # Knaster Graph
//!
//! This crate contains a dynamically modifiable graph implementation using the
//! foundation in knaster_core.
//!
//!
//! ## no_std
//!
//! knaster_graph is no_std, but requires `alloc`.
//!
//! ## Features
//!
//! - `std`: Enables std, disabling no_std.
//!
//! # Codebase conventions
//!
//! - The generic parameter `F` is the float type, f32 or f64
extern crate alloc;

// Switches between std and core based on features. This reduces boilerplate when importing.
mod core {
    #[cfg(not(feature = "std"))]
    pub use core::*;
    #[cfg(feature = "std")]
    pub use std::*;
}

pub use knaster_core::*;

pub mod audio_backend;
pub mod block;
mod buffer_allocator;
pub mod connectable;
mod dyngen;
mod edge;
pub mod graph;
pub(crate) mod graph_gen;
pub mod handle;
pub mod inspection;
mod node;
pub mod runner;
mod scheduling;
mod task;
#[cfg(test)]
mod tests;
pub mod wrappers_graph;

pub use scheduling::*;
