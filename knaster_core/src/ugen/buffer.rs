use core::marker::PhantomData;
use std::sync::Arc;

use knaster_primitives::{
    typenum::{U0, U1, U3},
    Float, Seconds, Size,
};

use crate::dsp::buffer::Buffer;

use super::UGen;

/// Reads a sample from a buffer and outputs it. In a multi channel [`Buffer`] only the first channel will be read.
/// TODO: Support rate through an argument with a default constant of 1
#[derive(Clone, Debug)]
pub struct BufferReader<F: Copy, Channels: Size> {
    buffer: Arc<Buffer<F>>,
    /// read pointer in samples
    read_pointer: f64,
    rate: f64,
    base_rate: f64, // The basic rate for playing the buffer at normal speed
    /// true if Self has finished reading the buffer
    finished: bool,
    /// true if the [`BufferReader`] should loop the buffer
    pub looping: bool,
    start_time: Seconds,
    _marker: PhantomData<Channels>,
}

impl<F: Float, Channels: Size> BufferReader<F, Channels> {
    #[allow(missing_docs)]
    #[must_use]
    pub fn new(buffer: impl Into<Arc<Buffer<F>>>, rate: f64, looping: bool) -> Self {
        BufferReader {
            buffer: buffer.into(),
            read_pointer: 0.0,
            base_rate: 0.0,
            rate,
            finished: false,
            looping,
            start_time: Seconds::ZERO,
            _marker: PhantomData,
        }
    }
    /// Jump back to the start of the buffer
    fn reset(&mut self) {
        self.jump_to(0.0);
    }
    /// Jump to a specific point in the buffer in samples
    fn jump_to(&mut self, new_pointer_pos: f64) {
        self.read_pointer = new_pointer_pos;
        self.finished = false;
    }
    /// Jump to a specific point in the buffer in samples. Has to be called before processing starts.
    pub fn start_at(mut self, start_time: Seconds) -> Self {
        self.start_time = start_time;
        self
    }
}
impl<F: Float, Channels: Size> UGen for BufferReader<F, Channels> {
    fn init(&mut self, ctx: &super::AudioCtx) {
        self.base_rate = self.buffer.buf_rate_scale(ctx.sample_rate());
        let start_frame = self.start_time.to_samples(self.buffer.sample_rate() as u64);
        self.jump_to(start_frame as f64);
    }

    type Sample = F;

    type Inputs = U0;

    type Outputs = U1;

    type Parameters = U3;

    fn process(
        &mut self,
        ctx: super::AudioCtx,
        flags: &mut super::UGenFlags,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        todo!()
    }

    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: super::BlockAudioCtx,
        flags: &mut super::UGenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_primitives::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_primitives::Block<Sample = Self::Sample>,
    {
        let mut stop_sample = None;
        if !self.finished {
            for i in 0..ctx.block_size() {
                let samples = self.buffer.get_interleaved((self.read_pointer) as usize);
                for (out_num, sample) in samples.iter().take(Channels::USIZE).enumerate() {
                    output.write(*sample, out_num, i);
                }
                self.read_pointer += self.base_rate * self.rate;
                if self.read_pointer >= self.buffer.num_frames() {
                    self.finished = true;
                    if self.looping {
                        self.reset();
                    } else {
                        flags.mark_done((i + 1) as u32);
                    }
                }
                if self.finished {
                    stop_sample = Some(i + 1);
                    break;
                }
            }
        } else {
            // Output zeroes if we're finished
            // eprintln!("Error: BufferReader: buffer doesn't exist in Resources");
            stop_sample = Some(0);
        }
        if let Some(stop_sample) = stop_sample {
            for out in output.iter_mut() {
                out[stop_sample..].fill(F::ZERO);
            }
        }
    }

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        ["rate", "loop", "t_restart"].into()
    }

    fn param_range(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterRange, Self::Parameters>
    {
        todo!()
    }

    fn param_apply(&mut self, ctx: super::AudioCtx, index: usize, value: crate::ParameterValue) {
        todo!()
    }
}

