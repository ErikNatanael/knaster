use crate::core::marker::PhantomData;

use knaster_macros::impl_ugen;
use knaster_primitives::{Float, PFloat};
#[cfg(any(feature = "alloc", feature = "std"))]
pub use wavetable_vec::*;

use super::AudioCtx;
#[cfg(any(feature = "alloc", feature = "std"))]
mod wavetable_vec {
    use crate::core::marker::PhantomData;
    #[cfg(feature = "alloc")]
    use embassy_sync::lazy_lock::LazyLock;
    use knaster_macros::impl_ugen;
    #[cfg(feature = "std")]
    use std::sync::LazyLock;

    use knaster_primitives::{Float, PFloat};

    use crate::dsp::wavetable::{
        FRACTIONAL_PART, NonAaWavetable, TABLE_SIZE, Wavetable, WavetablePhase,
    };

    /// Osciallator with an owned Wavetable
    #[derive(Debug, Clone)]
    pub struct OscWt<F> {
        step: u32,
        phase: WavetablePhase,
        phase_offset: WavetablePhase,
        wavetable: Wavetable<F>,
        freq_to_phase_inc: f64,
        freq: F,
    }

    #[impl_ugen]
    impl<F: Float> OscWt<F> {
        /// Set the frequency of the oscillation. This will be overwritten by the
        /// input frequency if used as a UGen.
        #[param]
        pub fn freq(&mut self, freq: PFloat) {
            self.freq = F::new(freq);
            self.step = (self.freq.to_f64() * self.freq_to_phase_inc) as u32;
        }
        /// Set the phase offset in a range 0-1
        #[param]
        pub fn phase_offset(&mut self, offset: PFloat) {
            self.phase_offset = WavetablePhase((offset.to_f64() * FRACTIONAL_PART as f64) as u32);
        }
        /// Reset the phase of the oscillator.
        #[param]
        pub fn reset_phase(&mut self) {
            self.phase.0 = 0;
        }

        /// Generate the next sample given the current settings.
        #[inline(always)]
        #[must_use]
        pub fn next_sample(&mut self) -> F {
            // Use the phase to index into the wavetable
            // self.wavetable.get_linear_interp(temp_phase) * self.amp
            let sample = self
                .wavetable
                .get(self.phase + self.phase_offset, self.freq);
            self.phase.increase(self.step);
            sample
        }
        pub fn process(&mut self) -> [F; 1] {
            [self.next_sample()]
        }
        pub fn process_block(&mut self, output: [&mut [F]; 1]) {
            // TODO: Try SIMDifying this with a buffer of phase etc
            for out in output[0].iter_mut() {
                *out = self.next_sample();
            }
        }
        fn init(&mut self, sample_rate: u32, __block_size: usize) {
            self.reset_phase();
            self.freq_to_phase_inc =
                TABLE_SIZE as f64 * FRACTIONAL_PART as f64 * (1.0 / sample_rate as f64);
        }
    }

    pub static SINE_WAVETABLE_F32: LazyLock<NonAaWavetable<f32>> =
        LazyLock::new(NonAaWavetable::sine);

    /// Sine wave based on a wavetable lookup.
    ///
    /// A sine wave does not need to be anti-aliased so it uses a simpler
    /// wavetable structure than OscWt.
    pub struct SinWt<F> {
        phase: WavetablePhase,
        phase_offset: WavetablePhase,
        phase_increment: u32,
        freq_to_phase_inc: f64,
        freq: F,
        wavetable: &'static NonAaWavetable<f32>,
        _marker: PhantomData<F>,
    }

    #[impl_ugen]
    impl<F: Float> SinWt<F> {
        pub fn new(freq: F) -> Self {
            Self {
                phase: WavetablePhase(0),
                phase_offset: WavetablePhase(0),
                phase_increment: 0,
                _marker: PhantomData,
                freq_to_phase_inc: 0.0,
                #[cfg(feature = "std")]
                wavetable: &SINE_WAVETABLE_F32,
                #[cfg(feature = "alloc")]
                wavetable: SINE_WAVETABLE_F32.get(),
                freq,
            }
        }
        /// Set the frequency of the oscillation. This will be overwritten by the
        /// input frequency if used as a UGen.
        #[param]
        pub fn freq(&mut self, freq: PFloat) {
            self.freq = F::new(freq);
            self.phase_increment = (self.freq.to_f64() * self.freq_to_phase_inc) as u32;
        }
        /// Set the phase offset in a range 0-1
        #[param]
        pub fn phase_offset(&mut self, offset: PFloat) {
            self.phase_offset = WavetablePhase((offset.to_f64() * FRACTIONAL_PART as f64) as u32);
        }
        /// Reset the phase of the oscillator.
        #[param]
        pub fn reset_phase(&mut self) {
            self.phase.0 = 0;
        }

