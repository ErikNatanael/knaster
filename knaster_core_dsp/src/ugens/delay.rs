//! # Delay
//!
//! [`UGen`]s for delays.

use knaster_core::impl_ugen;
#[allow(unused)]
use knaster_core::{AudioCtx, PFloat, UGen, UGenFlags};
use knaster_core::{Float, Seconds};
use std::prelude::v1::*;

/// Delay by an integer number of samples, no interpolation. This is good for e.g. triggers.
///
/// Delay time is given in seconds and converted to samples internally.
pub struct SampleDelay<F: Copy = f32> {
    buffer: Vec<F>,
    write_position: usize,
    delay_samples: usize,
    max_delay_length: Seconds,
}

#[impl_ugen]
impl<F: Float> SampleDelay<F> {
    /// Create a new SampleDelay with a maximum delay time.
    pub fn new(max_delay_length: Seconds) -> Self {
        Self {
            buffer: vec![F::ZERO; 0],
            max_delay_length,
            write_position: 0,
            delay_samples: 0,
        }
    }
    /// Set delay time in seconds
    #[param]
    pub fn delay_time(&mut self, ctx: &mut AudioCtx, seconds: PFloat) {
        self.delay_samples = (seconds * ctx.sample_rate() as PFloat) as usize;
    }
    fn process(&mut self, _ctx: &mut AudioCtx, _flags: &mut UGenFlags, input: [F; 1]) -> [F; 1] {
        self.buffer[self.write_position] = input[0];
        let out = self.buffer
            [(self.write_position + self.buffer.len() - self.delay_samples) % self.buffer.len()];
        self.write_position = (self.write_position + 1) % self.buffer.len();
        [out]
    }
    fn init(&mut self, sample_rate: u32, _block_size: usize) {
        self.buffer =
            vec![F::ZERO; (self.max_delay_length.to_secs_f64() * sample_rate as f64) as usize];
        self.write_position = 0;
    }
}

/// Schroeder (?) allpass interpolation
#[derive(Clone, Copy, Debug)]
pub struct AllpassInterpolator<F: Copy = f32> {
    coeff: F,
    prev_input: F,
    prev_output: F,
}

impl<F: Float> AllpassInterpolator<F> {
    /// Create a new and reset AllpassInterpolator
    pub fn new() -> Self {
        Self {
            coeff: F::ONE,
            prev_input: F::ONE,
            prev_output: F::ONE,
        }
    }
    /// Reset any state to 0
    pub fn clear(&mut self) {
        self.prev_input = F::ONE;
        self.prev_output = F::ONE;
    }
    /// Set the fractional number of frames in the delay time that we want to interpolate over
    pub fn set_delta(&mut self, delta: F) {
        self.coeff = (F::ONE - delta) / (F::ONE + delta);
    }
    /// Interpolate between the input sample and the previous value using the last set delta.
    pub fn process_sample(&mut self, input: F) -> F {
        let output = self.coeff * (input - self.prev_output) + self.prev_input;
        self.prev_output = output;
        self.prev_input = input;
        output
    }
}

impl<F: Float> Default for AllpassInterpolator<F> {
    fn default() -> Self {
        Self::new()
    }
}
/// Delay with allpass interpolation for non-integer sample delay lengths.
#[derive(Clone, Debug)]
pub struct AllpassDelay<F: Copy = f32> {
    buffer: Vec<F>,
    write_frame: usize,
    read_frame: usize,
    num_frames: usize,
    clear_nr_of_samples_left: usize,
    allpass: AllpassInterpolator<F>,
    max_delay_seconds: Seconds,
}

