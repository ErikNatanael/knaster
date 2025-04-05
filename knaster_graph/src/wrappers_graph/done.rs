use knaster_core::numeric_array::NumericArray;
use knaster_core::typenum::{Add1, B1, Unsigned};
use knaster_core::{
    AudioCtx, Block, BlockMetadata, BlockRead, Done, Frame, ParameterHint, ParameterValue, Size,
    UGen, UGenFlags,
};
use crate::core::ops::Add;
use crate::core::sync::Arc;
use crate::core::sync::atomic::AtomicBool;

/// Wrapper that can free a node once it has been marked as done. Unlike most wrappers_graph, this one
/// can only be added by the [`Graph`] when pushing a node using the corresponding function.
///
/// Adds a parameter, which is always the last parameter and always called "done_action", for
/// changing what action is taken when the internal node is marked as done. See [`Done`] for more
/// information.
pub struct WrDone<T> {
    pub(crate) ugen: T,
    pub(crate) free_self_flag: Arc<AtomicBool>,
    pub(crate) done_action: Done,
}
impl<T: UGen> WrDone<T> {
    fn process_flags(&mut self, ctx: &mut AudioCtx, flags: &mut UGenFlags) {
        if let Some(frame) = flags.done() {
            match self.done_action {
                Done::None => {}
                Done::FreeSelf => flags.mark_remove_self(ctx),
                Done::FreeParent => flags.mark_remove_parent(frame),
            }
        }
        if flags.remove_self() {
            self.free_self_flag
                .store(true, crate::core::sync::atomic::Ordering::Relaxed);
        }
    }
}

impl<T: UGen> UGen for WrDone<T>
where
    // Make sure we can add a parameter
    <T as UGen>::Parameters: Add<B1>,
    <<T as UGen>::Parameters as Add<B1>>::Output: Size,
{
    type Sample = T::Sample;

    type Inputs = T::Inputs;

    type Outputs = T::Outputs;

    fn init(&mut self, sample_rate: u32, block_size: usize) {
        self.ugen.init(sample_rate, block_size)
    }

    fn process(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        flags.clear_node_flags();
        flags.mark_remove_self_supported();
        let out = self.ugen.process(ctx, flags, input);
        self.process_flags(ctx, flags);
        out
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
        flags.clear_node_flags();
        flags.mark_remove_self_supported();
        self.ugen.process_block(ctx, flags, input, output);
        self.process_flags(ctx, flags);
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

    fn param_hints() -> NumericArray<ParameterHint, Self::Parameters> {
        let gd = T::param_hints();
        let mut d = NumericArray::default();
        for i in 0..T::Parameters::USIZE {
            d[i] = gd[i];
        }
        d[T::Parameters::USIZE] = ParameterHint::from_pinteger_enum::<Done>();
        d
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
        if index == T::Parameters::USIZE {
            self.done_action = value.integer().unwrap().into();
        } else {
            self.ugen.param_apply(ctx, index, value);
        }
    }
}
