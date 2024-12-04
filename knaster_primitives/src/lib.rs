#![cfg_attr(not(feature = "std"), no_std)]
// ^ enables no_std if the `std` features isn't enabled

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
pub use block::*;
mod float;
pub use float::*;
// Reexport typenum and numeric_array because we need to keep it consistent within everything that touches knaster
pub use numeric_array;
pub use numeric_array::typenum;

pub trait Size: ArrayLength + Clone + Sync + Send {}
impl<T: ArrayLength + Clone + Sync + Send> Size for T {}

use numeric_array::{ArrayLength, NumericArray};

pub type Frame<T, Size> = NumericArray<T, Size>;
