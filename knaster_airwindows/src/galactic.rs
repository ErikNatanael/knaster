//! Galactic reverb
//!
//! ported from airwindows Galactic plugin
//! License: MIT
// Original code: Copyright (c) 2016 airwindows, Airwindows uses the MIT license
// Ported code: Copyright 2023 Erik Natanael Gustafsson

use std::iter::Sum;

use knaster_core::{
    delay::StaticSampleDelay,
    noise::next_randomness_seed,
    typenum::{U2, U5},
    AudioCtx, Float, ParameterRange, Seconds, UGen,
};

pub struct Galactic<F> {
    delays_left: [StaticSampleDelay<F>; 12],
    delays_right: [StaticSampleDelay<F>; 12],
    feedback: [[F; 4]; 2],
    detune_delay_left: StaticSampleDelay<F>,
    detune_delay_right: StaticSampleDelay<F>,
    lowpass_pre: [F; 2],
    lowpass_post: [F; 2],
    fpd_l: u32,
    fpd_r: u32,
    oldfpd: f64,
    vib_m: f64,
    iir_al: F,
    iir_ar: F,
    iir_bl: F,
    iir_br: F,
    overallscale: F,
    // parameters
    brightness: F,
    wet: F,
    bigness: F,
    replace: F,
    detune: F,
}

const GALACTIC_DELAY_TIMES: [usize; 12] = [
    6480, 3660, 1720, 680, 9700, 6000, 2320, 940, 15220, 8460, 4540, 3200,
];

impl<F: Float> UGen for Galactic<F> {
    type Sample = F;

    type Inputs = U2;

    type Outputs = U2;

    type Parameters = U5;

    fn init(&mut self, ctx: &AudioCtx) {
        for (delay, time) in self.delays_left.iter_mut().zip(GALACTIC_DELAY_TIMES) {
            let time = ((time as f64 / 44100.) * ctx.sample_rate() as f64) as usize;
            *delay = StaticSampleDelay::new(time);
        }
        for (delay, time) in self.delays_right.iter_mut().zip(GALACTIC_DELAY_TIMES) {
            let time = ((time as f64 / 44100.) * ctx.sample_rate() as f64) as usize;
            *delay = StaticSampleDelay::new(time);
        }
        // self.detune_delay_left =
        //     StaticSampleDelay::new((0.07054421768707483 * *sample_rate) as usize);
        // self.detune_delay_right =
        //     StaticSampleDelay::new((0.07054421768707483 * *sample_rate) as usize);
        self.detune_delay_left = StaticSampleDelay::new(256);
        self.detune_delay_right = StaticSampleDelay::new(256);
        self.lowpass_pre = [F::ZERO, F::ZERO];
        self.lowpass_post = [F::ZERO, F::ZERO];

        let mut overallscale = 1.0;
        overallscale /= 44100.0;
        overallscale *= ctx.sample_rate() as f64;
        self.overallscale = F::new(overallscale);
    }

    fn process(
        &mut self,
        _ctx: knaster_core::AudioCtx,
        _flags: &mut knaster_core::UGenFlags,
        input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        let left_in = [input[0]];
        let right_in = [input[1]];
        let mut left_out = [F::ZERO];
        let mut right_out = [F::ZERO];
        self.process(&left_in, &right_in, &mut left_out, &mut right_out);
        [left_out[0], right_out[0]].into()
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        _ctx: knaster_core::BlockAudioCtx,
        _flags: &mut knaster_core::UGenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_core::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_core::Block<Sample = Self::Sample>,
    {
        let mut outs = output.iter_mut();
        let o0 = outs.next().unwrap();
        let o1 = outs.next().unwrap();
        self.process(input.channel_as_slice(0), input.channel_as_slice(1), o0, o1);
    }

    fn param_range(
    ) -> knaster_core::numeric_array::NumericArray<knaster_core::ParameterRange, Self::Parameters>
    {
        [
            ParameterRange::one(),
            ParameterRange::one(),
            ParameterRange::one(),
            ParameterRange::one(),
            ParameterRange::one(),
        ]
        .into()
    }
    fn param_descriptions(
    ) -> knaster_core::numeric_array::NumericArray<&'static str, Self::Parameters> {
        ["replace", "detune", "brightness", "bigness", "wet"].into()
    }

