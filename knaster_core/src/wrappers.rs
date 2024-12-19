//! Wrappers are composable type that wrap aroung a [`Gen`] + [`Parameterable`]
//! to provide some extra functionality.
//!
//!
//!
//! Wrapper types begin by "Wr" by convention to make it easier to spot what's what.

// TODO: Wrapper types that take one value per output channel

mod audio_rate;
mod closure;
pub use closure::*;
mod math;
pub use math::*;

pub use audio_rate::*;
mod smooth_params;
pub use smooth_params::*;

use crate::{Gen, Parameterable};

/// Adds methods as shortcuts for adding a range of wrappers to any `Gen + Parameterable`
///
/// The methods all take `self`, returning the new wrapper. Math operation
/// wrappers start with `wr_` to disambiguate them from `std::ops::*`
pub trait GenWrapperExt<T: Gen + Parameterable<T::Sample>> {
    fn wr<C: FnMut(T::Sample) -> T::Sample + 'static>(self, c: C) -> WrClosure<T, C>;
    fn wr_mul(self, v: T::Sample) -> WrMul<T>;
    fn wr_add(self, v: T::Sample) -> WrAdd<T>;
    fn wr_sub(self, v: T::Sample) -> WrSub<T>;
    fn wr_v_sub_gen(self, v: T::Sample) -> WrVSubGen<T>;
    fn wr_div(self, v: T::Sample) -> WrDiv<T>;
    fn wr_v_div_gen(self, v: T::Sample) -> WrVDivGen<T>;
    fn wr_powf(self, v: T::Sample) -> WrPowf<T>;
    fn wr_powi(self, v: i32) -> WrPowi<T>;
    /// Enable smoothing/easing functions for float parameters
    fn smooth_params(self) -> WrSmoothParams<T>;
    /// Enable setting a parameter to an audio rate signal
    fn ar_params(self) -> WrArParams<T>;
}

impl<T: Gen + Parameterable<T::Sample>> GenWrapperExt<T> for T {
    fn wr_mul(self, v: T::Sample) -> WrMul<T> {
        WrMul::new(self, v)
    }

    fn smooth_params(self) -> WrSmoothParams<T> {
        WrSmoothParams::new(self)
    }

    fn wr_add(self, v: <T as Gen>::Sample) -> WrAdd<T> {
        WrAdd::new(self, v)
    }

    fn wr_sub(self, v: <T as Gen>::Sample) -> WrSub<T> {
        WrSub::new(self, v)
    }

    fn wr_div(self, v: <T as Gen>::Sample) -> WrDiv<T> {
        WrDiv::new(self, v)
    }

    fn wr_powf(self, v: <T as Gen>::Sample) -> WrPowf<T> {
        WrPowf::new(self, v)
    }

    fn wr_v_sub_gen(self, v: <T as Gen>::Sample) -> WrVSubGen<T> {
        WrVSubGen::new(self, v)
    }

    fn wr_v_div_gen(self, v: <T as Gen>::Sample) -> WrVDivGen<T> {
        WrVDivGen::new(self, v)
    }

    fn wr<C: FnMut(<T as Gen>::Sample) -> <T as Gen>::Sample + 'static>(
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
}
