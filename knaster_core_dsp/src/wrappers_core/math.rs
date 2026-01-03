use crate::core::ops::Add;
use knaster_core::{
    AudioCtx, Float, FloatMethods, ParameterHint, ParameterValue, Size, UGen, UGenFlags,
    numeric_array::NumericArray,
    typenum::{Add1, Unsigned, bit::B1},
};

// TODO: SIMD implementations for blocks
// TODO: SIMD implementations for multi channel frame by frame outputs
// TODO: min, max, powi, sqrt, exp, exp2, abs, range/mul_add, cbrt, tanh
//
// TODO: Replace by macro_rules macro

/// `gen` * `value`
pub struct WrMul<T: UGen> {
    ugen: T,
    value: T::Sample,
}
impl<T: UGen> WrMul<T> {
    #[allow(missing_docs)]
    pub fn new(ugen: T, value: T::Sample) -> Self {
        Self { ugen, value }
    }
}
impl<T: UGen> UGen for WrMul<T>
where
    // Make sure we can add a parameter
    <T as UGen>::Parameters: Add<B1>,
    <<T as UGen>::Parameters as Add<B1>>::Output: Size,
    T::Sample: Float,
{
    type Sample = T::Sample;
    type Inputs = T::Inputs;
    type Outputs = T::Outputs;

    fn init(&mut self, sample_rate: u32, block_size: usize) {
        self.ugen.init(sample_rate, block_size);
    }

    fn process(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.ugen.process(ctx, flags, input);
        for sample in &mut out {
            *sample *= self.value;
        }
        out
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_core::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_core::Block<Sample = Self::Sample>,
    {
        self.ugen.process_block(ctx, flags, input, output);
        for channel in output.iter_mut() {
            for sample in channel {
                *sample *= self.value;
            }
        }
    }
    type Parameters = Add1<T::Parameters>;

    fn param_descriptions()
    -> knaster_core::numeric_array::NumericArray<&'static str, Self::Parameters> {
        let gd = T::param_descriptions();
        let mut d = NumericArray::default();
        for i in 0..T::Parameters::USIZE {
            d[i] = gd[i];
        }
        d[T::Parameters::USIZE] = "wr_mul";
        d
    }

    fn param_hints() -> knaster_core::numeric_array::NumericArray<ParameterHint, Self::Parameters> {
        let gd = T::param_hints();
        let mut d = NumericArray::default();
        for i in 0..T::Parameters::USIZE {
            d[i] = gd[i];
        }
        d[T::Parameters::USIZE] = ParameterHint::new_float(|h| h.logarithmic(true).infinite());
        d
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
        if index == T::Parameters::USIZE {
            self.value = T::Sample::new(value.float().unwrap());
        } else {
            T::param_apply(&mut self.ugen, ctx, index, value)
        }
    }
    unsafe fn set_ar_param_buffer(
        &mut self,
        ctx: &mut AudioCtx,
        index: usize,
        buffer: *const T::Sample,
    ) {
        unsafe {
            self.ugen.set_ar_param_buffer(ctx, index, buffer);
        }
    }
    fn set_delay_within_block_for_param(&mut self, ctx: &mut AudioCtx, index: usize, delay: u16) {
        self.ugen
            .set_delay_within_block_for_param(ctx, index, delay);
    }
}

/// `gen` + `value`
pub struct WrAdd<T: UGen> {
    ugen: T,
    value: T::Sample,
}
impl<T: UGen> WrAdd<T> {
    #[allow(missing_docs)]
    pub fn new(ugen: T, value: T::Sample) -> Self {
        Self { ugen, value }
    }
}
impl<T: UGen> UGen for WrAdd<T> {
    type Sample = T::Sample;
    type Inputs = T::Inputs;
    type Outputs = T::Outputs;

