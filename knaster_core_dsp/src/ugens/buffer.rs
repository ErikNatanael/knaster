//! # Buffer
//!
//! UGens related to buffer playback. This module requires `std` or `alloc`.

use crate::core::{marker::PhantomData, sync::Arc};

use knaster_core::{
    AudioCtx, Float, PFloat, Seconds, Size, UGenFlags, impl_ugen, numeric_array::NumericArray,
    rt_log, typenum::U0,
};

use crate::dsp::buffer::Buffer;
/// Reads a frame from a buffer and outputs it. The generic `Channels` determines how many
/// channels will be read from the buffer.
///
/// `duration_s` is the playback duration before looping or stopping. If given a negative value,
/// the duration will default to the buffer length.
// TODO: Negative rate, meaning the cmp for being finished needs to be inverted and the end_frame
// needs to be start_frame - dur_frame instead.
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
    start_frame: f64,
    dur_frame: f64,
    end_frame: f64,
    _marker: PhantomData<Channels>,
}

#[impl_ugen]
impl<F: Float, Channels: Size> BufferReader<F, Channels> {
    type Inputs = U0;
    type Outputs = Channels;
    #[allow(missing_docs)]
    #[must_use]
    pub fn new(buffer: impl Into<Arc<Buffer<F>>>, rate: f64, looping: bool) -> Self {
        let buffer = buffer.into();
        let buffer_length = buffer.length_seconds();
        BufferReader {
            buffer,
            read_pointer: 0.0,
            base_rate: 0.0,
            rate,
            finished: false,
            looping,
            start_frame: 0.,
            end_frame: buffer_length,
            dur_frame: buffer_length,
            _marker: PhantomData,
        }
    }
    /// Jump back to the start of the buffer
    fn reset(&mut self) {
        self.jump_to(self.start_frame);
    }
    /// Jump to a specific point in the buffer in samples
    fn jump_to(&mut self, new_pointer_pos: f64) {
        self.read_pointer = new_pointer_pos;
        self.finished = false;
    }
    /// Jump to a specific point in the buffer in samples. Has to be called before processing starts.
    pub fn start_at(mut self, start_time: Seconds) -> Self {
        self.start_frame = start_time.to_secs_f64();
        self
    }

    #[param]
    #[allow(missing_docs)]
    pub fn rate(&mut self, v: f64) {
        self.rate = v;
    }
    #[param]
    #[allow(missing_docs)]
    pub fn looping(&mut self, value: bool) {
        self.looping = value;
    }
    #[param]
    #[allow(missing_docs)]
    pub fn start_s(&mut self, start_s: PFloat) {
        let start_time = Seconds::from_secs_f64(start_s);
        self.start_frame = start_time.to_samples_f64(self.buffer.sample_rate());
        self.end_frame = self.start_frame + self.dur_frame;
    }
    #[param]
    #[allow(missing_docs)]
    pub fn duration_s(&mut self, duration_s: PFloat) {
        let dur_time = Seconds::from_secs_f64(duration_s);
        self.dur_frame = dur_time.to_samples_f64(self.buffer.sample_rate());
        self.end_frame = self.start_frame + self.dur_frame;
    }
    #[param]
    #[allow(missing_docs)]
    pub fn end_s(&mut self, end_s: PFloat) {
        let end_time = Seconds::from_secs_f64(end_s);
        self.end_frame = end_time.to_samples_f64(self.buffer.sample_rate());
    }
    #[param]
    #[allow(missing_docs)]
    pub fn t_restart(&mut self) {
        self.reset();
    }

    fn init(&mut self, sample_rate: u32, _block_size: usize) {
        self.base_rate = self.buffer.buf_rate_scale(sample_rate);
        self.start_frame =
            Seconds::from_secs_f64(self.start_frame).to_samples_f64(self.buffer.sample_rate());
        self.dur_frame =
            Seconds::from_secs_f64(self.dur_frame).to_samples_f64(self.buffer.sample_rate());
        self.end_frame = self.start_frame + self.dur_frame;
        self.jump_to(self.start_frame);
    }

    // Using UGen trait process fn signatures because of generic channels

    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        _input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        let mut output = NumericArray::default();
        if self.finished {
            output.fill(F::ZERO);
            return output;
        }
        for chan in 0..Channels::USIZE {
            let sample = self.buffer.get_linear_interp_f64(self.read_pointer, chan);
            output[chan] = sample;
        }
        self.read_pointer += self.base_rate * self.rate;
        if self.read_pointer >= self.end_frame {
            self.finished = true;
            if self.looping {
                self.reset();
            } else {
                flags.mark_done(0);
            }
        }
        output
    }

    fn process_block<InBlock, OutBlock>(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        _input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_core::BlockRead<Sample = Self::Sample> + ?Sized,
        OutBlock: knaster_core::Block<Sample = Self::Sample> + ?Sized,
    {
        let mut stop_sample = None;
        if !self.finished {
            for i in 0..ctx.block_size() {
                for chan in 0..Channels::USIZE {
                    let sample = self.buffer.get_linear_interp_f64(self.read_pointer, chan);
                    output.write(sample, chan, i);
                }
                self.read_pointer += self.base_rate * self.rate;
                if self.read_pointer >= self.end_frame {
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
            rt_log!(ctx.logger(); "Error: BufferReader: buffer doesn't exist in Resources");
            stop_sample = Some(0);
        }
        if let Some(stop_sample) = stop_sample {
            for out in output.iter_mut() {
                out[stop_sample..].fill(F::ZERO);
            }
        }
    }
}
