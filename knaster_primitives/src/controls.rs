use crate::{Float, PFloat};

/// Hint for acceptable ranges of controls.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ControlRange<F: Float> {
    /// Inclusive range (min, max).
    RangeInclusive(F, F),
    /// Less than `sample_rate/2`. Some filters blow up above this frequency.
    Nyquist,
    #[allow(missing_docs)]
    Infinity,
    #[allow(missing_docs)]
    PositiveInfinity,
    #[allow(missing_docs)]
    NegativeInfinity,
}

impl<F: Float> ControlRange<F> {
    /// Convert float type from F to PFloat
    pub fn to_pfloat(self) -> ControlRange<PFloat> {
        match self {
            ControlRange::RangeInclusive(min, max) => {
                ControlRange::RangeInclusive(min.to_f64(), max.to_f64())
            }
            ControlRange::Nyquist => ControlRange::Nyquist,
            ControlRange::Infinity => ControlRange::Infinity,
            ControlRange::PositiveInfinity => ControlRange::PositiveInfinity,
            ControlRange::NegativeInfinity => ControlRange::NegativeInfinity,
        }
    }
}

/// Description of the purpose or kind of a control.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ControlPurpose {
    #[allow(missing_docs)]
    Frequency,
    #[allow(missing_docs)]
    Phase,
    #[allow(missing_docs)]
    Q,
    #[allow(missing_docs)]
    ReciprocalQ,
    #[allow(missing_docs)]
    Amplitude,
    #[allow(missing_docs)]
    Decibel,
    #[allow(missing_docs)]
    Offset,
    #[allow(missing_docs)]
    Seconds,
    #[allow(missing_docs)]
    Pan,
}
