//! SVF filter for all your EQ needs
//!
//! Implemented based on [a technical paper by Andrew Simper, Cytomic, 2013](https://cytomic.com/files/dsp/SvfLinearTrapOptimised2.pdf) also available at <https://cytomic.com/technical-papers/>
//!

use crate::num_derive::{FromPrimitive, ToPrimitive};
use crate::numeric_array::NumericArray;
use crate::typenum::{U1, U5};
use crate::{
    AudioCtx, PInteger, PIntegerConvertible, ParameterHint, ParameterValue, UGen, UGenFlags,
};
use knaster_macros::KnasterIntegerParameter;
use knaster_primitives::num_traits;
use knaster_primitives::{Float, Frame};

/// Different supported filter types
#[derive(
    Clone, Copy, Debug, FromPrimitive, ToPrimitive, PartialEq, Default, KnasterIntegerParameter,
)]
#[num_traits = "num_traits"]
#[repr(u8)]
pub enum SvfFilterType {
    #[default]
    #[allow(missing_docs)]
    Low = 0,
    #[allow(missing_docs)]
    High,
    #[allow(missing_docs)]
    Band,
    #[allow(missing_docs)]
    Notch,
    #[allow(missing_docs)]
    Peak,
    #[allow(missing_docs)]
    All,
    #[allow(missing_docs)]
    Bell,
    #[allow(missing_docs)]
    LowShelf,
    #[allow(missing_docs)]
    HighShelf,
}
/// A versatile EQ filter implementation
///
/// Implemented based on [a technical paper by Andrew Simper, Cytomic, 2013](https://cytomic.com/files/dsp/SvfLinearTrapOptimised2.pdf) also available at <https://cytomic.com/technical-papers/>
#[derive(Clone, Debug)]
pub struct SvfFilter<F: Copy> {
    ty: SvfFilterType,
    cutoff_freq: F,
    q: F,
    gain_db: F,
    // state
    ic1eq: F,
    ic2eq: F,
    // coefficients
    a1: F,
    a2: F,
    a3: F,
    m0: F,
    m1: F,
    m2: F,
}

impl<F: Float> SvfFilter<F> {
    const CUTOFF_FREQ: usize = 0;
    const Q: usize = 1;
    const GAIN: usize = 2;
    const FILTER: usize = 3;
    const T_CALCULATE_COEFFICIENTS: usize = 4;
    #[allow(missing_docs)]
    pub fn new(ty: SvfFilterType, cutoff_freq: F, q: F, gain_db: F) -> Self {
        Self {
            ic1eq: F::ZERO,
            ic2eq: F::ZERO,
            a1: F::ZERO,
            a2: F::ZERO,
            a3: F::ZERO,
            m0: F::ZERO,
            m1: F::ZERO,
            m2: F::ZERO,
            ty,
            cutoff_freq,
            q,
            gain_db,
        }
    }
    /// Set the coefficients for the currently set filter type. `gain_db` is only used for Bell, HighShelf and LowShelf.
    pub fn set_coeffs(&mut self, cutoff: F, q: F, gain_db: F, sample_rate: F) {
        match self.ty {
            SvfFilterType::Low => {
                let g = ((F::PI * cutoff) / sample_rate).tan();
                let k = F::ONE / q;
                self.a1 = F::ONE / (F::ONE + g * (g + k));
                self.a2 = g * self.a1;
                self.a3 = g * self.a2;
                self.m0 = F::ZERO;
                self.m1 = F::ZERO;
                self.m2 = F::ONE;
            }
            SvfFilterType::Band => {
                let g = ((F::PI * cutoff) / sample_rate).tan();
                let k = F::ONE / q;
                self.a1 = F::ONE / (F::ONE + g * (g + k));
                self.a2 = g * self.a1;
                self.a3 = g * self.a2;
                self.m0 = F::ZERO;
                self.m1 = F::ONE;
                self.m2 = F::ZERO;
            }
            SvfFilterType::High => {
                let g = ((F::PI * cutoff) / sample_rate).tan();
                let k = F::ONE / q;
                self.a1 = F::ONE / (F::ONE + g * (g + k));
                self.a2 = g * self.a1;
                self.a3 = g * self.a2;
                self.m0 = F::ONE;
                self.m1 = -k;
                self.m2 = -F::ONE;
            }
            SvfFilterType::Notch => {
                let g = ((F::PI * cutoff) / sample_rate).tan();
                let k = F::ONE / q;
                self.a1 = F::ONE / (F::ONE + g * (g + k));
                self.a2 = g * self.a1;
                self.a3 = g * self.a2;
                self.m0 = F::ONE;
                self.m1 = -k;
                self.m2 = F::ZERO;
            }
            SvfFilterType::Peak => {
                let g = ((F::PI * cutoff) / sample_rate).tan();
                let k = F::ONE / q;
                self.a1 = F::ONE / (F::ONE + g * (g + k));
                self.a2 = g * self.a1;
                self.a3 = g * self.a2;
                self.m0 = F::ONE;
                self.m1 = -k;
                self.m2 = -F::new(2.);
            }
            SvfFilterType::All => {
                let g = ((F::PI * cutoff) / sample_rate).tan();
                let k = F::ONE / q;
                self.a1 = F::ONE / (F::ONE + g * (g + k));
                self.a2 = g * self.a1;
                self.a3 = g * self.a2;
                self.m0 = F::ONE;
                self.m1 = -F::new(2.) * k;
                self.m2 = F::ZERO;
            }
            SvfFilterType::Bell => {
                let amp = F::new(10.0).powf(gain_db / F::new(40.));
                let g = ((F::PI * cutoff) / sample_rate).tan() / amp.sqrt();
                let k = F::ONE / (q * amp);
                self.a1 = F::ONE / (F::ONE + g * (g + k));
                self.a2 = g * self.a1;
                self.a3 = g * self.a2;
                self.m0 = F::ONE;
                self.m1 = k * (amp * amp - F::ONE);
                self.m2 = F::ZERO;
            }
            SvfFilterType::LowShelf => {
                let amp = F::new(10.0).powf(gain_db / F::new(40.));
                let g = ((F::PI * cutoff) / sample_rate).tan() / amp.sqrt();
                let k = F::ONE / q;
                self.a1 = F::ONE / (F::ONE + g * (g + k));
                self.a2 = g * self.a1;
                self.a3 = g * self.a2;
                self.m0 = F::ONE;
                self.m1 = k * (amp - F::ONE);
                self.m2 = amp * amp - F::ONE;
            }
            SvfFilterType::HighShelf => {
                let amp = F::new(10.0).powf(gain_db / F::new(40.));
                let g = ((F::PI * cutoff) / sample_rate).tan() * amp.sqrt();
                let k = F::ONE / q;
                self.a1 = F::ONE / (F::ONE + g * (g + k));
                self.a2 = g * self.a1;
                self.a3 = g * self.a2;
                self.m0 = amp * amp;
                self.m1 = k * (F::ONE - amp) * amp;
                self.m2 = F::ONE - amp * amp;
            }
        }
    }
    // TODO: This is vectorisable such that multiple filters can be run at once, e.g. multiple channels with the same coefficients
    #[allow(missing_docs)]
    pub fn process_sample(&mut self, v0: F) -> F {
        let SvfFilter {
            ic1eq,
            ic2eq,
            a1,
            a2,
            a3,
            m0,
            m1,
            m2,
            ..
        } = self;

        unsafe {
            no_denormals::no_denormals(|| {
                let v3 = v0 - *ic2eq;
                let v1 = *a1 * *ic1eq + *a2 * v3;
                let v2 = *ic2eq + *a2 * *ic1eq + *a3 * v3;
                *ic1eq = F::new(2.) * v1 - *ic1eq;
                *ic2eq = F::new(2.) * v2 - *ic2eq;

                *m0 * v0 + *m1 * v1 + *m2 * v2
            })
        }
    }
}
impl<F: Float> UGen for SvfFilter<F> {
    type Sample = F;
    type Inputs = U1;
    type Outputs = U1;
    type Parameters = U5;

