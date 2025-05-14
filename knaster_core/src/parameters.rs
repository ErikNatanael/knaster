//! This approach gives us an enum which can apply itself to the exact type, but
//! that is not very useful when the type is erased. It is also not very ergonomic.

mod types;

use knaster_primitives::{FloatParameterKind, FloatParameterRange, PFloat};
pub use types::*;

use std::prelude::v1::*;

use thiserror::Error;

/// A parameter trigger value
///
/// Similar to a Bang in PureData in that it's a separate type value that triggers something to happen. It is unlike Bang in that UGens only receive `Trigger`s on specific dedicated parameter indices, whereas `inlet`s in PureData often accept multiple types of parameters.
#[derive(Copy, Clone, Debug)]
pub struct PTrigger;

/// A parameter integer type backed by a usize.
///
/// Many types, such as [`Done`], can be encoded as a [`PInteger`] when setting a parameter. To
/// enable this functionality for your own type, implement [`PIntegerConvertible`] for it.
#[derive(Copy, Clone, Debug)]
pub struct PInteger(pub usize);
impl PInteger {
    pub const MAX: Self = PInteger(usize::MAX);
    pub const MIN: Self = PInteger(usize::MIN);
    pub const ZERO: Self = PInteger(0);
}

/// Implement to ergonomically send a value as a parameter change through a PInteger
pub trait PIntegerConvertible: From<PInteger> + Into<PInteger> {
    // fn to_pinteger(self) -> PInteger;
    // fn from_pinteger(val: PInteger) -> Self;
    fn pinteger_range() -> (PInteger, PInteger);
    fn pinteger_descriptions(v: PInteger) -> Option<&'static str>;
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
    fn pinteger_range() -> (PInteger, PInteger) {
        (PInteger(usize::MIN), PInteger(usize::MAX))
    }

    fn pinteger_descriptions(v: PInteger) -> Option<&'static str> {
        None
    }
}
impl From<PInteger> for bool {
    fn from(val: PInteger) -> Self {
        val.0 > 0
    }
}
impl From<bool> for PInteger {
    fn from(val: bool) -> Self {
        PInteger(if val { 1 } else { 0 })
    }
}
impl PIntegerConvertible for bool {
    fn pinteger_range() -> (PInteger, PInteger) {
        (PInteger(0), PInteger(1))
    }

    fn pinteger_descriptions(v: PInteger) -> Option<&'static str> {
        Some(if v.0 > 0 { "True" } else { "False" })
    }
}

#[derive(Debug, Clone, Error)]
pub enum ParameterError {
    #[error(
        "Description parameters are not supported in this contect. Please use the parameter index instead. Description: `{0}`"
    )]
    DescriptionNotSupported(&'static str),
    #[error("The parameter description `{0}` does not match any parameter on this `UGen`")]
    DescriptionNotFound(&'static str),
    #[error("You are trying to set a parameter to a type it does not support.")]
    WrongParameterType,
    #[error("The parameter index is out of bounds.")]
    ParameterIndexOutOfBounds,
    #[error(
        "The graph within which the node you are trying to set parameters for does not exist anymore."
    )]
    GraphWasFreed,
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

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PFloatHint {
    pub default: Option<PFloat>,
    pub range: Option<FloatParameterRange>,
    pub kind: Option<FloatParameterKind>,
    pub is_logarithmic: Option<bool>,
}
impl PFloatHint {
    pub fn new() -> Self {
        Self {
            default: None,
            range: None,
            kind: None,
            is_logarithmic: None,
        }
    }
    pub fn default(mut self, v: PFloat) -> Self {
        self.default = Some(v);
        self
    }
    pub fn range(mut self, range: impl Into<FloatParameterRange>) -> Self {
        self.range = Some(range.into());
        self
    }
    pub fn kind(mut self, kind: FloatParameterKind) -> Self {
        self.kind = Some(kind);
        self
    }
    pub fn minmax(mut self, min: PFloat, max: PFloat) -> Self {
        self.range = Some(FloatParameterRange::Range(min, max));
        self
    }
    pub fn infinite(mut self) -> Self {
        self.range = Some(FloatParameterRange::Infinite);
        self
    }
    pub fn unipolar(mut self) -> Self {
        self.range = Some(FloatParameterRange::Range(0.0, 1.0));
        self
    }
    pub fn positive_infinite(mut self) -> Self {
        self.range = Some(FloatParameterRange::PositiveInfinite);
        self
    }
    pub fn negative_infinite(mut self) -> Self {
        self.range = Some(FloatParameterRange::NegativeInfinite);
        self
    }
    pub fn nyquist(mut self) -> Self {
        self.range = Some(FloatParameterRange::Nyquist);
        self
    }
    pub fn logarithmic(mut self, b: bool) -> Self {
        self.is_logarithmic = Some(b);
        self
    }
}

