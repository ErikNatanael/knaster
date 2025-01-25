//! Contains DSP related code in knaster_core which is neither implementations
//! of [`Gen`] nor wrappers_graph.

#[cfg(any(feature = "alloc", feature = "std"))]
pub mod buffer;
pub mod wavetable;
pub mod xorrng;
