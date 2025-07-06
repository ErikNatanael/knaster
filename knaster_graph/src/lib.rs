#![no_std]
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
#![deny(rustdoc::broken_intra_doc_links)] // error if there are broken intra-doc links
#![warn(missing_docs)]

// Switches between std and core based on features. This reduces boilerplate when importing.
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
pub use knaster_core::*;

// Deprecated
pub mod handle;
// pub mod connectable;
// pub mod connectable2;

pub mod audio_backend;
pub mod block;
mod buffer_allocator;
pub mod dynugen;
mod edge;
pub mod graph;
pub mod graph_edit;
pub(crate) mod graph_gen;
pub mod inspection;
mod node;
pub mod processor;
mod scheduling;
mod task;
#[cfg(test)]
mod tests;
pub mod wrappers_graph;

pub use scheduling::*;

/// Macro to set many parameters on a graph at once.
///
/// It is equivalent to calling [`Graph::set_many`] with the same parameters, calling `.into` on  
/// every element.
///
/// # Example
/// ```rust
/// # use knaster_graph::osc::SinNumeric;
/// # use knaster_graph::processor::{AudioProcessor, AudioProcessorOptions};
/// # use knaster_graph::set_many;
/// # use knaster_core::{Param, UGen, typenum::U0, typenum::U2};
/// # use knaster_graph::Time;
/// let (mut graph, mut audio_processor, log_receiver) = AudioProcessor::<f32>::new::<U0, U2>(AudioProcessorOptions::default());
/// let osc0 = graph.push(SinNumeric::new(440.));
/// let osc1 = graph.push(SinNumeric::new(440.));
/// set_many!(graph, Time::asap(); (&osc0, "freq", 440.), (&osc1, "freq", 880.));
/// ```
#[macro_export]
macro_rules! set_many {
    ($graph:expr, $time:expr; $(($node:expr, $param:expr, $value:expr)),* $(,)?) => {
        $graph.set_many(&[
            $((($node).into(), ($param).into(), ($value).into())),*
        ], $time.into());
    };
}
