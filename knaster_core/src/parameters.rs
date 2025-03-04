//! This approach gives us an enum which can apply itself to the exact type, but
//! that is not very useful when the type is erased. It is also not very ergonomic.

mod types;

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
    fn pinteger_range() -> (PInteger, PInteger);
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

#[derive(Copy, Clone, Debug)]
pub enum FloatRange {
    Range(PFloat, PFloat),
    /// Less than `sample_rate/2`. Some filters blow up above this frequency.
    Nyquist,
    Infinite,
    PositiveInfinite,
    NegativeInfinite,
}

#[derive(Copy, Clone, Debug)]
pub enum FloatKind {
    Amplitude,
    Frequency,
}

#[derive(Copy, Clone, Debug)]
pub struct FloatHint {
    pub default: Option<PFloat>,
    pub range: Option<FloatRange>,
    pub kind: Option<FloatKind>,
    pub is_logarithmic: Option<bool>,
}
impl FloatHint {
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
    pub fn minmax(mut self, min: PFloat, max: PFloat) -> Self {
        self.range = Some(FloatRange::Range(min, max));
        self
    }
    pub fn infinite(mut self) -> Self {
        self.range = Some(FloatRange::Infinite);
        self
    }
    pub fn positive_infinite(mut self) -> Self {
        self.range = Some(FloatRange::PositiveInfinite);
        self
    }
    pub fn negative_infinite(mut self) -> Self {
        self.range = Some(FloatRange::NegativeInfinite);
        self
    }
    pub fn nyquist(mut self) -> Self {
        self.range = Some(FloatRange::Nyquist);
        self
    }
    pub fn logarithmic(mut self, b: bool) -> Self {
        self.is_logarithmic = Some(b);
        self
    }
}

impl Default for FloatHint {
    fn default() -> Self {
        Self::new()
    }
}

/// An inclusive range for the supported values of a parameter
#[derive(Copy, Clone, Debug)]
pub enum ParameterHint {
    Float(FloatHint),
    /// Triggers do not have a range
    Trigger,
    Integer(PInteger, PInteger),
}
impl ParameterHint {
    pub fn float(with: impl FnOnce(FloatHint) -> FloatHint) -> Self {
        let hint = with(FloatHint::new());
        Self::Float(hint)
    }
    pub fn from_pinteger<T: PIntegerConvertible>() -> ParameterHint {
        let range = T::pinteger_range();
        ParameterHint::Integer(range.0, range.1)
    }
    // TODO: deprecate these helpers and use the proper syntax
    pub fn nyquist() -> Self {
        Self::float(|h| h.nyquist())
    }
    pub fn infinite_float() -> Self {
        Self::float(|h| h.infinite())
    }
    pub fn positive_infinite_float() -> Self {
        Self::float(|h| h.positive_infinite())
    }
    pub fn negative_infinite_float() -> Self {
        Self::float(|h| h.negative_infinite())
    }
    pub fn one() -> Self {
        Self::float(|h| h.minmax(0.0, 1.))
    }
    pub fn boolean() -> Self {
        ParameterHint::Integer(PInteger(0), PInteger(1))
    }
    pub fn ty(self) -> ParameterType {
        match self {
            ParameterHint::Float(_) => ParameterType::Float,
            ParameterHint::Trigger => ParameterType::Trigger,
            ParameterHint::Integer(_, _) => ParameterType::Integer,
        }
    }
}
impl Default for ParameterHint {
    fn default() -> Self {
        Self::Float(FloatHint::new())
    }
}
