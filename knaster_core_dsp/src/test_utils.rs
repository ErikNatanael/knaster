use knaster_core::{
    AudioCtx, ControlRange, Float, Frame, ParameterHint, ParameterValue, UGen, UGenFlags,
    controls::ControlMetadata,
    numeric_array::NumericArray,
    typenum::{U0, U1},
};

/// Outputs a static number every frame
pub(crate) struct TestNumUGen<F> {
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
        _ctx: &mut AudioCtx,
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

    fn param_apply(&mut self, _ctx: &mut AudioCtx, _index: usize, _value: ParameterValue) {}

    type Controls = U0;

    fn control_metadata(&self) -> NumericArray<ControlMetadata<Self::Sample>, Self::Controls> {
        [].into()
    }

    fn control(&mut self, _ctx: &mut AudioCtx, _index: usize, _value: Self::Sample) {}
}

/// Outputs a static number every frame
pub(crate) struct TestInPlusParamGen<F> {
    number: F,
}
impl<F: Float> TestInPlusParamGen<F> {
    pub fn new() -> Self {
        Self { number: F::ZERO }
    }
    pub fn number(&mut self, n: F) {
        self.number = n;
    }
}
impl<F: Float> UGen for TestInPlusParamGen<F> {
    type Sample = F;

    type Inputs = U1;

    type Outputs = U1;

    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        [self.number + input[0]].into()
    }
    type Parameters = U0;

    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        [].into()
    }

    fn param_hints() -> NumericArray<ParameterHint, Self::Parameters> {
        [].into()
    }

    fn param_apply(&mut self, _ctx: &mut AudioCtx, _index: usize, _value: ParameterValue) {}

    type Controls = U1;

    fn control_metadata(
        &self,
    ) -> NumericArray<knaster_core::controls::ControlMetadata<Self::Sample>, Self::Controls> {
        NumericArray::from([ControlMetadata::new(
            "number",
            F::ZERO,
            ControlRange::Infinity,
        )])
    }
    fn control(&mut self, _ctx: &mut AudioCtx, index: usize, value: Self::Sample) {
        if index == 0 {
            self.number(value)
        }
    }
}
