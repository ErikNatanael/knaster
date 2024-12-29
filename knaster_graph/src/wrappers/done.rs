use knaster_core::numeric_array::NumericArray;
use knaster_core::typenum::{Add1, Unsigned, B1};
use knaster_core::{
    AudioCtx, Block, BlockAudioCtx, BlockRead, Done, Frame, Gen, GenFlags, PFloat, ParameterRange,
    ParameterValue, Size,
};
use std::ops::Add;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Wrapper that can free a node once it has been marked as done. Unlike most wrappers, this one
/// can only be added by the [`Graph`] when pushing a node using the corresponding function.
/// 
/// Adds a parameter, which is always the last parameter and always called "done_action", for
/// changing what action is taken when the internal node is marked as done. See [`Done`] for more 
/// information.
pub struct WrDone<T> {
    pub(crate) gen: T,
    pub(crate) free_self_flag: Arc<AtomicBool>,
    pub(crate) done_action: Done,
}
impl<T: Gen> WrDone<T> {
    fn process_flags(&mut self, flags: &mut GenFlags) {
        if let Some(frame) = flags.done() {
            match self.done_action {
                Done::None => {}
                Done::FreeSelf => flags.mark_remove_self(),
                Done::FreeParent => flags.mark_remove_parent(frame),
            }
        }
        if flags.remove_self() {
            self.free_self_flag
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

impl<T: Gen> Gen for WrDone<T>
where
    // Make sure we can add a parameter
    <T as Gen>::Parameters: Add<B1>,
    <<T as Gen>::Parameters as Add<B1>>::Output: Size,
{
    type Sample = T::Sample;

    type Inputs = T::Inputs;

    type Outputs = T::Outputs;

    fn init(&mut self, ctx: &AudioCtx) {
        self.gen.init(ctx)
    }

    fn process(
        &mut self,
        ctx: AudioCtx,
        flags: &mut GenFlags,
        input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        let out = self.gen.process(ctx, flags, input);
        self.process_flags(flags);
        out
    }

    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: BlockAudioCtx,
        flags: &mut GenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: BlockRead<Sample = Self::Sample>,
        OutBlock: Block<Sample = Self::Sample>,
    {
        self.gen.process_block(ctx, flags, input, output);
        self.process_flags(flags);
    }
    type Parameters = Add1<T::Parameters>;

    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        let gd = T::param_descriptions();
        let mut d = NumericArray::default();
        for i in 0..T::Parameters::USIZE {
            d[i] = gd[i];
        }
        d[T::Parameters::USIZE] = "done_action";
        d
    }

    fn param_range() -> NumericArray<ParameterRange, Self::Parameters> {
        let gd = T::param_range();
        let mut d = NumericArray::default();
        for i in 0..T::Parameters::USIZE {
            d[i] = gd[i];
        }
        d[T::Parameters::USIZE] = ParameterRange::done();
        d
    }

    fn param_apply(&mut self, ctx: AudioCtx, index: usize, value: ParameterValue) {
        if index == T::Parameters::USIZE {
            self.done_action = value.index().unwrap().into();
        } else {
            self.gen.param_apply(ctx, index, value);
        }
    }
}
