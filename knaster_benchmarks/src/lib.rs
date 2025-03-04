use knaster_graph::numeric_array::NumericArray;
use knaster_graph::typenum::{U0, U1};
use knaster_graph::{AudioCtx, Float, Frame, ParameterHint, ParameterValue, UGen, UGenFlags};

pub fn add(signal: &[f32], rhs: f32, output: &mut [f32]) {
    for (sig, out) in signal.iter().zip(output.iter_mut()) {
        *out = sig + rhs;
    }
}
pub fn add_chunked(signal: &[f32], rhs: f32, output: &mut [f32]) {
    for (sig, out) in signal.chunks(8).zip(output.chunks_mut(8)) {
        for (out, sig) in out.iter_mut().zip(sig.iter()) {
            *out = sig + rhs;
        }
    }
    let remaining = signal.len() % 8;
    let skip = signal.len() - remaining;
    for (out, sig) in output.iter_mut().skip(skip).zip(signal.iter().skip(skip)) {
        *out = sig + rhs;
    }
}
/// Outputs a static number every frame
pub struct TestNumUGen<F> {
    number: F,
}
impl<F: Float> TestNumUGen<F> {
    pub fn new(n: F) -> Self {
        Self { number: n }
    }
}
impl<F: Float> UGen for TestNumUGen<F> {
    type Sample = F;

    type Inputs = U0;

    type Outputs = U1;

    fn process(
        &mut self,
        _ctx: AudioCtx,
        _flags: &mut UGenFlags,
        _input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        [self.number].into()
    }
    type Parameters = U0;

    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        [].into()
    }

    fn param_hints() -> NumericArray<ParameterHint, Self::Parameters> {
        [].into()
    }

    fn param_apply(&mut self, _ctx: AudioCtx, _index: usize, _value: ParameterValue) {}
}
