#[cfg(feature = "std")]
use crate::core::eprintln;
use crate::core::marker::PhantomData;

use knaster_primitives::{
    Float, Frame,
    numeric_array::NumericArray,
    typenum::{U0, U1, U3, Unsigned},
};
#[cfg(any(feature = "alloc", feature = "std"))]
pub use wavetable_vec::*;

use crate::{Param, ParameterError, ParameterHint, ParameterValue};

use super::{AudioCtx, UGen, UGenFlags};
#[cfg(any(feature = "alloc", feature = "std"))]
mod wavetable_vec {
    use crate::core::marker::PhantomData;
    use crate::core::sync::LazyLock;

    use knaster_primitives::{
        Float, Frame,
        numeric_array::NumericArray,
        typenum::{U0, U1, U3},
    };

    use crate::{
        AudioCtx, ParameterHint, ParameterType, ParameterValue, UGen, UGenFlags,
        dsp::wavetable::{FRACTIONAL_PART, NonAaWavetable, TABLE_SIZE, Wavetable, WavetablePhase},
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

    impl<F: Float> OscWt<F> {
        /// Set the frequency of the oscillation. This will be overwritten by the
        /// input frequency if used as a UGen.
        pub fn set_freq(&mut self, freq: F) {
            self.freq = freq;
            self.step = (freq.to_f64() * self.freq_to_phase_inc) as u32;
        }
        /// Set the phase offset in a range 0-1
        pub fn set_phase_offset(&mut self, offset: F) {
            self.phase_offset = WavetablePhase((offset.to_f64() * FRACTIONAL_PART as f64) as u32);
        }
        /// Reset the phase of the oscillator.
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
    }

    impl<F: Float> UGen for OscWt<F> {
        type Sample = F;
        type Inputs = U0;
        type Outputs = U1;
        fn init(&mut self, ctx: &crate::AudioCtx) {
            self.reset_phase();
            self.freq_to_phase_inc =
                TABLE_SIZE as f64 * FRACTIONAL_PART as f64 * (1.0 / ctx.sample_rate() as f64);
        }

        fn process(
            &mut self,
            _ctx: crate::AudioCtx,
            _flags: &mut UGenFlags,
            _input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
        ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
            [self.next_sample()].into()
        }
        fn process_block<InBlock, OutBlock>(
            &mut self,
            _ctx: crate::BlockAudioCtx,
            _flags: &mut UGenFlags,
            _input: &InBlock,
            output: &mut OutBlock,
        ) where
            InBlock: knaster_primitives::BlockRead<Sample = Self::Sample>,
            OutBlock: knaster_primitives::Block<Sample = Self::Sample>,
        {
            // TODO: Try SIMDifying this with a buffer of phase etc
            for out in output.channel_as_slice_mut(0) {
                *out = self.next_sample();
            }
        }
        type Parameters = U3;

        fn param_types() -> NumericArray<ParameterType, Self::Parameters> {
            NumericArray::from([
                ParameterType::Float,
                ParameterType::Float,
                ParameterType::Trigger,
            ])
        }

        fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
            NumericArray::from(["freq", "phase_offset", "reset_phase"])
        }

        fn param_hints() -> NumericArray<ParameterHint, Self::Parameters> {
            NumericArray::from([
                ParameterHint::positive_infinite_float(),
                ParameterHint::infinite_float(),
                ParameterHint::Trigger,
            ])
        }

        fn param_apply(&mut self, _ctx: AudioCtx, index: usize, value: ParameterValue) {
            if matches!(value, ParameterValue::Smoothing(..)) {
                // eprintln!("Tried to set parameter smoothing with out a wrapper");
                return;
            }
            match index {
                0 => self.set_freq(F::from(value.float().unwrap()).unwrap()),
                1 => self.set_phase_offset(F::from(value.float().unwrap()).unwrap()),
                2 => self.reset_phase(),
                _ => (),
            }
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

    impl<F: Float> SinWt<F> {
        pub fn new(freq: F) -> Self {
            Self {
                phase: WavetablePhase(0),
                phase_offset: WavetablePhase(0),
                phase_increment: 0,
                _marker: PhantomData,
                freq_to_phase_inc: 0.0,
                wavetable: &SINE_WAVETABLE_F32,
                freq,
            }
        }
        /// Set the frequency of the oscillation. This will be overwritten by the
        /// input frequency if used as a UGen.
        pub fn set_freq(&mut self, freq: F) {
            self.freq = freq;
            self.phase_increment = (freq.to_f64() * self.freq_to_phase_inc) as u32;
        }
        /// Set the phase offset in a range 0-1
        pub fn set_phase_offset(&mut self, offset: F) {
            self.phase_offset = WavetablePhase((offset.to_f64() * FRACTIONAL_PART as f64) as u32);
        }
        /// Reset the phase of the oscillator.
        pub fn reset_phase(&mut self) {
            self.phase.0 = 0;
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
    }

    impl<F: Float> UGen for SinWt<F> {
        type Sample = F;
        type Inputs = U0;
        type Outputs = U1;

        fn init(&mut self, ctx: &crate::AudioCtx) {
            self.reset_phase();
            self.freq_to_phase_inc =
                TABLE_SIZE as f64 * FRACTIONAL_PART as f64 * (1.0 / ctx.sample_rate() as f64);
            self.set_freq(self.freq); // init any frequency set before init was called
        }

        fn process(
            &mut self,
            _ctx: AudioCtx,
            _flags: &mut UGenFlags,
            _input: Frame<Self::Sample, Self::Inputs>,
        ) -> Frame<Self::Sample, Self::Outputs> {
            [self.next_sample()].into()
        }
        type Parameters = U3;

        fn param_types() -> NumericArray<ParameterType, Self::Parameters> {
            NumericArray::from([
                ParameterType::Float,
                ParameterType::Float,
                ParameterType::Trigger,
            ])
        }

        fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
            NumericArray::from(["freq", "phase_offset", "reset_phase"])
        }

        fn param_hints() -> NumericArray<ParameterHint, Self::Parameters> {
            NumericArray::from([
                ParameterHint::float(|h| h.logarithmic(true).minmax(0., 20000.)),
                ParameterHint::float(|h| h.minmax(0., 1.)),
                ParameterHint::Trigger,
            ])
        }

        fn param_apply(&mut self, _ctx: AudioCtx, index: usize, value: ParameterValue) {
            if matches!(value, ParameterValue::Smoothing(..)) {
                // eprintln!("Tried to set parameter smoothing with out a wrapper");
                return;
            }
            match index {
                0 => self.set_freq(F::from(value.float().unwrap()).unwrap()),
                1 => self.set_phase_offset(F::from(value.float().unwrap()).unwrap()),
                2 => self.reset_phase(),
                _ => (),
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
    fn set_freq(&mut self, freq: f64) {
        if self.freq_to_phase_step_mult == 0.0 {
            self.step = freq;
        } else {
            self.step = freq * self.freq_to_phase_step_mult;
        }
    }
}
impl<F: Float> UGen for Phasor<F> {
    type Sample = F;
    type Inputs = U0;
    type Outputs = U1;
    #[allow(missing_docs)]
    fn init(&mut self, ctx: &AudioCtx) {
        self.freq_to_phase_step_mult = 1.0_f64 / (ctx.sample_rate() as f64);
        self.set_freq(self.step);
    }

    fn process(
        &mut self,
        _ctx: AudioCtx,
        _flags: &mut UGenFlags,
        _input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        let mut out = Frame::default();
        out[0] = F::new(self.phase);
        self.phase += self.step;
        while self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        out
    }

    type Parameters = U1;

    fn param_descriptions()
    -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        ["freq"].into()
    }

    fn param_hints()
    -> knaster_primitives::numeric_array::NumericArray<crate::ParameterHint, Self::Parameters> {
        [ParameterHint::infinite_float()].into()
    }

    fn param_apply(&mut self, _ctx: AudioCtx, index: usize, value: crate::ParameterValue) {
        if index == 0 {
            self.set_freq(value.float().unwrap())
        }
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

impl<F: Float> SinNumeric<F> {
    pub fn new(freq: F) -> Self {
        Self {
            phase: freq,
            phase_offset: F::ZERO,
            phase_increment: F::ZERO,
        }
    }
    pub fn freq(&mut self, freq: F, sample_rate: F) {
        self.phase_increment = freq / sample_rate;
    }
    pub fn phase_offset(&mut self, phase_offset: F) {
        self.phase_offset = phase_offset;
    }
}

impl<F: Float> UGen for SinNumeric<F> {
    type Sample = F;
    type Inputs = U0;
    type Outputs = U1;

    fn init(&mut self, ctx: &AudioCtx) {
        // self.phase holds the freq set in the constructor, but only use it if the freq hasn't
        // been set any other way
        if self.phase_increment == F::ZERO {
            self.freq(self.phase, F::from(ctx.sample_rate).unwrap());
        }
        self.phase = F::ZERO;
    }

    fn process(
        &mut self,
        _ctx: AudioCtx,
        _flags: &mut UGenFlags,
        _input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        let out = ((self.phase + self.phase_offset) * F::TAU).sin();
        self.phase += self.phase_increment;
        if self.phase > F::ONE {
            self.phase -= F::ONE;
        }
        NumericArray::from([out])
    }
    type Parameters = U3;

    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        NumericArray::from(["freq", "phase_offset", "reset_phase"])
    }

    fn param_hints() -> NumericArray<ParameterHint, Self::Parameters> {
        NumericArray::from([
            ParameterHint::float(|h| h.logarithmic(true).nyquist()),
            ParameterHint::infinite_float(),
            ParameterHint::Trigger,
        ])
    }

    fn param_apply(&mut self, ctx: AudioCtx, index: usize, value: ParameterValue) {
        if matches!(value, ParameterValue::Smoothing(..)) {
            eprintln!("Tried to set parameter smoothing with out a wrapper");
            return;
        }
        match index {
            0 => self.freq(
                F::from(value.float().unwrap()).unwrap(),
                F::from(ctx.sample_rate()).unwrap(),
            ),
            1 => self.phase_offset(F::from(value.float().unwrap()).unwrap()),
            2 => self.phase_offset(F::ZERO),
            _ => (),
        }
    }

    fn param(
        &mut self,
        ctx: AudioCtx,
        param: impl Into<Param>,
        value: impl Into<ParameterValue>,
    ) -> Result<(), ParameterError> {
        let ctx = ctx.into();
        let var_name = match param.into() {
            Param::Index(i) => {
                if i >= Self::Parameters::USIZE {
                    return Err(ParameterError::ParameterIndexOutOfBounds);
                }
                self.param_apply(ctx, i, value.into());
                Ok(())
            }
            Param::Desc(desc) => {
                for (i, d) in Self::param_descriptions().into_iter().enumerate() {
                    if d == desc {
                        self.param_apply(ctx, i, value.into());
                        return Ok(());
                    }
                }
                Err(ParameterError::DescriptionNotFound(desc))
            }
        };
        var_name
    }
}
