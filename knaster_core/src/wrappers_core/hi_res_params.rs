use knaster_primitives::{Frame, numeric_array::NumericArray};

use crate::{AudioCtx, ParameterValue, UGen, UGenFlags, rt_log};

/// Enables sample accurate parameter changes within a block. Changes must be
/// scheduled in the order they are to be applied.
///
/// `DelayedChangesPerBlock` determines the maximum number of changes that can
/// be scheduled per block.
///
/// This wrapper needs to be outside of other wrappers that can run partial blocks, such as [`WrSmoothParams`] and [`WrArParams`]
pub struct WrPreciseTiming<const DELAYED_CHANGES_PER_BLOCK: usize, T: UGen> {
    ugen: T,
    // frame in block, parameter index, value
    waiting_changes: [Option<(u16, usize, ParameterValue)>; DELAYED_CHANGES_PER_BLOCK],
    // The time that the next change to the given parameter index should be delayed by.
    next_delay: NumericArray<u16, T::Parameters>,
    // The number of delayed changes this block to avoid unnecessary loops
    next_delay_i: usize,
}

impl<T: UGen, const DELAYED_CHANGES_PER_BLOCK: usize>
    WrPreciseTiming<DELAYED_CHANGES_PER_BLOCK, T>
{
    #[allow(missing_docs)]
    pub fn new(ugen: T) -> Self {
        WrPreciseTiming {
            ugen,
            waiting_changes: [None; DELAYED_CHANGES_PER_BLOCK],
            next_delay: NumericArray::default(),
            next_delay_i: 0,
        }
    }
}

impl<T: UGen, const DELAYED_CHANGES_PER_BLOCK: usize> UGen
    for WrPreciseTiming<DELAYED_CHANGES_PER_BLOCK, T>
{
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
        // The block size is one so all available changes should be applied
        for waiting_change in &mut self.waiting_changes {
            if let Some((_delay, index, value)) = waiting_change.take() {
                self.ugen.param_apply(ctx, index, value);
            }
        }
        self.ugen.process(ctx, flags, input)
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_primitives::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_primitives::Block<Sample = Self::Sample>,
    {
        let mut block_i = 0;
        let mut change_i = 0;
        let org_block = ctx.block;
        let num_changes_scheduled = self.next_delay_i;
        loop {
            let mut local_frames_to_process = ctx.frames_to_process() - block_i;
            // Process the next delyed change
            while change_i < num_changes_scheduled {
                if let Some((delay, index, value)) = &self.waiting_changes[change_i] {
                    if (*delay as usize) <= block_i + ctx.block_start_offset() {
                        self.ugen.param_apply(ctx, *index, *value);
                        self.waiting_changes[change_i] = None;
                    } else {
                        local_frames_to_process = local_frames_to_process
                            .min((*delay) as usize - ctx.block_start_offset() - block_i);
                        break;
                    }
                }
                change_i += 1;
            }
            if block_i >= ctx.frames_to_process() {
                break;
            }
            if local_frames_to_process == ctx.frames_to_process() {
                self.ugen.process_block(ctx, flags, input, output);
            } else {
                let input = input.partial(block_i, local_frames_to_process);
                let mut output = output.partial_mut(block_i, local_frames_to_process);
                let partial_block = org_block.make_partial(block_i, local_frames_to_process);
                ctx.block = partial_block;
                self.ugen.process_block(ctx, flags, &input, &mut output);
                ctx.block = org_block;
            }
            block_i += local_frames_to_process;
        }
        ctx.block = org_block;
        //
        self.next_delay_i = 0;
    }

    type Parameters = T::Parameters;

    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_hints() -> NumericArray<crate::parameters::ParameterHint, Self::Parameters> {
        T::param_hints()
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
        if self.next_delay[index] == 0 {
            self.ugen.param_apply(ctx, index, value);
        } else if self.next_delay_i < DELAYED_CHANGES_PER_BLOCK {
            self.waiting_changes[self.next_delay_i] = Some((self.next_delay[index], index, value));
            self.next_delay_i += 1;
        } else {
            rt_log!(ctx.logger(); "Warning: Not enough space for echeduled changes in WrHiResParams, change ignored. Allocate more space for saving scheduled changes by setting the generic DelayedChangesPerBlock to a higher number than the current", DELAYED_CHANGES_PER_BLOCK);
        }
    }

    unsafe fn set_ar_param_buffer(
        &mut self,
        ctx: &mut AudioCtx,
        index: usize,
        buffer: *const T::Sample,
    ) {
        unsafe { self.ugen.set_ar_param_buffer(ctx, index, buffer) }
    }

    fn set_delay_within_block_for_param(&mut self, _ctx: &mut AudioCtx, index: usize, delay: u16) {
        self.next_delay[index] = delay;
    }
}
