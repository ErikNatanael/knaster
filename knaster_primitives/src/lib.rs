#![no_std]

//! # Knaster Primitives
//!
//! This crate contains the building blocks for the knaster audio framework.
//!
//! ## no_std
//!
//! knaster_primitives is no_std with an optional alloc feature.
//!
//! ## Features
//!
//! - `alloc`: Enables a heap based implementation of [`Block`]: [`VecBlock`]
//! - `std`: Enables std, disabling no_std. Takes precedence over `alloc`

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "alloc")]
extern crate alloc;

// Switches between std and core based on features. This reduces boilerplate when importing.
mod core {
    #[cfg(not(feature = "std"))]
    pub use core::*;
    #[cfg(feature = "std")]
    pub use std::*;
}

// Reexport to not make the structure part of the public API and to reduce noise in paths.
mod block;
mod parameters;
mod time;
pub use block::*;
pub use parameters::*;
pub use time::*;
mod float;
pub use float::*;
// Reexport typenum and numeric_array because we need to keep it consistent within everything that touches knaster
pub use num_traits::Float as FloatMethods;
pub use numeric_array;
pub use numeric_array::typenum;

pub use num_derive;
pub use num_traits;

pub trait Size: ArrayLength + Clone + Copy + Sync + Send {}
impl<T: ArrayLength + Clone + Sync + Send> Size for T {}

use numeric_array::{ArrayLength, NumericArray};

pub type Frame<T, Size> = NumericArray<T, Size>;