    fn param_apply(
        &mut self,
        _ctx: knaster_core::AudioCtx,
        index: usize,
        value: knaster_core::ParameterValue,
    ) {
        match index {
            Self::REPLACE => self.replace = value.f().unwrap(),
            Self::DETUNE => self.detune = value.f().unwrap(),
            Self::BRIGHTNESS => self.brightness = value.f().unwrap(),
            Self::BIGNESS => self.bigness = value.f().unwrap(),
            Self::WET => self.wet = value.f().unwrap(),
            _ => (),
        }
    }
}
impl<F: Float> Galactic<F> {
    pub const REPLACE: usize = 0;
    pub const DETUNE: usize = 1;
    pub const BRIGHTNESS: usize = 2;
    pub const BIGNESS: usize = 3;
    pub const WET: usize = 4;
    pub fn new(replace: F, detune: F, brightness: F, bigness: F, wet: F) -> Self {
        let mut rng = fastrand::Rng::with_seed(next_randomness_seed());
        Self {
            delays_left: std::array::from_fn(|_| StaticSampleDelay::new(1)),
            delays_right: std::array::from_fn(|_| StaticSampleDelay::new(1)),
            detune_delay_left: StaticSampleDelay::new(1),
            detune_delay_right: StaticSampleDelay::new(1),
            lowpass_pre: [F::ZERO, F::ZERO],
            lowpass_post: [F::ZERO, F::ZERO],
            fpd_l: rng.u32(16386..u32::MAX),
            fpd_r: rng.u32(16386..u32::MAX),
            vib_m: 3.,
            feedback: [[F::ZERO; 4]; 2],
            oldfpd: 429496.7295,
            iir_al: F::ZERO,
            iir_ar: F::ZERO,
            iir_bl: F::ZERO,
            iir_br: F::ZERO,
            overallscale: F::ZERO,
            // parameters
            brightness,
            wet,
            bigness,
            replace,
            detune,
        }
    }
    // #[replace_float_literals(F::from(literal).expect("Literal must fit in T"))]
    pub fn process(&mut self, left: &[F], right: &[F], left_out: &mut [F], right_out: &mut [F]) {
        // double regen = 0.0625+((1.0-A)*0.0625); // High (0.125) if Replace is low
        // double attenuate = (1.0 - (regen / 0.125))*1.333; // 1.33 if regen is low / replace is high

        let regen = F::new(0.0625) + ((F::ONE - self.replace) * F::new(0.0625));
        let attenuate = (F::ONE - (regen / F::new(0.125))) * F::new(1.333); // 1.33 if regen is high / replace is low
        let lowpass =
            (F::new(1.00001) - (F::ONE - self.brightness)).powi(2) / (self.overallscale).sqrt(); // (0.00001 + Brightness).powi(2)/overallscale.sqrt()
        let drift = self.detune.powi(3) * F::new(0.001); // Detune.powi(3) * 0.001
        let size = (self.bigness * F::new(0.9)) + F::new(0.1);
        let wet = F::ONE - (F::ONE - self.wet).powi(3);

        for (delay_left, delay_right) in self
            .delays_left
            .iter_mut()
            .zip(self.delays_right.iter_mut())
        {
            delay_left.set_delay_length_fraction(size);
            delay_right.set_delay_length_fraction(size);
        }

        // let lengths = [3407., 1823., 859., 331., 4801., 2909., 1153., 461., 7607., 4217., 2269., 1597.];
        // for ((left, right), len) in self.delays_left.iter_mut().zip(self.delays_right.iter_mut()).zip(lengths) {
        //     let len = (len * size) as usize;
        //     left.set_delay_length(len);
        //     right.set_delay_length(len);
        // }

        for (((&input_sample_l, &input_sample_r), output_l), output_r) in left
            .iter()
            .zip(right.iter())
            .zip(left_out.iter_mut())
            .zip(right_out.iter_mut())
        {
            // # Per sample:
            // - If the input is very faint, use the fpd values instead (floating point dither, similar to the last output sample)

            // Apply dither
            let input_sample_l = if input_sample_l.abs().to_f64() < 1.18e-23 {
                F::from(self.fpd_l as f64 * 1.18e-17).unwrap()
            } else {
                input_sample_l
            };
            let input_sample_r = if input_sample_r.abs().to_f64() < 1.18e-23 {
                F::new(self.fpd_r as f64 * 1.18e-17)
            } else {
                input_sample_r
            };
            let dry_sample_l = input_sample_l;
            let dry_sample_r = input_sample_r;

            // - vibM cycles 0. - TAU, speed depending on drift (Detune) and the fpdL value last time it reset
            // vibM is phase 0-TAU, speed dpends on drift and fpd
            self.vib_m += self.oldfpd * drift.to_f64();
            if self.vib_m > (3.141592653589793238 * 2.0) {
                self.vib_m = 0.0;
                self.oldfpd = 0.4294967295 + (self.fpd_l as f64 * 0.0000000000618);
            }

            // - set the fixed size delay (256 frames) to the inputSample at the current position
            self.detune_delay_left
                .write_and_advance(input_sample_l * attenuate);
            self.detune_delay_right
                .write_and_advance(input_sample_r * attenuate);
            // - Get a sample from the aM buffer (lin interp)
            let vib_m_sin = self.vib_m.sin(); // TODO: replace by something faster
            let offset_ml = ((vib_m_sin) + 1.0) * 127.; // 0-256
            let offset_mr = ((self.vib_m + (3.141592653589793238 / 2.0)).sin() + 1.0) * 127.; // 0-256 90 degrees phase shifted
            let working_ml = self.detune_delay_left.position as f64 + offset_ml;
            let working_mr = self.detune_delay_right.position as f64 + offset_mr;
            let input_sample_l = self.detune_delay_left.read_at_lin(F::new(working_ml));
            let input_sample_r = self.detune_delay_right.read_at_lin(F::new(working_mr));
            // - Apply a lowpass filter to the output from the M delay (iirA variable)
            self.iir_al = (self.iir_al * (F::ONE - lowpass)) + (input_sample_l * lowpass);
            let input_sample_l = self.iir_al;
            self.iir_ar = (self.iir_ar * (F::ONE - lowpass)) + (input_sample_r * lowpass);
            let input_sample_r = self.iir_ar;
            // - Only calculate a new reverb sample once every 4 samples if SR is 44100*4

            // Reverb sample:
            // Set I-L delays for the input + respective feedback from last cycle for the opposite channel (left for right, right for left)
            // BLOCK 0

            for i in 0..4 {
                self.delays_left[i]
                    .write_and_advance((self.feedback[1][i] * regen) + input_sample_l);
            }
            for i in 0..4 {
                self.delays_right[i]
                    .write_and_advance((self.feedback[0][i] * regen) + input_sample_r);
            }

            let mut block_0_l = [F::ZERO; 4];
            for i in 0..4 {
                block_0_l[i] = self.delays_left[i].read();
            }
            let mut block_0_r = [F::ZERO; 4];
            for i in 0..4 {
                block_0_r[i] = self.delays_right[i].read();
            }
            // BLOCK 1

            for i in 0..4 {
                self.delays_left[i + 4].write_and_advance(
                    block_0_l[0 + i]
                        - (block_0_l[(1 + i) % 4]
                            + block_0_l[(2 + i) % 4]
                            + block_0_l[(3 + i) % 4]),
                );
            }
            for i in 0..4 {
                self.delays_right[i + 4].write_and_advance(
                    block_0_r[0 + i]
                        - (block_0_r[(1 + i) % 4]
                            + block_0_r[(2 + i) % 4]
                            + block_0_r[(3 + i) % 4]),
                );
            }

            let mut block_1_l = [F::ZERO; 4];
            for i in 0..4 {
                block_1_l[i] = self.delays_left[i + 4].read();
            }
            let mut block_1_r = [F::ZERO; 4];
            for i in 0..4 {
                block_1_r[i] = self.delays_right[i + 4].read();
            }

            // BLOCK 2

            for i in 0..4 {
                self.delays_left[i + 8].write_and_advance(
                    block_1_l[0 + i]
                        - (block_1_l[(1 + i) % 4]
                            + block_1_l[(2 + i) % 4]
                            + block_1_l[(3 + i) % 4]),
                );
            }
            for i in 0..4 {
                self.delays_right[i + 8].write_and_advance(
                    block_1_r[0 + i]
                        - (block_1_r[(1 + i) % 4]
                            + block_1_r[(2 + i) % 4]
                            + block_1_r[(3 + i) % 4]),
                );
            }

            let mut block_2_l = [F::ZERO; 4];
            for i in 0..4 {
                block_2_l[i] = self.delays_left[i + 8].read();
            }
            let mut block_2_r = [F::ZERO; 4];
            for i in 0..4 {
                block_2_r[i] = self.delays_right[i + 8].read();
            }

            // Set feedback
            for i in 0..4 {
                self.feedback[0][i] = block_2_l[i]
                    - (block_2_l[(1 + i) % 4] + block_2_l[(2 + i) % 4] + block_2_l[(3 + i) % 4]);
            }
            for i in 0..4 {
                self.feedback[1][i] = block_2_r[i]
                    - (block_2_r[(1 + i) % 4] + block_2_r[(2 + i) % 4] + block_2_r[(3 + i) % 4]);
            }

            let input_sample_l = block_2_l.iter().copied().sum::<F>() * F::new(0.125);
            let input_sample_r = block_2_r.iter().copied().sum::<F>() * F::new(0.125);

            // Get the output from I-L delays
            // Set A-D delays to a mixing configuration of the I-L outputs e.g. I - (J+K+L);
            // Same thing for E-H
            // Feedback delays are this same mixing of the outputs of E-H
            // For large sample rates, use linear interpolation to the new value, otherwise the sum of EFGH/8.
            //
            // Apply another lowpass to the reverbed value

            self.iir_bl = (self.iir_bl * (F::ONE - lowpass)) + input_sample_l * lowpass;
            let mut input_sample_l = self.iir_bl;
            self.iir_br = (self.iir_br * (F::ONE - lowpass)) + (input_sample_r * lowpass);
            let mut input_sample_r = self.iir_br;

            if wet < F::ONE {
                input_sample_l = (input_sample_l * wet) + (dry_sample_l * (F::ONE - wet));
                input_sample_r = (input_sample_r * wet) + (dry_sample_r * (F::ONE - wet));
            }

            let (_mantissa_l, exp_l) = frexp(input_sample_l.to_f32());
            let mut fpd_l = self.fpd_l;
            fpd_l ^= fpd_l << 13;
            fpd_l ^= fpd_l >> 17;
            fpd_l ^= fpd_l << 5;
            input_sample_l += F::new(
                ((fpd_l as f64) - (0x7fffffff_u32) as f64)
                    * 5.5e-36
                    * (2_u64.pow(exp_l + 62) as f64),
            );
            self.fpd_l = fpd_l;

            let (_mantissa_r, exp_r) = frexp(input_sample_r.to_f32());
            let mut fpd_r = self.fpd_r;
            fpd_r ^= fpd_r << 13;
            fpd_r ^= fpd_r >> 17;
            fpd_r ^= fpd_r << 5;
            input_sample_r += F::new(
                ((fpd_r as f64) - (0x7fffffff_u32) as f64)
                    * 5.5e-36
                    * (2_u64.pow(exp_r + 62) as f64),
            );
            self.fpd_r = fpd_r;

            *output_l = input_sample_l;
            *output_r = input_sample_r;
        }
    }
}

fn frexp(s: f32) -> (f32, u32) {
    if 0.0 == s {
        (s, 0)
    } else {
        let lg = s.abs().log2();
        let x = (lg - lg.floor() - 1.0).exp2();
        let exp = lg.floor() + 1.0;
        (s.signum() * x, exp as u32)
    }
}
