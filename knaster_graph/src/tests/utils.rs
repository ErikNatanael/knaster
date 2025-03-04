use knaster_core::{
    typenum::{U0, U1},
    Float, PFloat, ParameterHint, UGen, UGenFlags,
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
        _ctx: knaster_core::AudioCtx,
        _flags: &mut UGenFlags,
        _input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        [self.number].into()
    }
    type Parameters = U0;

    fn param_descriptions(
    ) -> knaster_core::numeric_array::NumericArray<&'static str, Self::Parameters> {
        [].into()
    }

    fn param_hints(
    ) -> knaster_core::numeric_array::NumericArray<knaster_core::ParameterHint, Self::Parameters>
    {
        [].into()
    }

    fn param_apply(
        &mut self,
        _ctx: knaster_core::AudioCtx,
        _index: usize,
        _value: knaster_core::ParameterValue,
    ) {
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

    fn process(
        &mut self,
        _ctx: knaster_core::AudioCtx,
        _flags: &mut UGenFlags,
        input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        [self.number + input[0]].into()
    }
    type Parameters = U1;

    fn param_descriptions(
    ) -> knaster_core::numeric_array::NumericArray<&'static str, Self::Parameters> {
        ["number"].into()
    }

    fn param_hints(
    ) -> knaster_core::numeric_array::NumericArray<knaster_core::ParameterHint, Self::Parameters>
    {
        [ParameterHint::infinite_float()].into()
    }

    fn param_apply(
        &mut self,
        _ctx: knaster_core::AudioCtx,
        index: usize,
        value: knaster_core::ParameterValue,
    ) {
        if index == 0 {
            self.set_number(F::new(value.float().unwrap()));
        }
    }
}