        fn init(&mut self, sample_rate: u32, _block_size: usize) {
            self.reset_phase();
            self.freq_to_phase_inc =
                TABLE_SIZE as f64 * FRACTIONAL_PART as f64 * (1.0 / sample_rate as f64);
            self.freq(self.freq.to_f64()); // init any frequency set before init was called
        }
        /// Generate the next sample given the current settings.
        #[inline(always)]
        #[must_use]
        pub fn next_sample(&mut self) -> F {
            // Use the phase to index into the wavetable
            let sample = self.wavetable.get(self.phase + self.phase_offset);
            self.phase.increase(self.phase_increment);
            F::new(sample)
        }
        pub fn process(&mut self) -> [F; 1] {
            [self.next_sample()]
        }
        pub fn process_block(&mut self, output: [&mut [F]; 1]) {
            // TODO: Try SIMDifying this with a buffer of phase etc
            for out in output[0].iter_mut() {
                *out = self.next_sample();
            }
        }
    }
}

/// Linear ramp from 0 to 1 at a given frequency. Will alias at higher frequencies.
pub struct Phasor<F> {
    phase: f64,
    step: f64,
    freq_to_phase_step_mult: f64,
    _phantom: PhantomData<F>,
}

#[impl_ugen]
impl<F: Float> Phasor<F> {
    #[allow(missing_docs)]
    pub fn new(freq: f64) -> Self {
        Self {
            phase: 0.0,
            step: freq,
            freq_to_phase_step_mult: 0.0,
            _phantom: PhantomData,
        }
    }
    #[param]
    pub fn freq(&mut self, freq: f64) {
        if self.freq_to_phase_step_mult == 0.0 {
            self.step = freq;
        } else {
            self.step = freq * self.freq_to_phase_step_mult;
        }
    }
    pub fn init(&mut self, sample_rate: u32, _block_size: usize) {
        self.freq_to_phase_step_mult = 1.0_f64 / (sample_rate as f64);
        self.freq(self.step);
    }
    pub fn process(&mut self) -> [F; 1] {
        let out = F::new(self.phase);
        self.phase += self.step;
        while self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        [out]
    }
}

/// Sine wave calculated using the trigonometric function for this platform, as
/// opposed to using a shared lookup table
///
/// Phase is set between 0 and 1
///
/// A lookup table is often faster, but (with libm) this implementation is
/// available on every platform, even without allocation.
pub struct SinNumeric<F> {
    phase: F,
    phase_offset: F,
    phase_increment: F,
}

#[impl_ugen]
impl<F: Float> SinNumeric<F> {
    pub fn new(freq: F) -> Self {
        Self {
            phase: freq,
            phase_offset: F::ZERO,
            phase_increment: F::ZERO,
        }
    }
    #[param]
    pub fn freq(&mut self, freq: PFloat, ctx: &AudioCtx) {
        self.phase_increment = F::new(freq) / F::new(ctx.sample_rate() as f32);
    }
    #[param]
    pub fn phase_offset(&mut self, phase_offset: PFloat) {
        self.phase_offset = F::new(phase_offset);
    }
    #[param]
    pub fn reset_phase(&mut self) {
        self.phase = F::ZERO;
    }
    fn init(&mut self, sample_rate: u32, _block_size: usize) {
        // self.phase holds the freq set in the constructor, but only use it if the freq hasn't
        // been set any other way
        if self.phase_increment == F::ZERO {
            // Set freq to the value it is set to by default
            self.phase_increment = self.phase / F::new(sample_rate as f32);
        }
        self.phase = F::ZERO;
    }
    pub fn process(&mut self) -> [F; 1] {
        let out = ((self.phase + self.phase_offset) * F::TAU).sin();
        self.phase += self.phase_increment;
        if self.phase > F::ONE {
            self.phase -= F::ONE;
        }
        [out]
    }
}