#[impl_ugen]
impl<F: Float> AllpassDelay<F> {
    /// Create a new allpass delay with the given maximum delay time. Doesn't allocate the delay
    /// buffer until [`Self::init`]
    pub fn new(max_delay_seconds: Seconds) -> Self {
        let buffer = vec![F::ZERO; 0];
        Self {
            buffer,
            write_frame: 0,
            read_frame: 0,
            num_frames: 1,
            allpass: AllpassInterpolator::new(),
            clear_nr_of_samples_left: 0,
            max_delay_seconds,
        }
    }
    /// Allocates internal buffer.
    pub fn init(&mut self, sample_rate: u32, _block_size: usize) {
        let max_delay_samples = self.max_delay_seconds.to_samples(sample_rate as u64);
        self.buffer = vec![F::ZERO; max_delay_samples as usize];
    }
    /// Produce one frame from the delay and put `input` into the delay.
    pub fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        input: [F; 1],
    ) -> [F; 1] {
        let out = self.read();
        self.write_and_advance(input[0]);
        [out]
    }
    /// Set delay time in seconds
    #[param]
    pub fn delay_time(&mut self, ctx: &mut AudioCtx, seconds: PFloat) {
        let delay_frames = seconds * ctx.sample_rate() as PFloat;
        if (delay_frames as usize) < self.buffer.len() {
            self.set_delay_in_frames(F::new(delay_frames));
        }
    }
    /// Read the current frame from the delay and allpass interpolate. Read before `write_and_advance` for the correct sample.
    #[inline]
    pub fn read(&mut self) -> F {
        if self.clear_nr_of_samples_left > 0 {
            // Instead of clearing the whole buffer, we amortise the cost and clear only what we need.
            // Samples between the read pointer and the write pointer will be 0 when cleared.
            self.clear_nr_of_samples_left -= 1;
            self.read_frame = (self.read_frame + 1) % self.buffer.len();
            F::ZERO
        } else {
            let v = self.allpass.process_sample(self.buffer[self.read_frame]);
            self.read_frame = (self.read_frame + 1) % self.buffer.len();
            v
        }
    }
    /// Set delay time in frames (f32 or f64)
    #[inline]
    pub fn set_delay_in_frames(&mut self, num_frames: F) {
        let num_frames_float = num_frames.floor();
        self.num_frames = num_frames_float.to_usize().unwrap();
        let mut delta = num_frames - num_frames_float;
        if num_frames > F::new(0.5) && delta < F::new(0.5) {
            delta += F::ONE;
            self.num_frames -= 1;
        }
        self.read_frame = if self.write_frame >= self.num_frames {
            self.write_frame - self.num_frames
        } else {
            self.buffer.len() - self.num_frames + self.write_frame
        };
        self.allpass.set_delta(delta);
    }
    #[inline]
    /// Call after set_delay_in_frames. Only data that won't be overwritten before read is cleared.
    pub fn clear(&mut self) {
        // We only need to clear memory from now until where the write pointer overwrites memory, which is self.num_frames into the future.
        // Samples between the read pointer and the write pointer will be 0 when cleared.
        // Zeroing memory is surprisingly expensive.
        self.clear_nr_of_samples_left = self.num_frames;
        // self.buffer.fill(0.0);
        // for sample in &mut self.buffer {
        //     *sample = 0.0;
        // }
        self.allpass.clear();
    }
    /// Reset the delay with a new length in frames
    pub fn set_delay_in_frames_and_clear(&mut self, num_frames: F) {
        for sample in &mut self.buffer {
            *sample = F::ZERO;
        }
        self.set_delay_in_frames(num_frames);
        // println!(
        //     "num_frames: {}, delta: {}",
        //     self.num_frames,
        //     (num_frames - self.num_frames as f64)
        // );
    }
    /// Write a new value into the delay after incrementing the sample pointer.
    #[inline]
    pub fn write_and_advance(&mut self, input: F) {
        self.buffer[self.write_frame] = input;
        self.write_frame = (self.write_frame + 1) % self.buffer.len();
    }
}

