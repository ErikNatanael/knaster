use crate::core::marker::PhantomData;

use knaster_primitives::{
    numeric_array::NumericArray,
    typenum::{Unsigned, U0, U1, U3},
    Float, Frame,
};
#[cfg(feature = "alloc")]
use wavetable_vec::*;

use crate::{
    PFloat, Param, ParameterError, ParameterRange, ParameterType, ParameterValue, Parameterable,
};

use super::{AudioCtx, Gen};
#[cfg(feature = "alloc")]
mod wavetable_vec {
    use crate::dsp::wavetable::{Wavetable, WavetablePhase};

    /// Osciallator with an owned Wavetable
    /// *inputs*
    /// 0. "freq": The frequency of oscillation
    /// *outputs*
    /// 0. "sig": The signal
    #[derive(Debug, Clone)]
    pub struct WavetableOscillator<F> {
        step: u32,
        phase: WavetablePhase,
        wavetable: Wavetable<F>,
        amp: F,
        freq_to_phase_inc: f64,
        freq: F,
    }

    impl<F: Float> WavetableOscillator<F> {
        /// Set the frequency of the oscillation. This will be overwritten by the
        /// input frequency if used as a Gen.
        pub fn set_freq(&mut self, freq: F) {
            self.freq = freq;
            self.step = (freq as f64 * self.freq_to_phase_inc) as u32;
        }
        /// Set the amplitude of the signal.
        pub fn set_amp(&mut self, amp: Sample) {
            self.amp = amp;
        }
        /// Reset the phase of the oscillator.
        pub fn reset_phase(&mut self) {
            self.phase.0 = 0;
        }

        /// Generate the next sample given the current settings.
        #[inline(always)]
        #[must_use]
        pub fn next_sample(&mut self) -> Sample {
            // Use the phase to index into the wavetable
            // self.wavetable.get_linear_interp(temp_phase) * self.amp
            let sample = self.wavetable.get(self.phase, self.freq) * self.amp;
            self.phase.increase(self.step);
            sample
        }
    }

    #[impl_gen(range=normal)]
    impl WavetableOscillator {
        #[allow(missing_docs)]
        #[must_use]
        pub fn new(wavetable: Wavetable) -> Self {
            WavetableOscillator {
                step: 0,
                phase: WavetablePhase(0),
                wavetable,
                amp: 1.0,
                freq_to_phase_inc: 0.0, // set to a real value in init
                freq: 0.,
            }
        }
        fn process(&mut self, freq: &[Sample], sig: &mut [Sample]) -> GenState {
            assert!(freq.len() == sig.len());
            for (&freq, o) in freq.iter().zip(sig.iter_mut()) {
                self.set_freq(freq);
                *o = self.next_sample();
            }
            GenState::Continue
        }
        fn init(&mut self, sample_rate: SampleRate) {
            self.reset_phase();
            self.freq_to_phase_inc =
                TABLE_SIZE as f64 * FRACTIONAL_PART as f64 * (1.0 / sample_rate.to_f64());
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
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            step: 0.0,
            freq_to_phase_step_mult: 0.0,
            _phantom: PhantomData,
        }
    }
    pub fn set_freq(&mut self, freq: f64) {
        self.step = freq * self.freq_to_phase_step_mult;
    }
}
impl<F: Float> Gen for Phasor<F> {
    type Sample = F;
    type Inputs = U0;
    type Outputs = U1;
    #[allow(missing_docs)]
    fn init(&mut self, ctx: &AudioCtx) {
        self.freq_to_phase_step_mult = 1.0_f64 / (ctx.sample_rate() as f64);
    }

    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
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
}
impl<F: Float> Parameterable<F> for Phasor<F> {
    type Parameters = U1;

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        ["freq"].into()
    }

    fn param_default_values(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterValue, Self::Parameters>
    {
        [ParameterValue::Float(1.)].into()
    }

    fn param_range(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterRange, Self::Parameters>
    {
        [ParameterRange::Float(f64::NEG_INFINITY, f64::INFINITY)].into()
    }

    fn param_apply(&mut self, _ctx: &AudioCtx, index: usize, value: crate::ParameterValue) {
        if index == 0 {
            self.set_freq(value.float().unwrap())
        }
    }
}

/// Sine wave calculated using the trigonometric function for this platform, as opposed to using a shared lookup table
///
/// A lookup table is often faster, but (with libm) this implementation is available on every platform, even without allocation.
pub struct SinMath<F> {
    phase: F,
    phase_offset: F,
    phase_increment: F,
}
impl<F: Float> Default for SinMath<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: Float> SinMath<F> {
    pub fn new() -> Self {
        Self {
            phase: F::ZERO,
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

impl<F: Float> Gen for SinMath<F> {
    type Sample = F;
    type Inputs = U0;
    type Outputs = U1;

    fn init(&mut self, _ctx: &AudioCtx) {}

    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        let from = F::TAU;
        let out = (self.phase * from).sin();
        self.phase += self.phase_increment;
        if self.phase > F::ONE {
            self.phase -= F::ONE;
        }
        NumericArray::from([out])
    }
}

impl<F: Float> Parameterable<F> for SinMath<F> {
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

    fn param_default_values() -> NumericArray<ParameterValue, Self::Parameters> {
        NumericArray::from([
            ParameterValue::Float(440. as PFloat),
            ParameterValue::Float(0.),
            ParameterValue::Trigger,
        ])
    }

    fn param_range() -> NumericArray<ParameterRange, Self::Parameters> {
        todo!()
    }

    fn param_apply(&mut self, ctx: &AudioCtx, index: usize, value: ParameterValue) {
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
        ctx: &AudioCtx,
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
