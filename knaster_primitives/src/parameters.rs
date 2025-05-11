// The current type of parameter changes. It is set here to easily change it in the future.
// It would be more robust to make this a newtype, since it avoids the risk that code uses the concrete type instead of the type alias, but the cost to ergonomics is significant.
pub type PFloat = f64;

#[derive(Copy, Clone, Debug)]
pub enum FloatParameterRange {
    Range(PFloat, PFloat),
    /// Less than `sample_rate/2`. Some filters blow up above this frequency.
    Nyquist,
    Infinite,
    PositiveInfinite,
    NegativeInfinite,
}

#[derive(Copy, Clone, Debug)]
pub enum FloatParameterKind {
    Amplitude,
    Frequency,
    Q,
}
