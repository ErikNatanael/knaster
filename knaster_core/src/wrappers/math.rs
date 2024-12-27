use knaster_primitives::FloatMethods;

use crate::{Gen, GenFlags, Parameterable};

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
        ctx: crate::AudioCtx,
        flags: &mut GenFlags,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.gen.process(ctx, flags, input);
        for sample in &mut out {
            *sample *= self.value;
        }
        out
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: crate::BlockAudioCtx,
        flags: &mut GenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_primitives::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_primitives::Block<Sample = Self::Sample>,
    {
        self.gen.process_block(ctx, flags, input, output);
        for channel in output.iter_mut() {
            for sample in channel {
                *sample *= self.value;
            }
        }
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

    fn param_apply(&mut self, ctx: crate::AudioCtx, index: usize, value: crate::ParameterValue) {
        T::param_apply(&mut self.gen, ctx, index, value)
    }
    unsafe fn set_ar_param_buffer(&mut self, index: usize, buffer: *const T::Sample) {
        self.gen.set_ar_param_buffer(index, buffer);
    }
    fn set_delay_within_block_for_param(&mut self, index: usize, delay: u16) {
        self.gen.set_delay_within_block_for_param(index, delay);
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
        ctx: crate::AudioCtx,
        flags: &mut GenFlags,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.gen.process(ctx, flags, input);
        for sample in &mut out {
            *sample += self.value;
        }
        out
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: crate::BlockAudioCtx,
        flags: &mut GenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_primitives::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_primitives::Block<Sample = Self::Sample>,
    {
        self.gen.process_block(ctx, flags, input, output);
        for channel in output.iter_mut() {
            for sample in channel {
                *sample += self.value;
            }
        }
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

    fn param_apply(&mut self, ctx: crate::AudioCtx, index: usize, value: crate::ParameterValue) {
        T::param_apply(&mut self.gen, ctx, index, value)
    }
    unsafe fn set_ar_param_buffer(&mut self, index: usize, buffer: *const T::Sample) {
        self.gen.set_ar_param_buffer(index, buffer);
    }
    fn set_delay_within_block_for_param(&mut self, index: usize, delay: u16) {
        self.gen.set_delay_within_block_for_param(index, delay);
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
        ctx: crate::AudioCtx,
        flags: &mut GenFlags,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.gen.process(ctx, flags, input);
        for sample in &mut out {
            *sample -= self.value;
        }
        out
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: crate::BlockAudioCtx,
        flags: &mut GenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_primitives::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_primitives::Block<Sample = Self::Sample>,
    {
        self.gen.process_block(ctx, flags, input, output);
        for channel in output.iter_mut() {
            for sample in channel {
                *sample -= self.value;
            }
        }
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

    fn param_apply(&mut self, ctx: crate::AudioCtx, index: usize, value: crate::ParameterValue) {
        T::param_apply(&mut self.gen, ctx, index, value)
    }
    unsafe fn set_ar_param_buffer(&mut self, index: usize, buffer: *const T::Sample) {
        self.gen.set_ar_param_buffer(index, buffer);
    }
    fn set_delay_within_block_for_param(&mut self, index: usize, delay: u16) {
        self.gen.set_delay_within_block_for_param(index, delay);
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
        ctx: crate::AudioCtx,
        flags: &mut GenFlags,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.gen.process(ctx, flags, input);
        for sample in &mut out {
            *sample = self.value - *sample;
        }
        out
    }

    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: crate::BlockAudioCtx,
        flags: &mut GenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_primitives::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_primitives::Block<Sample = Self::Sample>,
    {
        self.gen.process_block(ctx, flags, input, output);
        for channel in output.iter_mut() {
            for sample in channel {
                *sample = self.value - *sample;
            }
        }
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

    fn param_apply(&mut self, ctx: crate::AudioCtx, index: usize, value: crate::ParameterValue) {
        T::param_apply(&mut self.gen, ctx, index, value)
    }
    unsafe fn set_ar_param_buffer(&mut self, index: usize, buffer: *const T::Sample) {
        self.gen.set_ar_param_buffer(index, buffer);
    }
    fn set_delay_within_block_for_param(&mut self, index: usize, delay: u16) {
        self.gen.set_delay_within_block_for_param(index, delay);
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
        ctx: crate::AudioCtx,
        flags: &mut GenFlags,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.gen.process(ctx, flags, input);
        for sample in &mut out {
            *sample /= self.value;
        }
        out
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: crate::BlockAudioCtx,
        flags: &mut GenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_primitives::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_primitives::Block<Sample = Self::Sample>,
    {
        self.gen.process_block(ctx, flags, input, output);
        for channel in output.iter_mut() {
            for sample in channel {
                *sample /= self.value;
            }
        }
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

    fn param_apply(&mut self, ctx: crate::AudioCtx, index: usize, value: crate::ParameterValue) {
        T::param_apply(&mut self.gen, ctx, index, value)
    }
    unsafe fn set_ar_param_buffer(&mut self, index: usize, buffer: *const T::Sample) {
        self.gen.set_ar_param_buffer(index, buffer);
    }
    fn set_delay_within_block_for_param(&mut self, index: usize, delay: u16) {
        self.gen.set_delay_within_block_for_param(index, delay);
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
        ctx: crate::AudioCtx,
        flags: &mut GenFlags,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.gen.process(ctx, flags, input);
        for sample in &mut out {
            *sample = self.value / *sample;
        }
        out
    }

    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: crate::BlockAudioCtx,
        flags: &mut GenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_primitives::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_primitives::Block<Sample = Self::Sample>,
    {
        self.gen.process_block(ctx, flags, input, output);
        for channel in output.iter_mut() {
            for sample in channel {
            *sample = self.value / *sample;
            }
        }
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

    fn param_apply(&mut self, ctx: crate::AudioCtx, index: usize, value: crate::ParameterValue) {
        T::param_apply(&mut self.gen, ctx, index, value)
    }
    unsafe fn set_ar_param_buffer(&mut self, index: usize, buffer: *const T::Sample) {
        self.gen.set_ar_param_buffer(index, buffer);
    }
    fn set_delay_within_block_for_param(&mut self, index: usize, delay: u16) {
        self.gen.set_delay_within_block_for_param(index, delay);
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
        ctx: crate::AudioCtx,
        flags: &mut GenFlags,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.gen.process(ctx, flags, input);
        for sample in &mut out {
            *sample = sample.powf(self.value);
        }
        out
    }

    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: crate::BlockAudioCtx,
        flags: &mut GenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_primitives::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_primitives::Block<Sample = Self::Sample>,
    {
        self.gen.process_block(ctx, flags, input, output);
        for channel in output.iter_mut() {
            for sample in channel {

            *sample = sample.powf(self.value);
            }
        }
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

    fn param_apply(&mut self, ctx: crate::AudioCtx, index: usize, value: crate::ParameterValue) {
        T::param_apply(&mut self.gen, ctx, index, value)
    }
    unsafe fn set_ar_param_buffer(&mut self, index: usize, buffer: *const T::Sample) {
        self.gen.set_ar_param_buffer(index, buffer);
    }
    fn set_delay_within_block_for_param(&mut self, index: usize, delay: u16) {
        self.gen.set_delay_within_block_for_param(index, delay);
    }
}

/// `gen.powi(value)`
pub struct WrPowi<T: Gen + Parameterable<T::Sample>> {
    gen: T,
    value: i32,
}
impl<T: Gen + Parameterable<T::Sample>> WrPowi<T> {
    pub fn new(gen: T, value: i32) -> Self {
        Self { gen, value }
    }
}
impl<T: Gen + Parameterable<T::Sample>> Gen for WrPowi<T> {
    type Sample = T::Sample;
    type Inputs = T::Inputs;
    type Outputs = T::Outputs;

    fn process(
        &mut self,
        ctx: crate::AudioCtx,
        flags: &mut GenFlags,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.gen.process(ctx, flags, input);
        for sample in &mut out {
            *sample = sample.powi(self.value);
        }
        out
    }
}

impl<T: Gen + Parameterable<T::Sample>> Parameterable<T::Sample> for WrPowi<T> {
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

    fn param_apply(&mut self, ctx: crate::AudioCtx, index: usize, value: crate::ParameterValue) {
        T::param_apply(&mut self.gen, ctx, index, value)
    }
    unsafe fn set_ar_param_buffer(&mut self, index: usize, buffer: *const T::Sample) {
        self.gen.set_ar_param_buffer(index, buffer);
    }
    fn set_delay_within_block_for_param(&mut self, index: usize, delay: u16) {
        self.gen.set_delay_within_block_for_param(index, delay);
    }
}