    fn init(&mut self, sample_rate: u32, block_size: usize) {
        self.set_coeffs(
            self.cutoff_freq,
            self.q,
            self.gain_db,
            F::from(sample_rate).unwrap(),
        );
    }
    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        [self.process_sample(input[0])].into()
    }
    fn param_hints() -> NumericArray<ParameterHint, Self::Parameters> {
        [
            ParameterHint::float(|h| h.nyquist()),
            ParameterHint::positive_infinite_float(),
            ParameterHint::infinite_float(),
            ParameterHint::from_pinteger_enum::<SvfFilterType>(),
            ParameterHint::Trigger,
        ]
        .into()
    }
    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        [
            "cutoff_freq",
            "q",
            "gain_db",
            "filter",
            "t_calculate_coefficients",
        ]
        .into()
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
        match index {
            Self::CUTOFF_FREQ => {
                self.cutoff_freq = F::new(value.float().unwrap());
                self.set_coeffs(
                    self.cutoff_freq,
                    self.q,
                    self.gain_db,
                    F::from(ctx.sample_rate).unwrap(),
                );
            }
            Self::Q => {
                self.q = F::new(value.float().unwrap());
                self.set_coeffs(
                    self.cutoff_freq,
                    self.q,
                    self.gain_db,
                    F::from(ctx.sample_rate).unwrap(),
                );
            }
            Self::GAIN => {
                self.gain_db = F::new(value.float().unwrap());

                self.set_coeffs(
                    self.cutoff_freq,
                    self.q,
                    self.gain_db,
                    F::from(ctx.sample_rate).unwrap(),
                );
            }
            Self::FILTER => {
                self.ty = SvfFilterType::from(value.integer().unwrap());
                self.set_coeffs(
                    self.cutoff_freq,
                    self.q,
                    self.gain_db,
                    F::from(ctx.sample_rate).unwrap(),
                );
            }
            Self::T_CALCULATE_COEFFICIENTS => self.set_coeffs(
                self.cutoff_freq,
                self.q,
                self.gain_db,
                F::from(ctx.sample_rate).unwrap(),
            ),
            _ => (),
        }
    }
}
