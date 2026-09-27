use knaster_core::{
    AudioCtx, Block, BlockRead, ControlSmoothing, Float, Frame, ParameterHint, ParameterValue,
    PartialBlock, Rate, UGen, UGenFlags, numeric_array::NumericArray, typenum::*,
};

/// Wrapper that enables control smoothing for a [`UGen`].
///
/// First send a [`ParameterValue::Smoothing`] for a specific control to set
/// the type of smoothing to be applied. Then set the parameter to a new value.
/// The value will be interpolated from the old to the new value.
pub struct WrSmoothParams<T: UGen> {
    ugen: T,
    controls: NumericArray<Rate, T::Controls>,
    smoothing: NumericArray<ControlSmoothing, T::Controls>,
    smoothing_state: NumericArray<ParameterSmoothingState<T::Sample>, T::Controls>,
}

impl<T: UGen> WrSmoothParams<T> {
    #[allow(missing_docs)]
    pub fn new(ugen: T) -> Self {
        Self {
            ugen,
            controls: NumericArray::default(),
            smoothing: NumericArray::default(),
            // TODO: Initialise state to default parameter values
            smoothing_state: NumericArray::default(),
        }
    }
    /// Set the smoothing kind for a specific parameter index.
    pub fn set_smoothing(
        &mut self,
        index: usize,
        smoothing: ControlSmoothing,
        new_rate: Rate,
        sample_rate: f64,
    ) {
        self.smoothing[index] = smoothing;
        match smoothing {
            ControlSmoothing::None => match self.smoothing_state[index] {
                ParameterSmoothingState::None { .. } => (),
                ParameterSmoothingState::Linear {
                    start_value,
                    end_value,
                    duration_frames,
                    frames_elapsed,
                    ..
                } => {
                    let current_mix = T::Sample::from_usize(frames_elapsed)
                        / T::Sample::from_usize(duration_frames);
                    let current_value = (end_value - start_value) * current_mix + start_value;
                    self.smoothing_state[index] = ParameterSmoothingState::None { current_value };
                }
            },
            ControlSmoothing::Linear(new_duration) => {
                let new_duration_in_frames = (new_duration as f64 * sample_rate) as usize;
                match self.smoothing_state[index] {
                    ParameterSmoothingState::None { current_value } => {
                        self.smoothing_state[index] = ParameterSmoothingState::Linear {
                            start_value: current_value,
                            end_value: current_value,
                            duration_frames: new_duration_in_frames,
                            frames_elapsed: 0,
                            rate: new_rate,
                            done: true,
                        }
                    }
                    ParameterSmoothingState::Linear {
                        start_value,
                        end_value,
                        duration_frames,
                        frames_elapsed,
                        done,
                        rate: _,
                    } => {
                        if done {
                            self.smoothing_state[index] = ParameterSmoothingState::Linear {
                                start_value: end_value,
                                end_value,
                                duration_frames: new_duration_in_frames,
                                frames_elapsed: 0,
                                rate: new_rate,
                                done: true,
                            }
                        } else {
                            let current_mix = T::Sample::from_usize(frames_elapsed)
                                / T::Sample::from_usize(duration_frames);
                            let current_value =
                                (end_value - start_value) * current_mix + start_value;
                            self.smoothing_state[index] = ParameterSmoothingState::Linear {
                                start_value: current_value,
                                end_value,
                                duration_frames: new_duration_in_frames,
                                frames_elapsed,
                                rate: new_rate,
                                done: true,
                            }
                        }
                    }
                }
            }
        }
    }
}
impl<T: UGen> UGen for WrSmoothParams<T> {
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
        // We only have potentially block rate parameters, run the whole block
        for (j, state) in self.smoothing_state.iter_mut().enumerate() {
            if let Some(new_value) = state.next_value(1, 0) {
                self.ugen.control(ctx, j, new_value)
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
        InBlock: BlockRead<Sample = Self::Sample> + ?Sized,
        OutBlock: Block<Sample = Self::Sample> + ?Sized,
    {
        let mut there_is_an_ar_parameter = false;
        let org_block = ctx.block;
        // TODO: set the Rate  of a parameter
        for p in &self.controls {
            there_is_an_ar_parameter = there_is_an_ar_parameter || matches!(*p, Rate::AudioRate);
        }
        if there_is_an_ar_parameter {
            //  run sample by sample for as long as needed
            let mut i = 0;
            while i < ctx.frames_to_process() {
                let mut there_is_a_new_smoothing_value = false;

                for p in &self.smoothing_state {
                    there_is_a_new_smoothing_value =
                        there_is_a_new_smoothing_value || p.has_new_ar_value();
                }
                if there_is_a_new_smoothing_value {
                    for (j, state) in self.smoothing_state.iter_mut().enumerate() {
                        if let Some(new_value) = state.next_value(ctx.block_size(), i) {
                            self.ugen.control(ctx, j, new_value)
                        }
                    }
                    let input = PartialBlock::from_block(input, i, 1);
                    let mut output = output.partial_mut(i, 1);
                    let partial_ctx = org_block.make_partial(i, 1);
                    ctx.block = partial_ctx;
                    self.ugen.process_block(ctx, flags, &input, &mut output);
                    i += 1;
                } else {
                    // Process the full block
                    let input = PartialBlock::from_block(input, i, input.block_size() - i);
                    let mut output = output.partial_mut(i, output.block_size() - i);
                    let partial_ctx = org_block.make_partial(i, ctx.block_size() - i);
                    ctx.block = partial_ctx;
                    self.ugen.process_block(ctx, flags, &input, &mut output);
                    break;
                }
            }
        } else {
            // We only have potentially block rate parameters, run the whole block
            for (j, state) in self.smoothing_state.iter_mut().enumerate() {
                if let Some(new_value) = state.next_value(ctx.block_size(), 0) {
                    self.ugen.control(ctx, j, new_value)
                }
            }
            self.ugen.process_block(ctx, flags, input, output);
        }
    }
    type Parameters = T::Parameters;

    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_hints() -> NumericArray<ParameterHint, Self::Parameters> {
        T::param_hints()
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
        match value {
            ParameterValue::Smoothing(smoothing, rate) => {
                self.set_smoothing(index, smoothing, rate, ctx.sample_rate() as f64)
            }
            _ => {
                self.ugen.param_apply(ctx, index, value);
            }
        }
        self.ugen.param_apply(ctx, index, value);
    }

    type Controls = T::Controls;

    fn control_metadata(
        &self,
    ) -> NumericArray<knaster_core::controls::ControlMetadata<Self::Sample>, Self::Controls> {
        self.ugen.control_metadata()
    }

    fn control(&mut self, ctx: &mut AudioCtx, index: usize, value: Self::Sample) {
        if index >= T::Controls::USIZE {
            return;
        }
        // With an audio rate parameter, ignore other incoming parameter changes
        if !matches!(self.controls[index], Rate::AudioRate) {
            match &mut self.smoothing_state[index] {
                ParameterSmoothingState::None { .. } => self.ugen.control(ctx, index, value),
                ParameterSmoothingState::Linear {
                    start_value,
                    end_value,
                    duration_frames,
                    frames_elapsed,
                    done,
                    ..
                } => {
                    if *done {
                        *start_value = *end_value;
                    } else {
                        let current_mix = Self::Sample::from_usize(*frames_elapsed)
                            / Self::Sample::from_usize(*duration_frames);
                        let current_value =
                            (*end_value - *start_value) * current_mix + *start_value;
                        *start_value = current_value;
                    }
                    *end_value = value;
                    *done = false;
                    *frames_elapsed = 0;
                }
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
/// Represents the internal state of a channel of parameter smoothing.
enum ParameterSmoothingState<F: Float> {
    None {
        current_value: F,
    },
    Linear {
        start_value: F,
        end_value: F,
        duration_frames: usize,
        frames_elapsed: usize,
        rate: Rate,
        done: bool,
    },
}
impl<F: Float> ParameterSmoothingState<F> {
    pub fn next_value(&mut self, block_size: usize, frame_in_block: usize) -> Option<F> {
        match self {
            ParameterSmoothingState::None { .. } => None,
            ParameterSmoothingState::Linear {
                start_value,
                end_value,
                duration_frames,
                frames_elapsed,
                rate,
                done,
            } => {
                if matches!(rate, Rate::BlockRate) && frame_in_block != 0 {
                    // Only provide a new value at the start of a block in BlockRate
                    return None;
                }
                if *done {
                    None
                } else {
                    let current_mix =
                        F::from_usize(*frames_elapsed) / F::from_usize(*duration_frames);
                    let current_value = (*end_value - *start_value) * current_mix + *start_value;
                    if *frames_elapsed == *duration_frames {
                        *done = true;
                    } else {
                        match rate {
                            Rate::BlockRate => {
                                *frames_elapsed =
                                    (*frames_elapsed + block_size).min(*duration_frames);
                            }
                            Rate::AudioRate => {
                                *frames_elapsed += 1;
                            }
                        }
                    }
                    Some(current_value)
                }
            }
        }
    }
    /// Returns true if the parameter needs to be modulated at audio rate this
    /// block.
    pub fn has_new_ar_value(&self) -> bool {
        match self {
            ParameterSmoothingState::None { .. } => false,
            ParameterSmoothingState::Linear { done, rate, .. } => {
                !*done && matches!(rate, Rate::AudioRate)
            }
        }
    }
}
impl<F: Float> Default for ParameterSmoothingState<F> {
    fn default() -> Self {
        ParameterSmoothingState::None {
            current_value: F::ZERO,
        }
    }
}
