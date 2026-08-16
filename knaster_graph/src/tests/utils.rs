use anyhow::anyhow;
use knaster_core::{
    AudioCtx, BlockRead, ControlRange, Float, ParameterHint, UGen, UGenFlags,
    controls::ControlMetadata,
    impl_ugen,
    typenum::{U0, U1},
};

use crate::processor::AudioProcessor;

/// Outputs a static number every frame
pub(crate) struct TestNumUGen<F> {
    number: F,
}
#[impl_ugen]
impl<F: Float> TestNumUGen<F> {
    pub fn new(n: F) -> Self {
        Self { number: n }
    }
    fn process(&mut self) -> [F; 1] {
        [self.number]
    }
}

/// Outputs a static number every frame
pub(crate) struct TestInPlusParamUGen<F> {
    number: F,
}
impl<F: Float> TestInPlusParamUGen<F> {
    pub fn new() -> Self {
        Self { number: F::ZERO }
    }
    pub fn set_number(&mut self, n: F) {
        self.number = n;
    }
}
impl<F: Float> UGen for TestInPlusParamUGen<F> {
    type Sample = F;
    type Inputs = U1;
    type Outputs = U1;
    type Parameters = U0;
    type Controls = U1;

    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        [self.number + input[0]].into()
    }

    fn param_descriptions()
    -> knaster_core::numeric_array::NumericArray<&'static str, Self::Parameters> {
        [].into()
    }

    fn param_hints()
    -> knaster_core::numeric_array::NumericArray<knaster_core::ParameterHint, Self::Parameters>
    {
        [].into()
    }

    fn param_apply(
        &mut self,
        _ctx: &mut AudioCtx,
        _index: usize,
        _value: knaster_core::ParameterValue,
    ) {
    }

    fn control_metadata(
        &self,
    ) -> knaster_core::numeric_array::NumericArray<
        knaster_core::controls::ControlMetadata<Self::Sample>,
        Self::Controls,
    > {
        [ControlMetadata::new(
            "number",
            self.number,
            ControlRange::Infinity,
        )]
        .into()
    }

    fn control(&mut self, _ctx: &mut AudioCtx, index: usize, value: Self::Sample) {
        if index == 0 {
            self.set_number(value);
        }
    }
}

pub(super) fn assert_next_frame_eq_0_0<F: Float>(
    audio_processor: &mut AudioProcessor<F>,
    expected: F,
) -> Result<(), anyhow::Error> {
    audio_processor.run_without_inputs();
    let output = audio_processor.output_block();
    let frame = output.read(0, 0);
    if frame == expected {
        Ok(())
    } else {
        Err(anyhow!(
            "Expected first sample in block {expected:?}, got {frame:?}"
        ))
    }
}
