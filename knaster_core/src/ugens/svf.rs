//! SVF filter for all your EQ needs
//!
//! Implemented based on [a technical paper by Andrew Simper, Cytomic, 2013](https://cytomic.com/files/dsp/SvfLinearTrapOptimised2.pdf) also available at <https://cytomic.com/technical-papers/>
//!

use crate::num_derive::{FromPrimitive, ToPrimitive};
use crate::{AudioCtx, PInteger};
use knaster_macros::{KnasterIntegerParameter, impl_ugen};
use knaster_primitives::Float;
use knaster_primitives::{PFloat, num_traits};
use std::prelude::v1::*;

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

#[impl_ugen]
impl<F: Float> SvfFilter<F> {
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
    #[allow(missing_docs)]
    #[param(kind = Frequency)]
    pub fn cutoff_freq(&mut self, cutoff_freq: PFloat, ctx: &AudioCtx) {
        self.cutoff_freq = F::new(cutoff_freq);
        self.set_coeffs(
            self.cutoff_freq,
            self.q,
            self.gain_db,
            F::from(ctx.sample_rate).unwrap(),
        );
    }
    #[allow(missing_docs)]
    #[param]
    pub fn q(&mut self, q: PFloat, ctx: &AudioCtx) {
        self.q = F::new(q);
        self.set_coeffs(
            self.cutoff_freq,
            self.q,
            self.gain_db,
            F::from(ctx.sample_rate).unwrap(),
        );
    }
    /// Set gain in dB
    #[param]
    pub fn gain(&mut self, gain_db: PFloat, ctx: &AudioCtx) {
        self.gain_db = F::new(gain_db);
        self.set_coeffs(
            self.cutoff_freq,
            self.q,
            self.gain_db,
            F::from(ctx.sample_rate).unwrap(),
        );
    }
    /// Set filter type
    #[param]
    pub fn filter(&mut self, filter: PInteger, ctx: &AudioCtx) {
        self.ty = SvfFilterType::from(filter);
        self.set_coeffs(
            self.cutoff_freq,
            self.q,
            self.gain_db,
            F::from(ctx.sample_rate).unwrap(),
        );
    }
    /// Trigger recalculations of coefficients
    #[param]
    pub fn t_calculate_coefficients(&mut self, ctx: &AudioCtx) {
        self.set_coeffs(
            self.cutoff_freq,
            self.q,
            self.gain_db,
            F::from(ctx.sample_rate).unwrap(),
        );
    }
    fn init(&mut self, sample_rate: u32, _block_size: usize) {
        self.set_coeffs(
            self.cutoff_freq,
            self.q,
            self.gain_db,
            F::from(sample_rate).unwrap(),
        );
    }
    fn process(&mut self, input: [F; 1]) -> [F; 1] {
        [self.process_sample(input[0])]
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

        #[cfg(feature = "no_denormals")]
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
        #[cfg(not(feature = "no_denormals"))]
        {
            let v3 = v0 - *ic2eq;
            let v1 = *a1 * *ic1eq + *a2 * v3;
            let v2 = *ic2eq + *a2 * *ic1eq + *a3 * v3;
            *ic1eq = F::new(2.) * v1 - *ic1eq;
            *ic2eq = F::new(2.) * v2 - *ic2eq;

            *m0 * v0 + *m1 * v1 + *m2 * v2
        }
    }
}
