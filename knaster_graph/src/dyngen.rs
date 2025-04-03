use crate::block::{AggregateBlockRead, RawBlock};
use alloc::vec::Vec;
use knaster_core::{AudioCtx, BlockAudioCtx, Float, ParameterValue, UGen, UGenFlags};
use knaster_core::{ParameterHint, typenum::*};

/// Type erasing trait to allow us to store [`UGen`]s as trait objects. It
/// requires all nodes that are added to the [`Graph`] to implement both [`UGen`]
/// and [`Parameterable`].
///
///
/// For type erasure, we cannot be generic over the types of blocks. This is not
/// a problem since this interface is essentially Graph internal. A different
/// graph implementation can make a different tradeoff with different types.
pub trait DynUGen<F> {
    fn init(&mut self, ctx: &AudioCtx);
    fn process_block(
        &mut self,
        ctx: BlockAudioCtx,
        flags: &mut UGenFlags,
        input: &AggregateBlockRead<F>,
        output: &mut RawBlock<F>,
    ) where
        F: Float;
    fn inputs(&self) -> u16;
    fn outputs(&self) -> u16;
    fn parameters(&self) -> u16;

    unsafe fn set_ar_param_buffer(&mut self, index: usize, buffer: *const F);
    fn set_delay_within_block_for_param(&mut self, index: usize, delay: u16);
    fn param_apply(&mut self, ctx: AudioCtx, parameter: usize, value: ParameterValue);
    fn param_descriptions(&self) -> Vec<&'static str>;
    fn param_description_fn(&self) -> fn(usize) -> Option<&'static str>;
    fn param_hints(&self) -> Vec<ParameterHint>;
    fn param_hints_fn(&self) -> fn(usize) -> Option<ParameterHint>;
}
impl<F: Float, T: UGen<Sample = F>> DynUGen<F> for T {
    fn init(&mut self, ctx: &AudioCtx) {
        self.init(ctx)
    }

    fn process_block(
        &mut self,
        ctx: BlockAudioCtx,
        flags: &mut UGenFlags,
        input: &AggregateBlockRead<F>,
        output: &mut RawBlock<F>,
    ) where
        F: Float,
    {
        self.process_block(ctx, flags, input, output)
    }

    fn inputs(&self) -> u16 {
        T::Inputs::U16
    }
    fn outputs(&self) -> u16 {
        T::Outputs::U16
    }
    fn parameters(&self) -> u16 {
        T::Parameters::U16
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
    fn param_description_fn(&self) -> fn(usize) -> Option<&'static str> {
        |index: usize| T::param_descriptions().get(index).map(|s| *s)
    }
    fn param_hints(&self) -> Vec<ParameterHint> {
        Self::param_hints().to_vec()
    }

    fn param_hints_fn(&self) -> fn(usize) -> Option<ParameterHint> {
        |index: usize| T::param_hints().get(index).map(|s| *s)
    }
}