impl Default for PFloatHint {
    fn default() -> Self {
        Self::new()
    }
}

/// Hint metadata for a `PInteger` parameter. Hints are useful for accessibility, debugging and
/// GUIs
#[derive(Copy, Clone, Debug)]
pub struct PIntegerHint {
    pub default: Option<PInteger>,
    pub range: (PInteger, PInteger),
    value_descriptions: Option<fn(PInteger) -> Option<&'static str>>,
}
impl PIntegerHint {
    pub fn new(range: (PInteger, PInteger)) -> Self {
        Self {
            default: None,
            range,
            value_descriptions: None,
        }
    }
    pub fn description(&self, v: PInteger) -> Option<&'static str> {
        self.value_descriptions.and_then(|func| func(v))
    }
    #[cfg(any(feature = "std", feature = "alloc"))]
    /// Returns descriptions of the values for this parameter.
    pub fn descriptions(&self) -> Option<crate::core::vec::Vec<(PInteger, String)>> {
        self.value_descriptions.map(|func| {
            (self.range.0.0..self.range.1.0)
                .map(|pi| {
                    (
                        PInteger(pi),
                        func(PInteger(pi)).map_or_else(|| pi.to_string(), |s| s.to_string()),
                    )
                })
                .collect()
        })
    }
}

/// An inclusive range for the supported values of a parameter
#[derive(Copy, Clone, Debug)]
pub enum ParameterHint {
    Float(PFloatHint),
    /// Triggers do not have a range
    Trigger,
    Integer(PIntegerHint),
}
impl ParameterHint {
    pub fn new_float(with: impl FnOnce(PFloatHint) -> PFloatHint) -> Self {
        let hint = with(PFloatHint::new());
        Self::Float(hint)
    }
    /// Manually set the hints for an integer parameter
    pub fn new_integer(
        range: (PInteger, PInteger),
        with: impl FnOnce(PIntegerHint) -> PIntegerHint,
    ) -> Self {
        let hint = with(PIntegerHint::new(range));
        Self::Integer(hint)
    }
    /// Set hint values from a value that can be converted to a `PInteger`
    pub fn from_pinteger_enum<T: PIntegerConvertible + Default>() -> ParameterHint {
        ParameterHint::Integer(PIntegerHint {
            default: Some(T::default().into()),
            range: T::pinteger_range(),
            value_descriptions: Some(T::pinteger_descriptions),
        })
    }
    // TODO: deprecate these helpers and use the proper syntax
    #[deprecated]
    pub fn nyquist() -> Self {
        Self::new_float(|h| h.nyquist())
    }
    pub fn infinite_float() -> Self {
        Self::new_float(|h| h.infinite())
    }
    pub fn positive_infinite_float() -> Self {
        Self::new_float(|h| h.positive_infinite())
    }
    pub fn negative_infinite_float() -> Self {
        Self::new_float(|h| h.negative_infinite())
    }
    pub fn one() -> Self {
        Self::new_float(|h| h.minmax(0.0, 1.))
    }
    pub fn boolean() -> Self {
        Self::from_pinteger_enum::<bool>()
    }
    pub fn ty(self) -> ParameterType {
        match self {
            ParameterHint::Float(_) => ParameterType::Float,
            ParameterHint::Trigger => ParameterType::Trigger,
            ParameterHint::Integer(_) => ParameterType::Integer,
        }
    }
    pub fn float_hint(&self) -> Option<&PFloatHint> {
        match self {
            ParameterHint::Float(hint) => Some(hint),
            _ => None,
        }
    }
    pub fn integer_hint(&self) -> Option<&PIntegerHint> {
        match self {
            ParameterHint::Integer(hint) => Some(hint),
            _ => None,
        }
    }
}
impl Default for ParameterHint {
    fn default() -> Self {
        Self::Float(PFloatHint::new())
    }
}
