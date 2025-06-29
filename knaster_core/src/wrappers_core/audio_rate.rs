use crate::{
    UGenFlags,
    core::{marker::PhantomData, ops::Add},
};

use knaster_primitives::{
    Block, BlockRead, Float, Frame, PFloat, Size,
    numeric_array::NumericArray,
    typenum::{Add1, B1, Cmp, Less, Unsigned},
};

use crate::{AudioCtx, UGen, parameters::ParameterValue};

/// Wrapper that enables setting a parameter to an audio rate signal. This must
/// wrap a [`UGen`] for audio rate parameter changes to take effect.
pub struct WrArParams<T: UGen> {
    ugen: T,
    buffers: NumericArray<Option<*const T::Sample>, T::Parameters>,
    // Keeps track of where we are in a block if processing sample-by-sample
    block_index: usize,
}

unsafe impl<T: UGen> Send for WrArParams<T> {}

impl<T: UGen> WrArParams<T> {
    #[allow(missing_docs)]
    pub fn new(ugen: T) -> Self {
        Self {
            ugen,
            buffers: NumericArray::default(),
            block_index: 0,
        }
    }
}

impl<T: UGen> UGen for WrArParams<T> {
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
        input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        for (param, buffer) in self.buffers.iter().enumerate() {
            if let Some(ptr) = buffer {
                let value = unsafe { *ptr.add(self.block_index) }.to_f64() as PFloat;
                self.ugen
                    .param_apply(ctx, param, ParameterValue::Float(value))
            }
        }
        self.block_index = (self.block_index + 1) % ctx.block_size();
        self.ugen.process(ctx, flags, input)
    }
    // TODO: Be more efficient about processing

    type Parameters = T::Parameters;

    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_hints() -> NumericArray<crate::parameters::ParameterHint, Self::Parameters> {
        T::param_hints()
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
        if self.buffers[index].is_none() {
            self.ugen.param_apply(ctx, index, value);
        }
    }

    unsafe fn set_ar_param_buffer(
        &mut self,
        _ctx: &mut AudioCtx,
        index: usize,
        buffer: *const T::Sample,
    ) {
        debug_assert!(index < T::Parameters::USIZE);
        self.buffers[index] = Some(buffer);
    }
}

/// Adds an audio input channel and uses that channel to set a parameter every
/// sample. This disables block based processing and any optimisations related
/// to that for the inner [`UGen`]
///
/// For use in `knaster_graph`, prefer
pub struct WrArParamToInput<T, ParamIndex> {
    ugen: T,
    _index: PhantomData<ParamIndex>,
}

impl<T: UGen, ParamIndex: Unsigned> UGen for WrArParamToInput<T, ParamIndex>
where
    // ParamIndex must be less than the number of parameters. This is as much as
    // we can check statically, remaining checks will be done at runtime.
    ParamIndex: Cmp<T::Inputs, Output = Less>,
    <T as UGen>::Inputs: Add<B1>,
    <<T as UGen>::Inputs as Add<B1>>::Output: Size,
{
    type Sample = T::Sample;

    type Inputs = Add1<T::Inputs>;

    type Outputs = T::Outputs;

    fn init(&mut self, sample_rate: u32, block_size: usize) {
        // TODO: check that this parameter is a float parameter
        self.ugen.init(sample_rate, block_size)
    }

    fn process(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        // The index T::Inputs is one more than the previous number of audio
        // inputs, i.e. the one we added with the wrapper.
        self.ugen.param_apply(
            ctx,
            ParamIndex::USIZE,
            ParameterValue::Float(input[<T as UGen>::Inputs::USIZE].to_f64() as PFloat),
        );
        let mut new_input = NumericArray::default();
        for i in 0..T::Inputs::USIZE {
            new_input[i] = input[i];
        }
        self.ugen.process(ctx, flags, new_input)
    }

    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
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
            let out_frame = self.process(ctx, flags, in_frame);
            for i in 0..Self::Outputs::USIZE {
                output.write(out_frame[i], i, frame);
            }
        }
    }
    type Parameters = T::Parameters;

    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_hints() -> NumericArray<crate::parameters::ParameterHint, Self::Parameters> {
        T::param_hints()
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
        self.ugen.param_apply(ctx, index, value);
    }
}
