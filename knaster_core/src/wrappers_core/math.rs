use knaster_primitives::FloatMethods;

use crate::{Gen, GenFlags};

// TODO: SIMD implementations for blocks
// TODO: SIMD implementations for multi channel frame by frame outputs
// TODO: min, max, powi, sqrt, exp, exp2, abs, range/mul_add, cbrt, tanh

/// `gen` * `value`
pub struct WrMul<T: Gen> {
    gen: T,
    value: T::Sample,
}
impl<T: Gen> WrMul<T> {
    pub fn new(gen: T, value: T::Sample) -> Self {
        Self { gen, value }
    }
}
impl<T: Gen> Gen for WrMul<T> {
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
    type Parameters = T::Parameters;

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
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
pub struct WrAdd<T: Gen> {
    gen: T,
    value: T::Sample,
}
impl<T: Gen> WrAdd<T> {
    pub fn new(gen: T, value: T::Sample) -> Self {
        Self { gen, value }
    }
}
impl<T: Gen> Gen for WrAdd<T> {
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
    type Parameters = T::Parameters;

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
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
pub struct WrSub<T: Gen> {
    gen: T,
    value: T::Sample,
}
impl<T: Gen> WrSub<T> {
    pub fn new(gen: T, value: T::Sample) -> Self {
        Self { gen, value }
    }
}
impl<T: Gen> Gen for WrSub<T> {
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
    type Parameters = T::Parameters;

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
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
pub struct WrVSubGen<T: Gen> {
    gen: T,
    value: T::Sample,
}
impl<T: Gen> WrVSubGen<T> {
    pub fn new(gen: T, value: T::Sample) -> Self {
        Self { gen, value }
    }
}
impl<T: Gen> Gen for WrVSubGen<T> {
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
    type Parameters = T::Parameters;

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
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
pub struct WrDiv<T: Gen> {
    gen: T,
    value: T::Sample,
}
impl<T: Gen> WrDiv<T> {
    pub fn new(gen: T, value: T::Sample) -> Self {
        Self { gen, value }
    }
}
impl<T: Gen> Gen for WrDiv<T> {
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
    type Parameters = T::Parameters;

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
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
pub struct WrVDivGen<T: Gen> {
    gen: T,
    value: T::Sample,
}
impl<T: Gen> WrVDivGen<T> {
    pub fn new(gen: T, value: T::Sample) -> Self {
        Self { gen, value }
    }
}
impl<T: Gen> Gen for WrVDivGen<T> {
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
    type Parameters = T::Parameters;

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
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
pub struct WrPowf<T: Gen> {
    gen: T,
    value: T::Sample,
}
impl<T: Gen> WrPowf<T> {
    pub fn new(gen: T, value: T::Sample) -> Self {
        Self { gen, value }
    }
}
impl<T: Gen> Gen for WrPowf<T> {
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
    type Parameters = T::Parameters;

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
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
pub struct WrPowi<T: Gen> {
    gen: T,
    value: i32,
}
impl<T: Gen> WrPowi<T> {
    pub fn new(gen: T, value: i32) -> Self {
        Self { gen, value }
    }
}
impl<T: Gen> Gen for WrPowi<T> {
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

                *sample = sample.powi(self.value);
            }
        }
    }
    type Parameters = T::Parameters;

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
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
