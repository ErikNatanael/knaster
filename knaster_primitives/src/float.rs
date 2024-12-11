use crate::core::{
    f32, f64,
    ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Rem, RemAssign, Sub, SubAssign},
};

pub trait Float:
    num_traits::Float
    + Default
    + Add
    + Sub
    + Mul
    + Div
    + Div<Self>
    + AddAssign
    + SubAssign
    + MulAssign
    + DivAssign
    + Neg
    + Copy
    + Rem
    + RemAssign
    + core::fmt::Debug
    + 'static
{
    const ZERO: Self;
    const ONE: Self;
    const PI: Self;
    const TAU: Self;
    fn from_usize(i: usize) -> Self;
    fn new<F: Float>(v: F) -> Self;
    fn to_f32(self) -> f32;
    fn to_f64(self) -> f64;
}
impl Float for f32 {
    const ZERO: Self = 0.;
    const ONE: Self = 1.0;
    const PI: Self = f32::consts::PI;
    const TAU: Self = f32::consts::TAU;

    fn from_usize(i: usize) -> Self {
        i as Self
    }

    fn new<F: Float>(v: F) -> Self {
        // implementation is actually infallible for all float values
        v.to_f32()
    }

    fn to_f32(self) -> f32 {
        self
    }

    fn to_f64(self) -> f64 {
        self as f64
    }
}
impl Float for f64 {
    const ZERO: Self = 0.;
    const ONE: Self = 1.0;
    const PI: Self = f64::consts::PI;
    const TAU: Self = f64::consts::TAU;

    fn from_usize(i: usize) -> Self {
        i as Self
    }

    fn new<F: Float>(v: F) -> Self {
        v.to_f64()
    }

    fn to_f32(self) -> f32 {
        self as f32
    }

    fn to_f64(self) -> f64 {
        self
    }
}
