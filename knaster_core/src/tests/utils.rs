use crate::{
    typenum::{U0, U1}, AudioCtx, Float, ParameterHint, UGen, UGenFlags
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
        _input: crate::Frame<Self::Sample, Self::Inputs>,
    ) -> crate::Frame<Self::Sample, Self::Outputs> {
        [self.number].into()
    }
    type Parameters = U0;

    fn param_descriptions() -> crate::numeric_array::NumericArray<&'static str, Self::Parameters> {
        [].into()
    }

    fn param_hints() -> crate::numeric_array::NumericArray<crate::ParameterHint, Self::Parameters> {
        [].into()
    }

    fn param_apply(&mut self, _ctx: &mut AudioCtx, _index: usize, _value: crate::ParameterValue) {
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
impl<F: Float> UGen for TestInPlusParamGen<F> {
    type Sample = F;

    type Inputs = U1;

    type Outputs = U1;

    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        input: crate::Frame<Self::Sample, Self::Inputs>,
    ) -> crate::Frame<Self::Sample, Self::Outputs> {
        [self.number + input[0]].into()
    }
    type Parameters = U1;

    fn param_descriptions() -> crate::numeric_array::NumericArray<&'static str, Self::Parameters> {
        ["number"].into()
    }

    fn param_hints() -> crate::numeric_array::NumericArray<crate::ParameterHint, Self::Parameters> {
        [ParameterHint::infinite_float()].into()
    }

    fn param_apply(&mut self, _ctx: &mut AudioCtx, index: usize, value: crate::ParameterValue) {
        if index == 0 {
            self.set_number(F::new(value.float().unwrap()));
        }
    }
}
