use core::ops::{RangeFrom, RangeInclusive, RangeTo, RangeToInclusive};

// The current type of parameter changes. It is set here to easily change it in the future.
// It would be more robust to make this a newtype, since it avoids the risk that code uses the concrete type instead of the type alias, but the cost to ergonomics is significant.
/// The float type used for parameter values.
pub type PFloat = f64;

/// Hint for acceptable ranges of float parameters.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FloatParameterRange {
    /// Inclusive range (min, max).
    Range(PFloat, PFloat),
    /// Less than `sample_rate/2`. Some filters blow up above this frequency.
    Nyquist,
    #[allow(missing_docs)]
    Infinite,
    #[allow(missing_docs)]
    PositiveInfinite,
    #[allow(missing_docs)]
    NegativeInfinite,
}
impl From<RangeInclusive<PFloat>> for FloatParameterRange {
    fn from(range: RangeInclusive<PFloat>) -> Self {
        FloatParameterRange::Range(*range.start(), *range.end())
    }
}
impl From<RangeFrom<PFloat>> for FloatParameterRange {
    fn from(range: RangeFrom<PFloat>) -> Self {
        FloatParameterRange::Range(range.start, PFloat::MAX)
    }
}
impl From<RangeTo<PFloat>> for FloatParameterRange {
    fn from(range: RangeTo<PFloat>) -> Self {
        FloatParameterRange::Range(PFloat::MIN, range.end)
    }
}
impl From<RangeToInclusive<PFloat>> for FloatParameterRange {
    fn from(range: RangeToInclusive<PFloat>) -> Self {
        FloatParameterRange::Range(PFloat::MIN, range.end)
    }
}

/// A specific kind of float parameter, used for hinting to GUIs and similar.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FloatParameterKind {
    #[allow(missing_docs)]
    Amplitude,
    #[allow(missing_docs)]
    Frequency,
    #[allow(missing_docs)]
    Q,
    #[allow(missing_docs)]
    Seconds,
}
