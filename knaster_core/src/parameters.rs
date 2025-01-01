//! This approach gives us an enum which can apply itself to the exact type, but
//! that is not very useful when the type is erased. It is also not very ergonomic.

mod types;

use std::f32::consts::PI;
pub use types::*;

use thiserror::Error;

// The current type of parameter changes. It is set here to easily change it in the future.
// It would be more robust to make this a newtype, since it avoids the risk that code uses the concrete type instead of the type alias, but the cost to ergonomics is significant.
pub type PFloat = f64;

/// A parameter trigger value
///
/// Similar to a Bang in PureData in that it's a separate type value that triggers something to happen. It is unlike Bang in that Gen's only receive `Trigger`s on specific dedicated parameter indices, whereas `inlet`s in PureData often accept multiple types of parameters.
#[derive(Copy, Clone, Debug)]
pub struct Trigger;

/// A parameter integer type backed by a usize.
///
/// Many types, such as [`Done`], can be encoded as a [`PInteger`] when setting a parameter. To
/// enable this functionality for your own type, implement [`PIntegerConvertible`] for it.
#[derive(Copy, Clone, Debug)]
pub struct PInteger(pub usize);

/// Implement to ergonomically send a value as a parameter change through a PInteger
pub trait PIntegerConvertible: From<PInteger> + Into<PInteger> {
    // fn to_pinteger(self) -> PInteger;
    // fn from_pinteger(val: PInteger) -> Self;
    fn pinteger_range() -> (PInteger, PInteger) ;
}
impl From<PInteger> for usize {
    fn from(val: PInteger) -> Self {
        val.0
    }
}
impl From<usize> for PInteger {
    fn from(val: usize) -> Self {
        PInteger(val)
    }
}
impl PIntegerConvertible for usize {
    // fn to_pinteger(self) -> PInteger {
    //     PInteger(self)
    // }
    //
    // fn from_pinteger(val: PInteger) -> Self {
    //     val.0
    // }

    fn pinteger_range() -> (PInteger, PInteger) {
        (PInteger(usize::MIN), PInteger(usize::MAX))
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

/// An inclusive range for the supported values of a parameter
#[derive(Copy, Clone)]
pub enum ParameterRange {
    Float(PFloat, PFloat),
    Trigger,
    Integer(PInteger, PInteger),
}
impl ParameterRange {

    pub fn from_pinteger<T: PIntegerConvertible>() -> ParameterRange {
        let range = T::pinteger_range();
        ParameterRange::Integer(range.0, range.1)
    }
    pub fn ty(self) -> ParameterType {
        match self {
            ParameterRange::Float(_, _) => ParameterType::Float,
            ParameterRange::Trigger => ParameterType::Trigger,
            ParameterRange::Integer(_, _) => ParameterType::Integer,
        }
    }
}
impl Default for ParameterRange {
    fn default() -> Self {
        Self::Float(PFloat::NEG_INFINITY, PFloat::INFINITY)
    }
}