    fn init(&mut self, sample_rate: u32, block_size: usize) {
        self.ugen.init(sample_rate, block_size);
    }
    fn process(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.ugen.process(ctx, flags, input);
        for sample in &mut out {
            *sample += self.value;
        }
        out
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_core::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_core::Block<Sample = Self::Sample>,
    {
        self.ugen.process_block(ctx, flags, input, output);
        for channel in output.iter_mut() {
            for sample in channel {
                *sample += self.value;
            }
        }
    }
    type Parameters = T::Parameters;

    fn param_descriptions()
    -> knaster_core::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_hints() -> knaster_core::numeric_array::NumericArray<ParameterHint, Self::Parameters> {
        T::param_hints()
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
        T::param_apply(&mut self.ugen, ctx, index, value)
    }
    unsafe fn set_ar_param_buffer(
        &mut self,
        ctx: &mut AudioCtx,
        index: usize,
        buffer: *const T::Sample,
    ) {
        unsafe {
            self.ugen.set_ar_param_buffer(ctx, index, buffer);
        }
    }
    fn set_delay_within_block_for_param(&mut self, ctx: &mut AudioCtx, index: usize, delay: u16) {
        self.ugen
            .set_delay_within_block_for_param(ctx, index, delay);
    }
}

/// `gen` - `value`
pub struct WrSub<T: UGen> {
    ugen: T,
    value: T::Sample,
}
impl<T: UGen> WrSub<T> {
    #[allow(missing_docs)]
    pub fn new(ugen: T, value: T::Sample) -> Self {
        Self { ugen, value }
    }
}
impl<T: UGen> UGen for WrSub<T> {
    type Sample = T::Sample;
    type Inputs = T::Inputs;
    type Outputs = T::Outputs;

    fn init(&mut self, sample_rate: u32, block_size: usize) {
        self.ugen.init(sample_rate, block_size);
    }
    fn process(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.ugen.process(ctx, flags, input);
        for sample in &mut out {
            *sample -= self.value;
        }
        out
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_core::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_core::Block<Sample = Self::Sample>,
    {
        self.ugen.process_block(ctx, flags, input, output);
        for channel in output.iter_mut() {
            for sample in channel {
                *sample -= self.value;
            }
        }
    }
    type Parameters = T::Parameters;

    fn param_descriptions()
    -> knaster_core::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_hints() -> knaster_core::numeric_array::NumericArray<ParameterHint, Self::Parameters> {
        T::param_hints()
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
        T::param_apply(&mut self.ugen, ctx, index, value)
    }
    unsafe fn set_ar_param_buffer(
        &mut self,
        ctx: &mut AudioCtx,
        index: usize,
        buffer: *const T::Sample,
    ) {
        unsafe {
            self.ugen.set_ar_param_buffer(ctx, index, buffer);
        }
    }
    fn set_delay_within_block_for_param(&mut self, ctx: &mut AudioCtx, index: usize, delay: u16) {
        self.ugen
            .set_delay_within_block_for_param(ctx, index, delay);
    }
}
/// The inverse of WrSub, i.e. the inner UGen is the right hand operand:
/// `value` - `ugen`
pub struct WrVSub<T: UGen> {
    ugen: T,
    value: T::Sample,
}
impl<T: UGen> WrVSub<T> {
    #[allow(missing_docs)]
    pub fn new(ugen: T, value: T::Sample) -> Self {
        Self { ugen, value }
    }
}
impl<T: UGen> UGen for WrVSub<T> {
    type Sample = T::Sample;
    type Inputs = T::Inputs;
    type Outputs = T::Outputs;

    fn init(&mut self, sample_rate: u32, block_size: usize) {
        self.ugen.init(sample_rate, block_size);
    }
    fn process(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.ugen.process(ctx, flags, input);
        for sample in &mut out {
            *sample = self.value - *sample;
        }
        out
    }

    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_core::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_core::Block<Sample = Self::Sample>,
    {
        self.ugen.process_block(ctx, flags, input, output);
        for channel in output.iter_mut() {
            for sample in channel {
                *sample = self.value - *sample;
            }
        }
    }
    type Parameters = T::Parameters;