/// A Schroeder allpass delay with feedback. Wraps the `AllpassDelay`
#[derive(Clone, Debug)]
pub struct AllpassFeedbackDelay<F: Copy = f32> {
    /// The feedback value of the delay. Can be set directly.
    feedback: F,
    previous_delay_time: F,
    allpass_delay: AllpassDelay<F>,
}
#[impl_ugen]
impl<F: Float> AllpassFeedbackDelay<F> {
    /// Create a new allpass delay with the given maximum delay time. Doesn't allocate the delay
    /// buffer until [`Self::init`]
    #[must_use]
    pub fn new(max_delay_time: Seconds) -> Self {
        let allpass_delay = AllpassDelay::new(max_delay_time);

        Self {
            feedback: F::ZERO,
            allpass_delay,
            previous_delay_time: F::from(max_delay_time.to_secs_f64()).unwrap(),
        }
    }
    /// Set delay time in seconds
    #[param]
    pub fn delay_time(&mut self, ctx: &mut AudioCtx, seconds: PFloat) {
        self.set_delay_in_frames(F::new(seconds * ctx.sample_rate() as f64));
        self.previous_delay_time = F::new(seconds);
    }
    /// Set delay feedback
    #[param]
    pub fn feedback(&mut self, feedback: PFloat) {
        self.feedback = F::new(feedback);
    }
    fn init(&mut self, sample_rate: u32, _block_size: usize) {
        self.allpass_delay.init(sample_rate, _block_size);
    }

    fn process(&mut self, _ctx: &mut AudioCtx, _flags: &mut UGenFlags, input: [F; 1]) -> [F; 1] {
        [self.process_sample(input[0])]
    }
    /// Set the delay length counted in frames/samples
    pub fn set_delay_in_frames(&mut self, delay_length: F) {
        self.allpass_delay.set_delay_in_frames(delay_length);
    }
    /// Clear any values in the delay
    pub fn clear(&mut self) {
        self.allpass_delay.clear();
    }
    // fn calculate_values(&mut self) {
    //     self.feedback = (0.001 as F).powf(self.delay_time / self.decay_time.abs())
    //         * self.decay_time.signum();
    //     let delay_samples = self.delay_time * self.sample_rate;
    //     self.allpass_delay.set_num_frames(delay_samples as f64);
    // }
    /// Process on sample
    pub fn process_sample(&mut self, input: F) -> F {
        let delayed_sig = self.allpass_delay.read();
        let delay_write = delayed_sig * self.feedback + input;
        self.allpass_delay.write_and_advance(delay_write);

        delayed_sig - self.feedback * delay_write
    }
}
/// A sample delay with a static number of samples of delay
///
/// # Examples
/// ```
/// # use knaster_core_dsp::delay::StaticSampleDelay;
/// let mut delay = StaticSampleDelay::<f64>::new(4);
/// assert_eq!(delay.read(), 0.0);
/// delay.write_and_advance(1.0);
/// assert_eq!(delay.read(), 0.0);
/// delay.write_and_advance(2.0);
/// assert_eq!(delay.read(), 0.0);
/// delay.write_and_advance(3.0);
/// assert_eq!(delay.read(), 0.0);
/// delay.write_and_advance(4.0);
/// assert_eq!(delay.read(), 1.0);
/// delay.write_and_advance(0.0);
/// assert_eq!(delay.read(), 2.0);
/// delay.write_and_advance(0.0);
/// assert_eq!(delay.read(), 3.0);
/// delay.write_and_advance(0.0);
/// assert_eq!(delay.read(), 4.0);
/// delay.write_and_advance(0.0);
/// assert_eq!(delay.read(), 0.0);
/// delay.write_and_advance(0.0);
/// delay.write_block_and_advance(&[1.0, 2.0]);
/// let mut block = [0.0; 2];
/// delay.read_block(&mut block);
/// delay.write_block_and_advance(&[3.0, 4.0]);
/// delay.read_block(&mut block);
/// assert_eq!(block, [1.0, 2.0]);
/// delay.write_block_and_advance(&[5.0, 6.0]);
/// delay.read_block(&mut block);
/// assert_eq!(block, [3.0, 4.0]);
/// delay.write_block_and_advance(&[0.0, 0.0]);
/// delay.read_block(&mut block);
/// assert_eq!(block, [5.0, 6.0]);
/// ```
pub struct StaticSampleDelay<F> {
    buffer: Vec<F>,
    /// The current read/write position in the buffer. Public because in some situations it is more efficient to have access to it directly.
    pub position: usize,
    delay_length: usize,
}
impl<F: Float> StaticSampleDelay<F> {
    #[must_use]
    /// Create a new Self. delay_length_in_samples must be more than 0
    ///
    /// # Panics
    /// If delay_length_in_samples is 0
    pub fn new(delay_length_in_samples: usize) -> Self {
        assert!(delay_length_in_samples != 0);
        Self {
            buffer: vec![F::ZERO; delay_length_in_samples],
            position: 0,
            delay_length: delay_length_in_samples,
        }
    }

