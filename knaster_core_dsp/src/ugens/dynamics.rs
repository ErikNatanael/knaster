//! Dynamics
//!
//! [`UGen`]s for dyanamics control (limiters, compressors, etc.)
use core::marker::PhantomData;

#[allow(unused)]
use knaster_core::UGen;
use knaster_core::{AudioCtx, Float, UGenFlags, impl_ugen};

/// Safety limiter
///
/// - clamps values to (-1.0, 1.0)
/// - replaces NaN values by 0.0
pub struct SafetyLimiter<F: Float> {
    _float: PhantomData<F>,
}
#[impl_ugen]
impl<F: Float> SafetyLimiter<F> {
    #[allow(clippy::new_without_default)]
    #[allow(missing_docs)]
    pub fn new() -> Self {
        Self {
            _float: PhantomData,
        }
    }
    fn process(&mut self, _ctx: &mut AudioCtx, _flags: &mut UGenFlags, input: [F; 1]) -> [F; 1] {
        let s = input[0];
        let s = s.clamp(F::new(-1.0), F::new(1.0));
        let s = if s.is_nan() { F::new(0.0) } else { s };
        [s]
    }
}
