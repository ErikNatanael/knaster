use knaster_core::{
    AudioCtx, BlockAudioCtx, Float, Gen, Param, ParameterValue, Parameterable, Size,
};

use crate::block::{AggregateBlock, AggregateBlockRead, RawBlock};

/// Type erasing trait to allow us to store [`Gen`]s as trait objects. It
/// requires all nodes that are added to the [`Graph`] to implement both [`Gen`]
/// and [`Parameterable`].
///
///
/// For type erasure, we cannot be generic over the types of blocks. This is not
/// a problem since this interface is essentially Graph internal. A different
/// graph implementation can make a different tradeoff with different types.
pub trait DynGen<F> {
    fn init(&mut self, ctx: &AudioCtx);
    fn process_block(
        &mut self,
        ctx: &mut BlockAudioCtx,
        input: &AggregateBlockRead<F>,
        output: &mut RawBlock<F>,
    ) where
        F: Float;
    fn inputs(&self) -> usize;
    fn outputs(&self) -> usize;
    fn parameters(&self) -> usize;
    unsafe fn set_ar_param_buffer(&mut self, index: usize, buffer: *const F);
    fn param_apply(&mut self, ctx: &AudioCtx, parameter: Param, value: ParameterValue);
}
impl<
        F: Float,
        Inputs: Size,
        Outputs: Size,
        Parameters: Size,
        T: Gen<Sample = F, Inputs = Inputs, Outputs = Outputs>
            + Parameterable<F, Parameters = Parameters>,
    > DynGen<F> for T
{
    fn init(&mut self, ctx: &AudioCtx) {
        self.init(ctx)
    }

    fn inputs(&self) -> usize {
        T::Inputs::USIZE
    }

    fn outputs(&self) -> usize {
        T::Outputs::USIZE
    }
    fn parameters(&self) -> usize {
        T::Parameters::USIZE
    }

    fn process_block(
        &mut self,
        ctx: &mut BlockAudioCtx,
        input: &AggregateBlockRead<F>,
        output: &mut RawBlock<F>,
    ) where
        F: Float,
    {
        self.process_block(ctx, input, output)
    }

    unsafe fn set_ar_param_buffer(&mut self, index: usize, buffer: *const F) {
        unsafe { self.param_set_ar_param_buffer(index, buffer) };
    }

    fn param_apply(&mut self, ctx: &AudioCtx, parameter: Param, value: ParameterValue) {
        self.param(ctx, parameter, value);
    }
}
