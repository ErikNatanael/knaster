use core::ops::{RangeFrom, RangeInclusive, RangeTo, RangeToInclusive};

// The current type of parameter changes. It is set here to easily change it in the future.
// It would be more robust to make this a newtype, since it avoids the risk that code uses the concrete type instead of the type alias, but the cost to ergonomics is significant.
pub type PFloat = f64;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum FloatParameterRange {
    Range(PFloat, PFloat),
    /// Less than `sample_rate/2`. Some filters blow up above this frequency.
    Nyquist,
    Infinite,
    PositiveInfinite,
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum FloatParameterKind {
    Amplitude,
    Frequency,
    Q,
    Seconds,
}
