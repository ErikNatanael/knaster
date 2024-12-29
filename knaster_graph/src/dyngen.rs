use knaster_core::{
    AudioCtx, BlockAudioCtx, Float, Gen, GenFlags, ParameterValue
};
use knaster_core::typenum::*;
use crate::block::{AggregateBlockRead, RawBlock};

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
        ctx: BlockAudioCtx,
        flags: &mut GenFlags,
        input: &AggregateBlockRead<F>,
        output: &mut RawBlock<F>,
    ) where
        F: Float;
    fn inputs(&self) -> usize;
    fn outputs(&self) -> usize;
    unsafe fn set_ar_param_buffer(&mut self, index: usize, buffer: *const F);
    fn set_delay_within_block_for_param(&mut self, index: usize, delay: u16);
    fn param_apply(&mut self, ctx: AudioCtx, parameter: usize, value: ParameterValue);
    fn param_descriptions(&self) -> Vec<&'static str>;
}
impl<
        F: Float,
        T: Gen<Sample = F >,
    > DynGen<F> for T
{
    fn init(&mut self, ctx: &AudioCtx) {
        self.init(ctx)
    }

    fn process_block(
        &mut self,
        ctx: BlockAudioCtx,
        flags: &mut GenFlags,
        input: &AggregateBlockRead<F>,
        output: &mut RawBlock<F>,
    ) where
        F: Float,
    {
        self.process_block(ctx, flags, input, output)
    }

    fn inputs(&self) -> usize {
        T::Inputs::USIZE
    }
    fn outputs(&self) -> usize {
        T::Outputs::USIZE
    }

    unsafe fn set_ar_param_buffer(&mut self, index: usize, buffer: *const F) {
        unsafe { self.set_ar_param_buffer(index, buffer) };
    }

    fn set_delay_within_block_for_param(&mut self, index: usize, delay: u16) {
        self.set_delay_within_block_for_param(index, delay);
    }

    fn param_apply(&mut self, ctx: AudioCtx, parameter: usize, value: ParameterValue) {
        self.param_apply(ctx, parameter, value);
    }

    fn param_descriptions(&self) -> Vec<&'static str> {
        Self::param_descriptions().to_vec()
    }
}