    /// Set a new delay length for the delay. Real time safe. If the given length is longer than the max delay length it will be set to the max delay length.
    #[inline]
    pub fn set_delay_length(&mut self, delay_length_in_samples: usize) {
        self.delay_length = delay_length_in_samples.min(self.buffer.len());
    }
    /// Set a new delay length for the delay as a fraction of the entire delay length. Real time safe.
    pub fn set_delay_length_fraction(&mut self, fraction: F) {
        self.delay_length = (F::from(self.buffer.len()).expect("delay length should fit in Float")
            * fraction)
            .to_usize()
            .unwrap();
    }

    /// Read a whole block at a time. Only use this if the delay time is longer than 1 block.
    #[inline]
    pub fn read_block(&mut self, output: &mut [F]) {
        let block_size = output.len();
        assert!(self.buffer.len() >= block_size);
        let read_end = self.position + block_size;
        if read_end <= self.buffer.len() {
            output.copy_from_slice(&self.buffer[self.position..read_end]);
        } else {
            // block wraps around
            let read_end = read_end % self.delay_length;
            output[0..(block_size - read_end)].copy_from_slice(&self.buffer[self.position..]);
            output[(block_size - read_end)..].copy_from_slice(&self.buffer[0..read_end]);
        }
    }
    /// Write a whole block at a time. Only use this if the delay time is longer than 1 block. Advances the frame pointer.
    #[inline]
    pub fn write_block_and_advance(&mut self, input: &[F]) {
        let block_size = input.len();
        assert!(self.buffer.len() >= block_size);
        let write_end = self.position + block_size;
        if write_end <= self.buffer.len() {
            self.buffer[self.position..write_end].copy_from_slice(input);
        } else {
            // block wraps around
            let write_end = write_end % self.delay_length;
            self.buffer[self.position..].copy_from_slice(&input[0..block_size - write_end]);
            self.buffer[0..write_end].copy_from_slice(&input[block_size - write_end..]);
        }
        self.position = (self.position + block_size) % self.buffer.len();
    }
    /// Read a sample from the buffer. Does not advance the frame pointer. Read first, then write.
    #[inline]
    pub fn read(&mut self) -> F {
        self.buffer[self.position]
    }
    /// Read a sample from the buffer at a given position. Does not advance the frame pointer. Read first, then write.
    pub fn read_at(&mut self, index: usize) -> F {
        self.buffer[index]
    }
    /// Read a sample from the buffer at a given position with linear interpolation. Does not advance the frame pointer. Read first, then write.
    pub fn read_at_lin(&mut self, index: F) -> F {
        let mut low = index.floor().to_usize().unwrap();
        let mut high = index.ceil().to_usize().unwrap();
        while low >= self.buffer.len() {
            low -= self.buffer.len();
            high -= self.buffer.len();
        }
        if high >= self.buffer.len() {
            high -= self.buffer.len();
        }
        let low_sample = self.buffer[low];
        let high_sample = self.buffer[high];
        low_sample + (high_sample - low_sample) * index.fract()
    }
    /// Write a sample to the buffer. Advances the frame pointer.
    #[inline]
    pub fn write_and_advance(&mut self, input: F) {
        self.buffer[self.position] = input;
        self.position = (self.position + 1) % self.delay_length;
    }
    /// Process one block of the delay. Will choose block based processing at runtime if the delay time is larger than the block size.
    #[inline]
    pub fn process(&mut self, input: &[F], output: &mut [F], block_size: usize) {
        if self.buffer.len() > block_size {
            self.read_block(output);
            self.write_block_and_advance(input);
        } else {
            for (i, o) in input.iter().zip(output.iter_mut()) {
                *o = self.read();
                self.write_and_advance(*i);
            }
        }
    }
}
