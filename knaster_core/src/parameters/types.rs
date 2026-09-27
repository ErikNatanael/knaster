
#[allow(unused)]
use crate::{PInteger, PIntegerConvertible, Rate, UGen};

use super::PTrigger;

/// The type of a [`UGen`] parameter
#[derive(Copy, Clone)]
pub enum ParameterType {
    #[allow(missing_docs)]
    Float,
    #[allow(missing_docs)]
    Trigger,
    #[allow(missing_docs)]
    Integer,
    #[allow(missing_docs)]
    Bool,
    // etc?
}
/// A parameter value of one of the supported parameter types.
///
/// This value is what is sent to the [`UGen`] when setting a parameter.
#[derive(Copy, Clone, Debug)]
pub enum ParameterValue {
    #[allow(missing_docs)]
    Trigger,
    #[allow(missing_docs)]
    Integer(PInteger),
    #[allow(missing_docs)]
    Bool(bool),
    /// The smoothing setting for a Float parameter. Smoothing is not built into all UGens, you generally need a Wrapper to do smoothing for you.
    Smoothing(ControlSmoothing, Rate),
}

impl<T: PIntegerConvertible> From<T> for ParameterValue {
    fn from(val: T) -> Self {
        ParameterValue::Integer(val.into())
    }
}
impl From<PTrigger> for ParameterValue {
    fn from(_val: PTrigger) -> Self {
        ParameterValue::Trigger
    }
}
impl From<bool> for ParameterValue {
    fn from(_val: bool) -> Self {
        ParameterValue::Bool(_val)
    }
}
impl ParameterValue {
    /// Get the parameter value as a PInteger if it is an integer value, otherwise None.
    pub fn integer(self) -> Option<PInteger> {
        match self {
            ParameterValue::Integer(value) => Some(value),
            _ => None,
        }
    }
    /// Get the parameter value as a bool if it is a bool value, otherwise None.
    pub fn bool(self) -> Option<bool> {
        match self {
            ParameterValue::Bool(value) => Some(value),
            _ => None,
        }
    }
    /// Get the type of the parameter value
    pub fn ty(self) -> ParameterType {
        match self {
            ParameterValue::Trigger => ParameterType::Trigger,
            ParameterValue::Integer(_) => ParameterType::Integer,
            ParameterValue::Smoothing(_, _) => ParameterType::Float,
            ParameterValue::Bool(_) => ParameterType::Bool,
        }
    }
}

/// A setting for smoothing of parameter changes.
///
/// Used to interpolate parameter changes over time when the UGen is wrapped in a [`WrSmoothParams`]
#[derive(Copy, Clone, Debug, Default)]
pub enum ControlSmoothing {
    /// No smoothing
    #[default]
    None,
    /// Linear smoothing over the given number of seconds
    Linear(f32),
}
impl From<ControlSmoothing> for ParameterValue {
    fn from(val: ControlSmoothing) -> Self {
        ParameterValue::Smoothing(val, Rate::BlockRate)
    }
}
impl From<(ControlSmoothing, Rate)> for ParameterValue {
    fn from(val: (ControlSmoothing, Rate)) -> Self {
        ParameterValue::Smoothing(val.0, val.1)
    }
}
