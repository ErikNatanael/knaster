//! One pole filters make good and cheap lowpass 6dB/octave rolloff filters.
//! It is also good for removing zipping from parameter changes.

use crate::numeric_array::NumericArray;
use crate::typenum::U1;
use crate::{AudioCtx, ParameterRange, ParameterValue, UGen, UGenFlags};
use knaster_primitives::{Float, Frame};

// To use it as a DC blocker:
//
// `OnePole *dcBlockerLp = new OnePole(10.0 / sampleRate);`
// for each sample:
// `sample -= dcBlockerLp->process(sample);`
/// One pole filter. Used internally for the `OnePoleLpf` and `OnePoleHpf` Gens.
#[derive(Debug, Clone, Copy)]
pub struct OnePole<T: Float> {
    last_output: T,
    a0: T,
    b1: T,
}

impl<T: Float> OnePole<T> {
    /// Create a new reset OnePole
    pub fn new() -> Self {
        Self {
            last_output: T::new(0.0),
            a0: T::new(1.0),
            b1: T::new(0.0),
        }
    }
    /// Reset memory of last sample, keep coefficients
    #[inline]
    pub fn reset(&mut self) {
        self.last_output = T::zero();
    }
    /// Calculate coefficients for a lowpass OnePole
    #[inline]
    pub fn set_freq_lowpass(&mut self, freq: T, sample_rate: T) {
        // let freq: T = freq
        //     .max(T::zero())
        //     .min(sample_rate * T::from_f32(0.5).unwrap());
        // if freq > sample_rate * T::from_f32(0.5).unwrap() {
        //     println!("OnePole freq out of bounds: {freq}");
        // }
        let f: T = freq / sample_rate;
        let b_tmp: T = (T::new(-2.0_f64) * T::PI * f).exp();
        self.b1 = b_tmp;
        self.a0 = T::new(1.0_f64) - self.b1;
    }
    // TODO: Not verified to set the frequency correctly. In fact, I suspect it doesn't
    /// Calculate coefficients for a highpass OnePole
    #[inline]
    pub fn set_freq_highpass(&mut self, freq: T, sample_rate: T) {
        // let x = T::from_f32(2.).unwrap() * FloatConst::PI() * (freq / sample_rate);
        // let p = (T::from_f32(2.).unwrap() + x.cos())
        //     - ((T::from_f32(2.0).unwrap() + x.cos()).powi(2) - T::one()).sqrt();
        // self.b1 = p * T::from_f32(-1.0).unwrap();
        // self.a0 = p - T::one();
        // self.set_freq_lowpass(freq, sample_rate);
        // self.a0 = self.b1 - T::one();
        // self.b1 = self.b1 * T::from_f32(-1.0).unwrap();
        self.set_freq_lowpass(freq, sample_rate);
    }

    /// Process one sample assuming the OnePole is set to lowpass
    #[inline]
    pub fn process_lp(&mut self, input: T) -> T {
        unsafe {
            no_denormals::no_denormals(|| {
                self.last_output = input * self.a0 + self.last_output * self.b1;
            })
        }
        self.last_output
    }
    /// Process one sample assuming the OnePole is set to highpass
    #[inline]
    pub fn process_hp(&mut self, input: T) -> T {
        unsafe {
            no_denormals::no_denormals(|| {
                self.last_output = input * self.a0 + self.last_output * self.b1;
            })
        }
        input - self.last_output
    }
    /// A cheap, but pretty accurate approximation for compensating for the delay introduced by this filter on very short delay lengths.
    #[inline]
    pub fn cheap_tuning_compensation_lpf(&self) -> T {
        T::new(-2.) * (T::one() - self.b1).ln()
    }
    // /// Phase delay of a one pole filter?
    // pub fn phase_delay(fstringhz: T, fcutoffhz: T) -> T {
    //     fstringhz.atan2(fcutoffhz) * T::from_f32(-1.).unwrap()
    // }
}

impl<T: Float> Default for OnePole<T> {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Debug, Clone)]
/// One pole lowpass filter UGen
pub struct OnePoleLpf<F: Float> {
    /// The interval one pole filter implementation
    pub op: OnePole<F>,
}
impl<F: Float> OnePoleLpf<F> {
    #[allow(missing_docs)]
    pub fn new(cutoff_freq: F) -> Self {
        let mut op = OnePole::new();
        op.b1 = cutoff_freq;
        Self { op }
    }
}

impl<F: Float> UGen for OnePoleLpf<F> {
    type Sample = F;
    type Inputs = U1;
    type Outputs = U1;
    type Parameters = U1;
    fn init(&mut self, ctx: &AudioCtx) {
        // Only assume b1 is frequency if a0 is set to its standard value
        if self.op.a0 == F::ONE {
            let freq = self.op.b1;
            self.op
                .set_freq_lowpass(freq, F::new(ctx.sample_rate() as f32));
        }
    }
    fn process(
        &mut self,
        _ctx: AudioCtx,
        _flags: &mut UGenFlags,
        input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        [self.op.process_lp(input[0])].into()
    }

    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        ["cutoff_freq"].into()
    }

    fn param_range() -> NumericArray<ParameterRange, Self::Parameters> {
        [ParameterRange::Nyquist].into()
    }

    fn param_apply(&mut self, ctx: AudioCtx, index: usize, value: ParameterValue) {
        #[allow(clippy::single_match)]
        match index {
            0 => self.op.set_freq_lowpass(
                F::new(value.float().unwrap()),
                F::from(ctx.sample_rate).unwrap(),
            ),
            _ => (),
        }
    }
}

#[derive(Debug, Clone)]
/// One pole highpass filter UGen
pub struct OnePoleHpf<F: Float> {
    /// The interval one pole filter implementation
    pub op: OnePole<F>,
}
impl<F: Float> OnePoleHpf<F> {
    #[allow(missing_docs)]
    pub fn new() -> Self {
        Self { op: OnePole::new() }
    }
}

impl<F: Float> Default for OnePoleHpf<F> {
    fn default() -> Self {
        Self::new()
    }
}
impl<F: Float> UGen for OnePoleHpf<F> {
    type Sample = F;
    type Inputs = U1;
    type Outputs = U1;
    type Parameters = U1;
    fn process(
        &mut self,
        _ctx: AudioCtx,
        _flags: &mut UGenFlags,
        input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        [self.op.process_hp(input[0])].into()
    }

    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        ["cutoff_freq"].into()
    }

    fn param_range() -> NumericArray<ParameterRange, Self::Parameters> {
        [ParameterRange::Nyquist].into()
    }

    fn param_apply(&mut self, ctx: AudioCtx, index: usize, value: ParameterValue) {
        #[allow(clippy::single_match)]
        match index {
            0 => self.op.set_freq_highpass(
                F::new(value.float().unwrap()),
                F::from(ctx.sample_rate).unwrap(),
            ),
            _ => (),
        }
    }
}
