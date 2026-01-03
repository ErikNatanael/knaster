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

#[cfg(not(any(feature = "std", feature = "alloc")))]
compile_error!("Either std or alloc must be enabled");

// Switches between std and core based on features. This reduces boilerplate when importing.
extern crate no_std_compat as std;

// Switches between std and core based on features. This reduces boilerplate when importing.
mod core {
    #[cfg(any(feature = "std", feature = "alloc"))]
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
pub use knaster_core_dsp::*;

// All these cfg are to make sure the error message for neither std nor alloc is shown. Without
// them, too many other errors are shown which is confusing.

// Deprecated
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod handle;
// pub mod connectable;
// pub mod connectable2;

#[cfg(any(feature = "std", feature = "alloc"))]
pub mod audio_backend;
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod block;
#[cfg(any(feature = "std", feature = "alloc"))]
mod buffer_allocator;
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod dynugen;
#[cfg(any(feature = "std", feature = "alloc"))]
mod edge;
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod graph;
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod graph_edit;
#[cfg(any(feature = "std", feature = "alloc"))]
pub(crate) mod graph_gen;
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod inspection;
#[cfg(any(feature = "std", feature = "alloc"))]
mod node;
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod processor;
#[cfg(any(feature = "std", feature = "alloc"))]
mod scheduling;
#[cfg(any(feature = "std", feature = "alloc"))]
mod task;
#[cfg(test)]
mod tests;
#[cfg(any(feature = "std", feature = "alloc"))]
pub mod wrappers_graph;

#[cfg(any(feature = "std", feature = "alloc"))]
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
