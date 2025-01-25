use knaster_primitives::Float;

use crate::{PInteger, PIntegerConvertible, Rate};

use super::{PFloat, Trigger};

#[derive(Copy, Clone)]
pub enum ParameterType {
    Float,
    Trigger,
    Integer,
    // etc?
}
#[derive(Copy, Clone, Debug)]
pub enum ParameterValue {
    Float(PFloat),
    Trigger,
    Integer(PInteger),
    /// The smoothing setting for a Float parameter. Smoothing is not built into all UGens, you generally need a Wrapper to do smoothing for you.
    Smoothing(ParameterSmoothing, Rate),
}

impl<T: PIntegerConvertible> From<T> for ParameterValue {
    fn from(val: T) -> Self {
        ParameterValue::Integer(val.into())
    }
}
impl From<f32> for ParameterValue {
    fn from(val: f32) -> Self {
        ParameterValue::Float(val as PFloat)
    }
}
impl From<f64> for ParameterValue {
    fn from(val: f64) -> Self {
        ParameterValue::Float(val as PFloat)
    }
}
impl From<Trigger> for ParameterValue {
    fn from(_val: Trigger) -> Self {
        ParameterValue::Trigger
    }
}
impl ParameterValue {
    pub fn float(self) -> Option<f64> {
        match self {
            ParameterValue::Float(value) => Some(value),
            _ => None,
        }
    }
    pub fn f<F: Float>(self) -> Option<F> {
        match self {
            ParameterValue::Float(value) => Some(F::new(value)),
            _ => None,
        }
    }
    pub fn integer(self) -> Option<PInteger> {
        match self {
            ParameterValue::Integer(value) => Some(value),
            _ => None,
        }
    }
    pub fn ty(self) -> ParameterType {
        match self {
            ParameterValue::Float(_) => ParameterType::Float,
            ParameterValue::Trigger => ParameterType::Trigger,
            ParameterValue::Integer(_) => ParameterType::Integer,
            ParameterValue::Smoothing(_, _) => ParameterType::Float,
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
pub enum ParameterSmoothing {
    #[default]
    None,
    Linear(f32),
}
impl From<ParameterSmoothing> for ParameterValue {
    fn from(val: ParameterSmoothing) -> Self {
        ParameterValue::Smoothing(val, Rate::BlockRate)
    }
}
impl From<(ParameterSmoothing, Rate)> for ParameterValue {
    fn from(val: (ParameterSmoothing, Rate)) -> Self {
        ParameterValue::Smoothing(val.0, val.1)
    }
}
