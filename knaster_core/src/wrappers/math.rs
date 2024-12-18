use knaster_primitives::FloatMethods;

use crate::{Gen, Parameterable};

// TODO: SIMD implementations for blocks
// TODO: SIMD implementations for multi channel frame by frame outputs
// TODO: min, max, powi, sqrt, exp, exp2, abs, range/mul_add, cbrt, tanh

/// `gen` * `value`
pub struct WrMul<T: Gen + Parameterable<T::Sample>> {
    gen: T,
    value: T::Sample,
}
impl<T: Gen + Parameterable<T::Sample>> WrMul<T> {
    pub fn new(gen: T, value: T::Sample) -> Self {
        Self { gen, value }
    }
}
impl<T: Gen + Parameterable<T::Sample>> Gen for WrMul<T> {
    type Sample = T::Sample;
    type Inputs = T::Inputs;
    type Outputs = T::Outputs;

    fn process(
        &mut self,
        ctx: &mut crate::AudioCtx,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.gen.process(ctx, input);
        for sample in &mut out {
            *sample *= self.value;
        }
        out
    }
}

impl<T: Gen + Parameterable<T::Sample>> Parameterable<T::Sample> for WrMul<T> {
    type Parameters = T::Parameters;

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_default_values(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterValue, Self::Parameters>
    {
        T::param_default_values()
    }

    fn param_range(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterRange, Self::Parameters>
    {
        T::param_range()
    }

    fn param_apply(&mut self, ctx: &crate::AudioCtx, index: usize, value: crate::ParameterValue) {
        T::param_apply(&mut self.gen, ctx, index, value)
    }
}

/// `gen` + `value`
pub struct WrAdd<T: Gen + Parameterable<T::Sample>> {
    gen: T,
    value: T::Sample,
}
impl<T: Gen + Parameterable<T::Sample>> WrAdd<T> {
    pub fn new(gen: T, value: T::Sample) -> Self {
        Self { gen, value }
    }
}
impl<T: Gen + Parameterable<T::Sample>> Gen for WrAdd<T> {
    type Sample = T::Sample;
    type Inputs = T::Inputs;
    type Outputs = T::Outputs;

    fn process(
        &mut self,
        ctx: &mut crate::AudioCtx,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.gen.process(ctx, input);
        for sample in &mut out {
            *sample += self.value;
        }
        out
    }
}

impl<T: Gen + Parameterable<T::Sample>> Parameterable<T::Sample> for WrAdd<T> {
    type Parameters = T::Parameters;

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_default_values(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterValue, Self::Parameters>
    {
        T::param_default_values()
    }

    fn param_range(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterRange, Self::Parameters>
    {
        T::param_range()
    }

    fn param_apply(&mut self, ctx: &crate::AudioCtx, index: usize, value: crate::ParameterValue) {
        T::param_apply(&mut self.gen, ctx, index, value)
    }
}

/// `gen` - `value`
pub struct WrSub<T: Gen + Parameterable<T::Sample>> {
    gen: T,
    value: T::Sample,
}
impl<T: Gen + Parameterable<T::Sample>> WrSub<T> {
    pub fn new(gen: T, value: T::Sample) -> Self {
        Self { gen, value }
    }
}
impl<T: Gen + Parameterable<T::Sample>> Gen for WrSub<T> {
    type Sample = T::Sample;
    type Inputs = T::Inputs;
    type Outputs = T::Outputs;

    fn process(
        &mut self,
        ctx: &mut crate::AudioCtx,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.gen.process(ctx, input);
        for sample in &mut out {
            *sample -= self.value;
        }
        out
    }
}

impl<T: Gen + Parameterable<T::Sample>> Parameterable<T::Sample> for WrSub<T> {
    type Parameters = T::Parameters;

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_default_values(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterValue, Self::Parameters>
    {
        T::param_default_values()
    }

    fn param_range(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterRange, Self::Parameters>
    {
        T::param_range()
    }

    fn param_apply(&mut self, ctx: &crate::AudioCtx, index: usize, value: crate::ParameterValue) {
        T::param_apply(&mut self.gen, ctx, index, value)
    }
}
/// `value` - `gen`
pub struct WrVSubGen<T: Gen + Parameterable<T::Sample>> {
    gen: T,
    value: T::Sample,
}
impl<T: Gen + Parameterable<T::Sample>> WrVSubGen<T> {
    pub fn new(gen: T, value: T::Sample) -> Self {
        Self { gen, value }
    }
}
impl<T: Gen + Parameterable<T::Sample>> Gen for WrVSubGen<T> {
    type Sample = T::Sample;
    type Inputs = T::Inputs;
    type Outputs = T::Outputs;

    fn process(
        &mut self,
        ctx: &mut crate::AudioCtx,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.gen.process(ctx, input);
        for sample in &mut out {
            *sample -= self.value;
        }
        out
    }
}

impl<T: Gen + Parameterable<T::Sample>> Parameterable<T::Sample> for WrVSubGen<T> {
    type Parameters = T::Parameters;

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_default_values(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterValue, Self::Parameters>
    {
        T::param_default_values()
    }

    fn param_range(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterRange, Self::Parameters>
    {
        T::param_range()
    }

    fn param_apply(&mut self, ctx: &crate::AudioCtx, index: usize, value: crate::ParameterValue) {
        T::param_apply(&mut self.gen, ctx, index, value)
    }
}

/// `gen` / `value`
pub struct WrDiv<T: Gen + Parameterable<T::Sample>> {
    gen: T,
    value: T::Sample,
}
impl<T: Gen + Parameterable<T::Sample>> WrDiv<T> {
    pub fn new(gen: T, value: T::Sample) -> Self {
        Self { gen, value }
    }
}
impl<T: Gen + Parameterable<T::Sample>> Gen for WrDiv<T> {
    type Sample = T::Sample;
    type Inputs = T::Inputs;
    type Outputs = T::Outputs;

    fn process(
        &mut self,
        ctx: &mut crate::AudioCtx,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.gen.process(ctx, input);
        for sample in &mut out {
            *sample /= self.value;
        }
        out
    }
}

impl<T: Gen + Parameterable<T::Sample>> Parameterable<T::Sample> for WrDiv<T> {
    type Parameters = T::Parameters;

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_default_values(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterValue, Self::Parameters>
    {
        T::param_default_values()
    }

    fn param_range(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterRange, Self::Parameters>
    {
        T::param_range()
    }

    fn param_apply(&mut self, ctx: &crate::AudioCtx, index: usize, value: crate::ParameterValue) {
        T::param_apply(&mut self.gen, ctx, index, value)
    }
}

/// `value` / `gen`
pub struct WrVDivGen<T: Gen + Parameterable<T::Sample>> {
    gen: T,
    value: T::Sample,
}
impl<T: Gen + Parameterable<T::Sample>> WrVDivGen<T> {
    pub fn new(gen: T, value: T::Sample) -> Self {
        Self { gen, value }
    }
}
impl<T: Gen + Parameterable<T::Sample>> Gen for WrVDivGen<T> {
    type Sample = T::Sample;
    type Inputs = T::Inputs;
    type Outputs = T::Outputs;

    fn process(
        &mut self,
        ctx: &mut crate::AudioCtx,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.gen.process(ctx, input);
        for sample in &mut out {
            *sample /= self.value;
        }
        out
    }
}

impl<T: Gen + Parameterable<T::Sample>> Parameterable<T::Sample> for WrVDivGen<T> {
    type Parameters = T::Parameters;

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_default_values(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterValue, Self::Parameters>
    {
        T::param_default_values()
    }

    fn param_range(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterRange, Self::Parameters>
    {
        T::param_range()
    }

    fn param_apply(&mut self, ctx: &crate::AudioCtx, index: usize, value: crate::ParameterValue) {
        T::param_apply(&mut self.gen, ctx, index, value)
    }
}

/// `gen.powf(value)`
pub struct WrPowf<T: Gen + Parameterable<T::Sample>> {
    gen: T,
    value: T::Sample,
}
impl<T: Gen + Parameterable<T::Sample>> WrPowf<T> {
    pub fn new(gen: T, value: T::Sample) -> Self {
        Self { gen, value }
    }
}
impl<T: Gen + Parameterable<T::Sample>> Gen for WrPowf<T> {
    type Sample = T::Sample;
    type Inputs = T::Inputs;
    type Outputs = T::Outputs;

    fn process(
        &mut self,
        ctx: &mut crate::AudioCtx,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.gen.process(ctx, input);
        for sample in &mut out {
            sample.powf(self.value);
        }
        out
    }
}

impl<T: Gen + Parameterable<T::Sample>> Parameterable<T::Sample> for WrPowf<T> {
    type Parameters = T::Parameters;

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_default_values(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterValue, Self::Parameters>
    {
        T::param_default_values()
    }

    fn param_range(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterRange, Self::Parameters>
    {
        T::param_range()
    }

    fn param_apply(&mut self, ctx: &crate::AudioCtx, index: usize, value: crate::ParameterValue) {
        T::param_apply(&mut self.gen, ctx, index, value)
    }
}
