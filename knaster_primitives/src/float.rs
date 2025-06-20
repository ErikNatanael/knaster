use crate::core::{
    f32, f64,
    iter::Sum,
    ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Rem, RemAssign, Sub, SubAssign},
};

#[cfg(not(feature = "unstable"))]
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
    + Sum
    + core::fmt::Debug
    + 'static
{
    const ZERO: Self;
    const ONE: Self;
    const PI: Self;
    const TAU: Self;
    /// A good guess for a number which, when added to a signal as DC, will prevent denormals
    /// without adding any perceivable noise to the output. This value is a compromise, you may
    /// choose a larger value, which allows you to add it less frequently, or a smaller value,
    /// having a smaller effect on the values produced.
    const ANTI_DENORMAL: Self;

    const SIMD_WIDTH: usize = 1;
    fn from_usize(i: usize) -> Self;
    fn new<F: Float>(v: F) -> Self;
    fn to_f32(self) -> f32;
    fn to_f64(self) -> f64;
}

#[cfg(feature = "unstable")]
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
    + Sum
    + core::fmt::Debug
    + core::simd::SimdElement
    + 'static
{
    const ZERO: Self;
    const ONE: Self;
    const PI: Self;
    const TAU: Self;
    /// A good guess for a number which, when added to a signal as DC, will prevent denormals
    /// without adding any perceivable noise to the output. This value is a compromise, you may
    /// choose a larger value, which allows you to add it less frequently, or a smaller value,
    /// having a smaller effect on the values produced.
    const ANTI_DENORMAL: Self;
    const SIMD_WIDTH: usize;

    fn from_usize(i: usize) -> Self;
    fn new<F: Float>(v: F) -> Self;
    fn to_f32(self) -> f32;
    fn to_f64(self) -> f64;
    fn simd_add(a: &[Self], b: &[Self], out: &mut [Self]);
}
impl Float for f32 {
    const ZERO: Self = 0.;
    const ONE: Self = 1.0;
    const PI: Self = f32::consts::PI;
    const TAU: Self = f32::consts::TAU;
    const ANTI_DENORMAL: Self = 1e-20;
    #[cfg(feature = "unstable")]
    const SIMD_WIDTH: usize =
        if let Some(size) = target_features::CURRENT_TARGET.suggested_simd_width::<f32>() {
            size
        } else {
            // If SIMD isn't supported natively, we use a vector of 1 element.
            // This is effectively a scalar value.
            1
        };
    #[cfg(not(feature = "unstable"))]
    const SIMD_WIDTH: usize = 1;

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
    #[cfg(feature = "unstable")]
    #[inline(always)]
    fn simd_add(a: &[Self], b: &[Self], out: &mut [Self]) {
        simd_add_f32(a, b, out);
    }
}
impl Float for f64 {
    const ZERO: Self = 0.;
    const ONE: Self = 1.0;
    const PI: Self = f64::consts::PI;
    const TAU: Self = f64::consts::TAU;
    const ANTI_DENORMAL: Self = 1e-20;
    #[cfg(feature = "unstable")]
    const SIMD_WIDTH: usize =
        if let Some(size) = target_features::CURRENT_TARGET.suggested_simd_width::<f64>() {
            size
        } else {
            // If SIMD isn't supported natively, we use a vector of 1 element.
            // This is effectively a scalar value.
            1
        };
    #[cfg(not(feature = "unstable"))]
    const SIMD_WIDTH: usize = 1;

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
    #[cfg(feature = "unstable")]
    #[inline(always)]
    fn simd_add(a: &[Self], b: &[Self], out: &mut [Self]) {
        simd_add_f64(a, b, out);
    }
}

// The auto-vectorisation is faster than the hand-written SIMD code in benchmarks, so I'm disabling it for now.
// Leaving the implementation here for future optimisation work.
#[cfg(feature = "unstable")]
use core::simd::Simd;
#[cfg(feature = "unstable")]
macro_rules! simd_add {
    ($T:ty, $fn_name:ident) => {
        fn $fn_name(a: &[$T], b: &[$T], out: &mut [$T])
        where
            core::simd::Simd<f32, { <$T>::SIMD_WIDTH }>: core::ops::Add,
        {
            debug_assert!(a.len() == b.len() && a.len() == out.len());
            let elements = a.len();
            let mut i = 0;
            // Process LANE_SIZE elements at a time
            while elements - i >= <$T>::SIMD_WIDTH {
                // Load data into SIMD vectors
                // let a_simd = Simd::<T, { SIMD_WIDTH }>::from_slice(&a[i..i + T::SIMD_WIDTH]);
                // let b_simd = Simd::<T, SIMD_WIDTH>::from_slice(&b[i..i + T::SIMD_WIDTH]);
                let a_simd: Simd<$T, { <$T>::SIMD_WIDTH }> =
                    Simd::from_slice(&a[i..i + <$T>::SIMD_WIDTH]);
                let b_simd: Simd<$T, { <$T>::SIMD_WIDTH }> =
                    Simd::from_slice(&b[i..i + <$T>::SIMD_WIDTH]);

                // Add vectors in a single instruction
                let result = a_simd + b_simd;

                // Store result back to memory
                result.copy_to_slice(&mut out[i..i + <$T>::SIMD_WIDTH]);
                i += <$T>::SIMD_WIDTH;
            }
            for ((a, b), out) in a[i..].iter().zip(b[i..].iter()).zip(out[i..].iter_mut()) {
                *out = *a + *b;
            }
        }
    };
}
#[cfg(feature = "unstable")]
simd_add!(f32, simd_add_f32);
#[cfg(feature = "unstable")]
simd_add!(f64, simd_add_f64);
