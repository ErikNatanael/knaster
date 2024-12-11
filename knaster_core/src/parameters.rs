//! This approach gives us an enum which can apply itself to the exact type, but
//! that is not very useful when the type is erased. It is also not very ergonomic.

mod types;
pub use types::*;

use thiserror::Error;

use knaster_primitives::numeric_array::NumericArray;
use knaster_primitives::typenum::*;
use knaster_primitives::Size;

use crate::AudioCtx;

pub type PFloat = f64;

/// This is the interface to modifying parameters of a [`Gen`].
///
/// A wrapper will pass on parameters to an inner type, unless it is a wrapper
/// with special treatment of parameters.
pub trait Parameterable<F> {
    type Parameters: Size;
    fn param_types() -> NumericArray<ParameterType, Self::Parameters> {
        Self::param_default_values()
            .into_iter()
            .map(|v| v.ty())
            .collect()
    }
    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters>;
    fn param_default_values() -> NumericArray<ParameterValue, Self::Parameters>;
    fn param_range() -> NumericArray<ParameterRange, Self::Parameters>;
    /// Meant for internal use. See Parameterable::param for a safer public interface.
    ///
    /// Tries to apply the parameter change without checking the validity of the
    /// values. May panic or do nothing given unexpected values.
    fn param_apply(&mut self, ctx: &AudioCtx, index: usize, value: ParameterValue);
    /// Set an audio buffer to control a parameter. Does nothing unless an
    /// `ArParams` wrapper or alternative wrapper making use of this value wraps the Gen.
    ///
    /// There is little use in calling this directly unless you are implementing
    /// a graph. If you are not using a graph, using the ArParams wrapper and
    /// this function is equivalent to running frame-by-frame and setting the
    /// value every frame.
    ///
    /// # Safety:
    /// The caller guarantees that the pointer will point to a
    /// contiguous allocation of at least block size until it is replaced,
    /// disabled, or the inner struct is dropped.
    #[allow(unused)]
    unsafe fn param_set_ar_param_buffer(&mut self, index: usize, buffer: *const F) {}
    /// Apply a parameter change. Typechecks and bounds checks the arguments and
    /// provides sensible errors. Calls [`Parameterable::param_apply`] under the hood.
    fn param(
        &mut self,
        ctx: &AudioCtx,
        param: impl Into<Param>,
        value: impl Into<ParameterValue>,
    ) -> Result<(), ParameterError> {
        match param.into() {
            Param::Index(i) => {
                if i >= Self::Parameters::USIZE {
                    return Err(ParameterError::ParameterIndexOutOfBounds);
                }
                self.param_apply(ctx, i, value.into());
                Ok(())
            }
            Param::Desc(desc) => {
                for (i, d) in Self::param_descriptions().into_iter().enumerate() {
                    if d == desc {
                        self.param_apply(ctx, i, value.into());
                        return Ok(());
                    }
                }
                Err(ParameterError::DescriptionNotFound(desc))
            }
        }
    }
}
#[derive(Debug, Clone, Error)]
pub enum ParameterError {
    #[error("The parameter description `{0}` does not match any parameter on this `Gen`")]
    DescriptionNotFound(&'static str),
    #[error("You are trying to set a parameter to a type it does not support.")]
    WrongParameterType,
    #[error("The parameter index is out of bounds.")]
    ParameterIndexOutOfBounds,
}
#[derive(Debug, Clone, Copy)]
pub enum Param {
    Index(usize),
    Desc(&'static str),
}
impl From<usize> for Param {
    fn from(val: usize) -> Self {
        Param::Index(val)
    }
}
impl From<&'static str> for Param {
    fn from(val: &'static str) -> Self {
        Param::Desc(val)
    }
}

#[derive(Copy, Clone)]
pub enum ParameterRange {
    Float(PFloat, PFloat),
    Trigger,
    Index(usize, usize),
    // etc?
}
#[derive(Copy, Clone, Debug)]
pub struct Trigger;
// #[derive(Copy, Clone, Debug)]
// pub struct GraphContext {
//     pub sample_rate: u32,
//     pub block_size: u32,
// }
