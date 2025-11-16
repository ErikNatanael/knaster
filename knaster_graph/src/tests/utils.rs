use knaster_core::{AudioCtx, Float, ParameterHint, UGen, UGenFlags, impl_ugen, typenum::U1};

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

    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        [self.number + input[0]].into()
    }
    type FloatParameters = U1;

    fn param_descriptions()
    -> knaster_core::numeric_array::NumericArray<&'static str, Self::FloatParameters> {
        ["number"].into()
    }

    fn param_hints()
    -> knaster_core::numeric_array::NumericArray<knaster_core::ParameterHint, Self::FloatParameters>
    {
        [ParameterHint::infinite_float()].into()
    }

    fn param_apply(
        &mut self,
        _ctx: &mut AudioCtx,
        index: usize,
        value: knaster_core::ParameterValue,
    ) {
        if index == 0 {
            self.set_number(F::new(value.float().unwrap()));
        }
    }
}
