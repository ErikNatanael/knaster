use knaster_core::{
    typenum::{U0, U1}, Float, Gen, GenFlags, PFloat, ParameterRange, Parameterable
};

/// Outputs a static number every frame
pub(crate) struct TestNumGen<F> {
    number: F,
}
impl<F: Float> TestNumGen<F> {
    pub fn new(n: F) -> Self {
        Self { number: n }
    }
}
impl<F: Float> Gen for TestNumGen<F> {
    type Sample = F;

    type Inputs = U0;

    type Outputs = U1;

    fn process(
        &mut self,
        _ctx: knaster_core::AudioCtx,
        _flags: &mut GenFlags,
        _input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        [self.number].into()
    }
}
impl<F: Float> Parameterable<F> for TestNumGen<F> {
    type Parameters = U0;

    fn param_descriptions(
    ) -> knaster_core::numeric_array::NumericArray<&'static str, Self::Parameters> {
        [].into()
    }

    fn param_default_values(
    ) -> knaster_core::numeric_array::NumericArray<knaster_core::ParameterValue, Self::Parameters>
    {
        [].into()
    }

    fn param_range(
    ) -> knaster_core::numeric_array::NumericArray<knaster_core::ParameterRange, Self::Parameters>
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
pub(crate) struct TestInPlusParamGen<F> {
    number: F,
}
impl<F: Float> TestInPlusParamGen<F> {
    pub fn new() -> Self {
        Self { number: F::ZERO }
    }
    pub fn set_number(&mut self, n: F) {
        self.number = n;
    }
}
impl<F: Float> Gen for TestInPlusParamGen<F> {
    type Sample = F;

    type Inputs = U1;

    type Outputs = U1;

    fn process(
        &mut self,
        _ctx: knaster_core::AudioCtx,
        _flags: &mut GenFlags,
        input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        [self.number + input[0]].into()
    }
}
impl<F: Float> Parameterable<F> for TestInPlusParamGen<F> {
    type Parameters = U1;

    fn param_descriptions(
    ) -> knaster_core::numeric_array::NumericArray<&'static str, Self::Parameters> {
        ["number"].into()
    }

    fn param_default_values(
    ) -> knaster_core::numeric_array::NumericArray<knaster_core::ParameterValue, Self::Parameters>
    {
        [knaster_core::ParameterValue::Float(0.5)].into()
    }

    fn param_range(
    ) -> knaster_core::numeric_array::NumericArray<knaster_core::ParameterRange, Self::Parameters>
    {
        [ParameterRange::Float(
            PFloat::NEG_INFINITY,
            PFloat::INFINITY,
        )]
        .into()
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