/*
/// Play back a buffer with multiple channels. You cannot change the number of
/// channels after pushing this to a graph. If the buffer has fewer channels
/// than `num_channels`, the remaining outputs will be left at their current
/// value, not zeroed.
#[derive(Clone, Debug)]
pub struct BufferReaderMulti {
    buffer_key: IdOrKey<BufferId, BufferKey>,
    read_pointer: f64,
    rate: f64,
    num_channels: usize,
    base_rate: f64, // The basic rate for playing the buffer at normal speed
    finished: bool,
    /// true if the BufferReaderMulti should loop the buffer
    pub looping: bool,
    stop_action: StopAction,
}

// TODO: Make this generic over the number of inputs? How would that interact with the impl_gen macro?
impl BufferReaderMulti {
    #[allow(missing_docs)]
    pub fn new(
        buffer: impl Into<IdOrKey<BufferId, BufferKey>>,
        rate: f64,
        stop_action: StopAction,
    ) -> Self {
        Self {
            buffer_key: buffer.into(),
            read_pointer: 0.0,
            base_rate: 0.0, // initialise to the correct value the first time next() is called
            rate,
            num_channels: 1,
            finished: false,
            looping: false,
            stop_action,
        }
    }
    /// Set looping
    pub fn looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }
    /// Set the number of channels to read and play
    pub fn channels(mut self, num_channels: usize) -> Self {
        self.num_channels = num_channels;
        self
    }
    /// Jump back to the start of the buffer
    pub fn reset(&mut self) {
        self.jump_to(0.0);
    }
    /// Jump to a specific point in time in samples
    pub fn jump_to(&mut self, new_pointer_pos: f64) {
        self.read_pointer = new_pointer_pos;
        self.finished = false;
    }
    /// Upload to the current graph, returning a handle to the new node
    pub fn upload(self) -> knyst::handles::Handle<BufferReaderMultiHandle> {
        let num_channels = self.num_channels;
        let id = knyst::prelude::KnystCommands::push_without_inputs(&mut knyst_commands(), self);
        knyst::handles::Handle::new(BufferReaderMultiHandle {
            node_id: id,
            num_channels,
        })
    }
}

impl Gen for BufferReaderMulti {
    fn process(&mut self, ctx: GenContext, resources: &mut crate::Resources) -> GenState {
        let mut stop_sample = None;
        if !self.finished {
            if let IdOrKey::Id(id) = self.buffer_key {
                match resources.buffer_key_from_id(id) {
                    Some(key) => self.buffer_key = IdOrKey::Key(key),
                    None => (),
                }
            }
            if let IdOrKey::Key(buffer_key) = self.buffer_key {
                if let Some(buffer) = &mut resources.buffer(buffer_key) {
                    // Initialise the base rate if it hasn't been set
                    if self.base_rate == 0.0 {
                        self.base_rate = buffer.buf_rate_scale(ctx.sample_rate.into());
                    }
                    for i in 0..ctx.block_size() {
                        let samples = buffer.get_interleaved((self.read_pointer) as usize);
                        for (out_num, sample) in samples.iter().take(self.num_channels).enumerate()
                        {
                            ctx.outputs.write(*sample, out_num, i);
                        }
                        self.read_pointer += self.base_rate * self.rate;
                        if self.read_pointer >= buffer.num_frames() {
                            self.finished = true;
                            if self.looping {
                                self.reset();
                            }
                        }
                        if self.finished {
                            stop_sample = Some(i + 1);
                            break;
                        }
                    }
                }
            } else {
                // Output zeroes if the buffer doesn't exist.
                // TODO: Send error back to the user that the buffer doesn't exist without interrupting the audio thread.
                // eprintln!("Error: BufferReader: buffer doesn't exist in Resources");
                stop_sample = Some(0);
            }
        } else {
            stop_sample = Some(0);
        }
        if let Some(stop_sample) = stop_sample {
            let mut outputs = ctx.outputs.iter_mut();
            let output = outputs.next().unwrap();
            for out in output[stop_sample..].iter_mut() {
                *out = 0.;
            }
            self.stop_action.to_gen_state(stop_sample)
        } else {
            GenState::Continue
        }
    }

    fn num_inputs(&self) -> usize {
        0
    }

    fn num_outputs(&self) -> usize {
        self.num_channels
    }

    fn output_desc(&self, output: usize) -> &'static str {
        if output < self.num_channels {
            output_str(output)
        } else {
            ""
        }
    }

    fn name(&self) -> &'static str {
        "BufferReaderMulti"
    }
}
*/