    fn param_descriptions()
    -> knaster_core::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_hints() -> knaster_core::numeric_array::NumericArray<ParameterHint, Self::Parameters> {
        T::param_hints()
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
        T::param_apply(&mut self.ugen, ctx, index, value)
    }
    unsafe fn set_ar_param_buffer(
        &mut self,
        ctx: &mut AudioCtx,
        index: usize,
        buffer: *const T::Sample,
    ) {
        unsafe {
            self.ugen.set_ar_param_buffer(ctx, index, buffer);
        }
    }
    fn set_delay_within_block_for_param(&mut self, ctx: &mut AudioCtx, index: usize, delay: u16) {
        self.ugen
            .set_delay_within_block_for_param(ctx, index, delay);
    }
}

/// `gen` / `value`
pub struct WrDiv<T: UGen> {
    ugen: T,
    value: T::Sample,
}
impl<T: UGen> WrDiv<T> {
    #[allow(missing_docs)]
    pub fn new(ugen: T, value: T::Sample) -> Self {
        Self { ugen, value }
    }
}
impl<T: UGen> UGen for WrDiv<T> {
    type Sample = T::Sample;
    type Inputs = T::Inputs;
    type Outputs = T::Outputs;

    fn init(&mut self, sample_rate: u32, block_size: usize) {
        self.ugen.init(sample_rate, block_size);
    }
    fn process(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.ugen.process(ctx, flags, input);
        for sample in &mut out {
            *sample /= self.value;
        }
        out
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_core::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_core::Block<Sample = Self::Sample>,
    {
        self.ugen.process_block(ctx, flags, input, output);
        for channel in output.iter_mut() {
            for sample in channel {
                *sample /= self.value;
            }
        }
    }
    type Parameters = T::Parameters;

    fn param_descriptions()
    -> knaster_core::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_hints() -> knaster_core::numeric_array::NumericArray<ParameterHint, Self::Parameters> {
        T::param_hints()
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
        T::param_apply(&mut self.ugen, ctx, index, value)
    }
    unsafe fn set_ar_param_buffer(
        &mut self,
        ctx: &mut AudioCtx,
        index: usize,
        buffer: *const T::Sample,
    ) {
        unsafe {
            self.ugen.set_ar_param_buffer(ctx, index, buffer);
        }
    }
    fn set_delay_within_block_for_param(&mut self, ctx: &mut AudioCtx, index: usize, delay: u16) {
        self.ugen
            .set_delay_within_block_for_param(ctx, index, delay);
    }
}

/// `value` / `gen`
pub struct WrVDiv<T: UGen> {
    ugen: T,
    value: T::Sample,
}
impl<T: UGen> WrVDiv<T> {
    #[allow(missing_docs)]
    pub fn new(ugen: T, value: T::Sample) -> Self {
        Self { ugen, value }
    }
}
impl<T: UGen> UGen for WrVDiv<T> {
    type Sample = T::Sample;
    type Inputs = T::Inputs;
    type Outputs = T::Outputs;

    fn init(&mut self, sample_rate: u32, block_size: usize) {
        self.ugen.init(sample_rate, block_size);
    }
    fn process(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.ugen.process(ctx, flags, input);
        for sample in &mut out {
            *sample = self.value / *sample;
        }
        out
    }

    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_core::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_core::Block<Sample = Self::Sample>,
    {
        self.ugen.process_block(ctx, flags, input, output);
        for channel in output.iter_mut() {
            for sample in channel {
                *sample = self.value / *sample;
            }
        }
    }
    type Parameters = T::Parameters;

