use knaster_primitives::{numeric_array::NumericArray, Frame, Size};

use crate::{AudioCtx, Gen, ParameterValue, Parameterable};

/// Enables sample accurate parameter changes within a block. Changes must be
/// scheduled in the order they are to be applied.
///
/// `DelayedChangesPerBlock` determines the maximum number of changes that can
/// be scheduled per block.
///
/// It is recommended to apply this outside of [`WrSmoothParams`]
pub struct WrHiResParams<T: Gen + Parameterable<T::Sample>, DelayedChangesPerBlock: Size> {
    gen: T,
    // frame in block, parameter index, value
    waiting_changes: NumericArray<Option<(u16, usize, ParameterValue)>, DelayedChangesPerBlock>,
    next_delay: NumericArray<u16, T::Parameters>,
}

impl<T: Gen + Parameterable<T::Sample>, DelayedChangesPerBlock: Size> Parameterable<T::Sample>
    for WrHiResParams<T, DelayedChangesPerBlock>
{
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
        if self.next_delay[index] == 0 {
            self.gen.param_apply(ctx, index, value);
        } else {
            let mut i = 0;
            while i < DelayedChangesPerBlock::USIZE {
                if self.waiting_changes[i].is_none() {
                    self.waiting_changes[i] = Some((self.next_delay[index], index, value));
                }
            }
        }
        self.next_delay[index] = 0;
    }

    unsafe fn param_set_ar_param_buffer(&mut self, index: usize, buffer: *const T::Sample) {
        self.gen.param_set_ar_param_buffer(index, buffer)
    }

    fn param_set_delay_in_block_for_index(&mut self, index: usize, delay: u16) {
        self.next_delay[index] = delay;
    }
}
impl<T: Gen + Parameterable<T::Sample>, DelayedChangesPerBlock: Size> Gen
    for WrHiResParams<T, DelayedChangesPerBlock>
{
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
        // The block size is one so all available changes should be applied
        for waiting_change in &mut self.waiting_changes {
            if let Some((_delay, index, value)) = waiting_change.take() {
                self.gen.param_apply(ctx, index, value);
            }
        }
        self.gen.process(ctx, input)
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: &mut crate::BlockAudioCtx,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_primitives::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_primitives::Block<Sample = Self::Sample>,
    {
        let mut block_i = 0;
        let mut change_i = 0;
        let mut local_frames_to_process = ctx.frames_to_process();
        loop {
            // Process the next delyed change
            while change_i < DelayedChangesPerBlock::USIZE {
                if let Some((delay, index, value)) = &self.waiting_changes[change_i] {
                    if (*delay as usize) < block_i + ctx.block_start_offset() {
                        self.gen.param_apply(ctx.into(), *index, *value);
                        self.waiting_changes[change_i] = None;
                    } else {
                        local_frames_to_process = local_frames_to_process
                            .min((*delay) as usize - ctx.block_start_offset());

                        break;
                    }
                }
                change_i += 1;
            }
            if block_i >= ctx.frames_to_process() {
                break;
            }
            if local_frames_to_process == ctx.frames_to_process() {
                self.gen.process_block(ctx, input, output);
            } else {
                let input = input.partial(block_i, local_frames_to_process);
                let mut output = output.partial_mut(block_i, local_frames_to_process);
                let mut partial_ctx = ctx.make_partial(block_i, local_frames_to_process);
                self.gen
                    .process_block(&mut partial_ctx, &input, &mut output);
                ctx.combine_flag_state(&mut partial_ctx);
            }
            block_i += local_frames_to_process;
        }
    }
}
