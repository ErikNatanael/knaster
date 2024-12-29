//! This approach gives us an enum which can apply itself to the exact type, but
//! that is not very useful when the type is erased. It is also not very ergonomic.

mod types;
pub use types::*;

use thiserror::Error;

pub type PFloat = f64;

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
}
impl ParameterRange {
    pub fn ty(self) -> ParameterType {
        match self {
            ParameterRange::Float(_, _) => ParameterType::Float,
            ParameterRange::Trigger => ParameterType::Trigger,
            ParameterRange::Index(_, _) => ParameterType::Index,
        }
    }
}
#[derive(Copy, Clone, Debug)]
pub struct Trigger;
