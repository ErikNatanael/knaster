//! Wrappers are composable type that wrap aroung a [`Gen`] + [`Parameterable`]
//! to provide some extra functionality.
//!
//! Wrapper types begin by "Wr" by convention to make it easier to spot what's what.

// TODO: Wrapper types that take one value per output channel

mod audio_rate;
mod closure;
mod precise_timing;
pub use closure::*;
pub use precise_timing::*;
mod math;
pub use math::*;

pub use audio_rate::*;
mod smooth_params;
pub use smooth_params::*;

use crate::UGen;

/// Adds methods as shortcuts for adding a range of wrappers_graph to any [`UGen`]
///
/// The methods all take `self`, returning the new wrapper. Math operation
/// wrappers_graph start with `wr_` to disambiguate them from `std::ops::*`
pub trait UGenWrapperCoreExt<T: UGen> {
    /// Apply closure `c` to every sample of every channel of the wrapped UGen, i.e. `c(self)`.
    fn wr<C: FnMut(T::Sample) -> T::Sample + 'static>(self, c: C) -> WrClosure<T, C>;
    /// Multiply the output of the wrapped UGen by `v`, i.e. `self * v`.
    fn wr_mul(self, v: T::Sample) -> WrMul<T>;
    /// Add `v` to the output of the wrapped UGen, i.e. `self + v`.
    fn wr_add(self, v: T::Sample) -> WrAdd<T>;
    /// Subtract `v` from the output of the wrapped UGen, i.e. `self - v`.
    fn wr_sub(self, v: T::Sample) -> WrSub<T>;
    /// Subtract the output of the wrapped UGen from `v`, i.e. `v - self`.
    fn wr_v_sub_gen(self, v: T::Sample) -> WrVSub<T>;
    /// Divide the output of the wrapped UGen by `v`, i.e. `self / v`.
    fn wr_div(self, v: T::Sample) -> WrDiv<T>;
    /// Divide `v` by the output of the wrapped UGen, i.e. `v / self`.
    fn wr_v_div_gen(self, v: T::Sample) -> WrVDiv<T>;
    /// Raise the output of the wrapped UGen to the power of `v`, i.e. `self.powf(v)`.
    fn wr_powf(self, v: T::Sample) -> WrPowf<T>;
    /// Raise the output of the wrapped UGen to the power of `v`, i.e. `self.powi(v)`.
    fn wr_powi(self, v: i32) -> WrPowi<T>;
    /// Enable smoothing/easing functions for float parameters
    fn smooth_params(self) -> WrSmoothParams<T>;
    /// Enable setting a parameter to an audio rate signal
    fn ar_params(self) -> WrArParams<T>;
    /// Precise timing. Requires a generic parameter for how many changes can be handled per block.
    ///
    /// Unless you are sending very frequent changes or are using a huge block size, a low number
    /// will suffice.
    fn precise_timing<const MAX_CHANGES_PER_BLOCK: usize>(
        self,
    ) -> WrPreciseTiming<MAX_CHANGES_PER_BLOCK, T>;
}

impl<T: UGen> UGenWrapperCoreExt<T> for T {
    fn wr_mul(self, v: T::Sample) -> WrMul<T> {
        WrMul::new(self, v)
    }

    fn smooth_params(self) -> WrSmoothParams<T> {
        WrSmoothParams::new(self)
    }

    fn wr_add(self, v: <T as UGen>::Sample) -> WrAdd<T> {
        WrAdd::new(self, v)
    }

    fn wr_sub(self, v: <T as UGen>::Sample) -> WrSub<T> {
        WrSub::new(self, v)
    }

    fn wr_div(self, v: <T as UGen>::Sample) -> WrDiv<T> {
        WrDiv::new(self, v)
    }

    fn wr_powf(self, v: <T as UGen>::Sample) -> WrPowf<T> {
        WrPowf::new(self, v)
    }

    fn wr_v_sub_gen(self, v: <T as UGen>::Sample) -> WrVSub<T> {
        WrVSub::new(self, v)
    }

    fn wr_v_div_gen(self, v: <T as UGen>::Sample) -> WrVDiv<T> {
        WrVDiv::new(self, v)
    }

    fn wr<C: FnMut(<T as UGen>::Sample) -> <T as UGen>::Sample + 'static>(
        self,
        c: C,
    ) -> WrClosure<T, C> {
        WrClosure::new(self, c)
    }

    fn ar_params(self) -> WrArParams<T> {
        WrArParams::new(self)
    }

    fn wr_powi(self, v: i32) -> WrPowi<T> {
        WrPowi::new(self, v)
    }

    fn precise_timing<const MAX_CHANGES_PER_BLOCK: usize>(
        self,
    ) -> WrPreciseTiming<MAX_CHANGES_PER_BLOCK, T> {
        WrPreciseTiming::new(self)
    }
}
