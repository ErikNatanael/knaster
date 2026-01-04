use knaster_core::{AudioCtx, Block, BlockRead, ParameterHint, ParameterValue, UGen, UGenFlags};

/// Applies the closure to every sample of every channel in the [`UGen`] output
///
/// This is almost certainly less performant than using the dedicated maths
/// wrappers, but good for prototyping with relaxed performance constraints.
pub struct WrClosure<T: UGen, C: FnMut(T::Sample) -> T::Sample + 'static> {
    ugen: T,
    closure: C,
}
impl<T: UGen, C: FnMut(T::Sample) -> T::Sample + 'static> WrClosure<T, C> {
    #[allow(missing_docs)]
    pub fn new(ugen: T, closure: C) -> Self {
        Self { ugen, closure }
    }
}
impl<T: UGen, C: FnMut(T::Sample) -> T::Sample + 'static> UGen for WrClosure<T, C> {
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
            *sample = (self.closure)(*sample);
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
        InBlock: BlockRead<Sample = Self::Sample> + ?Sized,
        OutBlock: Block<Sample = Self::Sample> + ?Sized,
    {
        self.ugen.process_block(ctx, flags, input, output);
        for channel in output.iter_mut() {
            for sample in channel {
                *sample = (self.closure)(*sample);
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