    fn param_descriptions()
    -> knaster_core::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_hints() -> knaster_core::numeric_array::NumericArray<ParameterHint, Self::Parameters> {
        T::param_hints()
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
        T::param_apply(&mut self.ugen, ctx, index, value)
    }
    unsafe fn set_ar_param_buffer(
        &mut self,
        ctx: &mut AudioCtx,
        index: usize,
        buffer: *const T::Sample,
    ) {
        unsafe {
            self.ugen.set_ar_param_buffer(ctx, index, buffer);
        }
    }
    fn set_delay_within_block_for_param(&mut self, ctx: &mut AudioCtx, index: usize, delay: u16) {
        self.ugen
            .set_delay_within_block_for_param(ctx, index, delay);
    }
}

/// `gen.powf(value)`
pub struct WrPowf<T: UGen> {
    ugen: T,
    value: T::Sample,
}
impl<T: UGen> WrPowf<T> {
    #[allow(missing_docs)]
    pub fn new(ugen: T, value: T::Sample) -> Self {
        Self { ugen, value }
    }
}
impl<T: UGen> UGen for WrPowf<T> {
    type Sample = T::Sample;
    type Inputs = T::Inputs;
    type Outputs = T::Outputs;

    fn init(&mut self, sample_rate: u32, block_size: usize) {
        self.ugen.init(sample_rate, block_size);
    }
    fn process(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.ugen.process(ctx, flags, input);
        for sample in &mut out {
            *sample = sample.powf(self.value);
        }
        out
    }

    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_core::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_core::Block<Sample = Self::Sample>,
    {
        self.ugen.process_block(ctx, flags, input, output);
        for channel in output.iter_mut() {
            for sample in channel {
                *sample = sample.powf(self.value);
            }
        }
    }
    type Parameters = T::Parameters;

    fn param_descriptions()
    -> knaster_core::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_hints() -> knaster_core::numeric_array::NumericArray<ParameterHint, Self::Parameters> {
        T::param_hints()
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
        T::param_apply(&mut self.ugen, ctx, index, value)
    }
    unsafe fn set_ar_param_buffer(
        &mut self,
        ctx: &mut AudioCtx,
        index: usize,
        buffer: *const T::Sample,
    ) {
        unsafe {
            self.ugen.set_ar_param_buffer(ctx, index, buffer);
        }
    }
    fn set_delay_within_block_for_param(&mut self, ctx: &mut AudioCtx, index: usize, delay: u16) {
        self.ugen
            .set_delay_within_block_for_param(ctx, index, delay);
    }
}

/// `gen.powi(value)`
pub struct WrPowi<T: UGen> {
    ugen: T,
    value: i32,
}
impl<T: UGen> WrPowi<T> {
    #[allow(missing_docs)]
    pub fn new(ugen: T, value: i32) -> Self {
        Self { ugen, value }
    }
}
impl<T: UGen> UGen for WrPowi<T> {
    type Sample = T::Sample;
    type Inputs = T::Inputs;
    type Outputs = T::Outputs;

    fn init(&mut self, sample_rate: u32, block_size: usize) {
        self.ugen.init(sample_rate, block_size);
    }
    fn process(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.ugen.process(ctx, flags, input);
        for sample in &mut out {
            *sample = sample.powi(self.value);
        }
        out
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_core::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_core::Block<Sample = Self::Sample>,
    {
        self.ugen.process_block(ctx, flags, input, output);
        for channel in output.iter_mut() {
            for sample in channel {
                *sample = sample.powi(self.value);
            }
        }
    }
    type Parameters = T::Parameters;

    fn param_descriptions()
    -> knaster_core::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }
    fn param_hints() -> knaster_core::numeric_array::NumericArray<ParameterHint, Self::Parameters> {
        T::param_hints()
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
        T::param_apply(&mut self.ugen, ctx, index, value)
    }
    unsafe fn set_ar_param_buffer(
        &mut self,
        ctx: &mut AudioCtx,
        index: usize,
        buffer: *const T::Sample,
    ) {
        unsafe {
            self.ugen.set_ar_param_buffer(ctx, index, buffer);
        }
    }
    fn set_delay_within_block_for_param(&mut self, ctx: &mut AudioCtx, index: usize, delay: u16) {
        self.ugen
            .set_delay_within_block_for_param(ctx, index, delay);
    }
}
