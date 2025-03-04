use crate::core::marker::PhantomData;
use crate::{ParameterHint, ParameterValue};

use knaster_primitives::{
    numeric_array::NumericArray,
    typenum::{U0, U1},
    Float, Frame,
};

use crate::{AudioCtx, UGen, UGenFlags};

/// Sets the done flag when it receives a trigger. Use in combination with `Graph::push_with_done_action` or [`WrDone`] and a [`Done`] which frees more than the current node.
pub struct DoneOnTrig<F> {
    triggered: bool,
    _phantom: PhantomData<F>,
}
impl<F: Float> DoneOnTrig<F> {
    pub fn new() -> Self {
        Self {
            triggered: false,
            _phantom: PhantomData,
        }
    }
}
impl<F: Float> UGen for DoneOnTrig<F> {
    type Sample = F;

    type Inputs = U0;

    type Outputs = U0;

    type Parameters = U1;

    fn process(
        &mut self,
        _ctx: AudioCtx,
        flags: &mut UGenFlags,
        _input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        if self.triggered {
            flags.mark_done(0);
        }
        [].into()
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: super::BlockAudioCtx,
        flags: &mut UGenFlags,
        _input: &InBlock,
        _output: &mut OutBlock,
    ) where
        InBlock: knaster_primitives::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_primitives::Block<Sample = Self::Sample>,
    {
        if self.triggered {
            flags.mark_done(ctx.block_start_offset() as u32);
        }
    }

    fn param_hints() -> NumericArray<ParameterHint, Self::Parameters> {
        [ParameterHint::Trigger].into()
    }
    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        ["t_done"].into()
    }

    fn param_apply(&mut self, _ctx: AudioCtx, index: usize, _value: ParameterValue) {
        if index == 0 {
            self.triggered = true
        }
    }
}

pub struct Constant<F: Float> {
    value: F,
}
impl<F: Float> Constant<F> {
    pub fn new(value: F) -> Self {
        Self { value }
    }
}
impl<F: Float> UGen for Constant<F> {
    type Sample = F;

    type Inputs = U0;

    type Outputs = U1;

    type Parameters = U1;

    fn process(
        &mut self,
        _ctx: AudioCtx,
        _flags: &mut UGenFlags,
        _input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        [self.value].into()
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        _ctx: super::BlockAudioCtx,
        _flags: &mut UGenFlags,
        _input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_primitives::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_primitives::Block<Sample = Self::Sample>,
    {
        output.channel_as_slice_mut(0).fill(self.value);
    }

    fn param_hints() -> NumericArray<ParameterHint, Self::Parameters> {
        [ParameterHint::infinite_float()].into()
    }
    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        ["value"].into()
    }

    fn param_apply(&mut self, _ctx: AudioCtx, index: usize, value: ParameterValue) {
        if index == 0 {
            self.value = value.f().unwrap();
        }
    }
}
