use crate::core::{marker::PhantomData, ops::Add};

use knaster_primitives::{
    numeric_array::NumericArray,
    typenum::{Add1, Cmp, Less, Unsigned, B1},
    Block, BlockRead, Float, Frame, Size,
};

use crate::{
    parameters::{PFloat, ParameterValue, Parameterable},
    AudioCtx, BlockAudioCtx, Gen,
};

/// Wrapper that enables setting a parameter to an audio rate signal. This must
/// wrap a [`Gen`] for audio rate parameter changes to take effect.
pub struct ArParams<T: Gen + Parameterable<T::Sample>> {
    gen: T,
    buffers: NumericArray<Option<*const T::Sample>, T::Parameters>,
    // Keeps track of where we are in a block if processing sample-by-sample
    block_index: usize,
}

unsafe impl<T: Gen + Parameterable<T::Sample>> Send for ArParams<T> {}

impl<T: Gen + Parameterable<T::Sample>> ArParams<T> {
    pub fn new(gen: T) -> Self {
        Self {
            gen,
            buffers: NumericArray::default(),
            block_index: 0,
        }
    }
}

impl<T: Gen + Parameterable<T::Sample>> Parameterable<T::Sample> for ArParams<T> {
    type Parameters = T::Parameters;

    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_default_values() -> NumericArray<ParameterValue, Self::Parameters> {
        T::param_default_values()
    }

    fn param_range() -> NumericArray<crate::parameters::ParameterRange, Self::Parameters> {
        T::param_range()
    }

    fn param_apply(&mut self, ctx: &AudioCtx, index: usize, value: ParameterValue) {
        if self.buffers[index].is_none() {
            self.gen.param_apply(ctx, index, value);
        }
    }

    unsafe fn param_set_ar_param_buffer(&mut self, index: usize, buffer: *const T::Sample) {
        debug_assert!(index < T::Parameters::USIZE);
        self.buffers[index] = Some(buffer);
    }
}
impl<T: Gen + Parameterable<T::Sample>> Gen for ArParams<T> {
    type Sample = T::Sample;

    type Inputs = T::Inputs;

    type Outputs = T::Outputs;

    fn init(&mut self, ctx: &AudioCtx) {
        self.gen.init(ctx);
    }

    fn process(
        &mut self,
        ctx: &mut AudioCtx,
        input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        for (param, buffer) in self.buffers.iter().enumerate() {
            if let Some(ptr) = buffer {
                let value = unsafe { *ptr.add(self.block_index) }.to_f64() as PFloat;
                self.gen
                    .param_apply(ctx, param, ParameterValue::Float(value))
            }
        }
        self.block_index = (self.block_index + 1) % ctx.block_size() as usize;
        self.gen.process(ctx, input)
    }
    // TODO: Be more efficient about processing
}

/// Adds an audio input channel and uses that channel to set a parameter every
/// sample. This disables block based processing and any optimisations related
/// to that for the inner [`Gen`]
///
/// For use in `knaster_graph`, prefer
pub struct ArParamToInput<T, ParamIndex> {
    gen: T,
    _index: PhantomData<ParamIndex>,
}

impl<T: Gen + Parameterable<T::Sample>, ParamIndex: Unsigned> Gen for ArParamToInput<T, ParamIndex>
where
    // ParamIndex must be less than the number of parameters. This is as much as
    // we can check statically, remaining checks will be done at runtime.
    ParamIndex: Cmp<T::Inputs, Output = Less>,
    <T as Gen>::Inputs: Add<B1>,
    <<T as Gen>::Inputs as Add<B1>>::Output: Size,
{
    type Sample = T::Sample;

    type Inputs = Add1<T::Inputs>;

    type Outputs = T::Outputs;

    fn init(&mut self, ctx: &AudioCtx) {
        // TODO: check that this parameter is a float parameter
        self.gen.init(ctx)
    }

    fn process(
        &mut self,
        ctx: &mut AudioCtx,
        input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        // The index T::Inputs is one more than the previous number of audio
        // inputs, i.e. the one we added with the wrapper.
        self.gen.param_apply(
            ctx,
            ParamIndex::USIZE,
            ParameterValue::Float(input[<T as Gen>::Inputs::USIZE].to_f64() as PFloat),
        );
        let mut new_input = NumericArray::default();
        for i in 0..T::Inputs::USIZE {
            new_input[i] = input[i];
        }
        self.process(ctx, new_input)
    }

    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: &mut BlockAudioCtx,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: BlockRead<Sample = Self::Sample>,
        OutBlock: Block<Sample = Self::Sample>,
    {
        for frame in 0..ctx.frames_to_process() {
            // This is potentially a tiny bit inefficient because it initialises the memory before overwriting it.
            let mut in_frame = Frame::default();
            for i in 0..Self::Inputs::USIZE {
                in_frame[i] = input.read(i, frame);
            }
            let out_frame = self.process(ctx.into(), in_frame);
            for i in 0..Self::Outputs::USIZE {
                output.write(out_frame[i], i, frame);
            }
        }
    }
}
