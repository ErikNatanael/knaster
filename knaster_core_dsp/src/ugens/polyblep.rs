//! # PolyBlep
//!
//! PolyBlep UGen for anti-aliased waveforms using the polyblep method
//!
//! Ported from Martin Finke's C++ port of the PolyBLEP Waveform
//! generator from the Jesusonic code by Tale
//! (https://github.com/martinfinke/PolyBLEP)
// Ported from Martin Finke's C++ port of the port below (https://github.com/martinfinke/PolyBLEP)
/*
PolyBLEP Waveform generator ported from the Jesusonic code by Tale
http://www.taletn.com/reaper/mono_synth/

Permission has been granted to release this port under the WDL/IPlug license:

    This software is provided 'as-is', without any express or implied
    warranty.  In no event will the authors be held liable for any damages
    arising from the use of this software.

    Permission is granted to anyone to use this software for any purpose,
    including commercial applications, and to alter it and redistribute it
    freely, subject to the following restrictions:

    1. The origin of this software must not be misrepresented; you must not
       claim that you wrote the original software. If you use this software
       in a product, an acknowledgment in the product documentation would be
       appreciated but is not required.
    2. Altered source versions must be plainly marked as such, and must not be
       misrepresented as being the original software.
    3. This notice may not be removed or altered from any source distribution.
*/

use crate::core::ops::Mul;
use knaster_core::{
    Float, KnasterIntegerParameter, PFloat, PInteger, impl_ugen,
    num_derive::{FromPrimitive, ToPrimitive},
};
use std::prelude::v1::*;

fn square_number<T: Mul + Copy>(num: T) -> <T as Mul>::Output {
    num * num
}

// Adapted from "Phaseshaping Oscillator Algorithms for Musical Sound
// Synthesis" by Jari Kleimola, Victor Lazzarini, Joseph Timoney, and Vesa
// Valimaki.
// http://www.acoustics.hut.fi/publications/papers/smc2010-phaseshaping/
fn blep<F: Float>(t: F, dt: F) -> F {
    if t < dt {
        -square_number(t / dt - F::ONE)
    } else if t > F::ONE - dt {
        square_number((t - F::ONE) / dt + F::ONE)
    } else {
        F::ZERO
    }
}

// Derived from blep().
fn blamp<F: Float>(mut t: F, dt: F) -> F {
    if t < dt {
        t = t / dt - F::ONE;
        -F::ONE / F::new(3.) * square_number(t) * t
    } else if t > F::ONE - dt {
        t = (t - F::ONE) / dt + F::ONE;
        F::ONE / F::new(3.) * square_number(t) * t
    } else {
        F::ZERO
    }
}

fn bitwise_or_zero<F: Float>(t: F) -> F {
    t.trunc()
}
use knaster_core::num_traits;

#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    FromPrimitive,
    ToPrimitive,
    KnasterIntegerParameter,
)]
/// PolyBlep waveforms
#[num_traits = "num_traits"]
#[repr(u8)]
pub enum Waveform {
    #[allow(missing_docs)]
    #[default]
    Sawtooth = 0,
    #[allow(missing_docs)]
    Sine,
    #[allow(missing_docs)]
    Cosine,
    #[allow(missing_docs)]
    Triangle,
    #[allow(missing_docs)]
    Square,
    #[allow(missing_docs)]
    Rectangle,
    #[allow(missing_docs)]
    Ramp,
    #[allow(missing_docs)]
    ModifiedTriangle,
    #[allow(missing_docs)]
    ModifiedSquare,
    #[allow(missing_docs)]
    HalfWaveRectifiedSine,
    #[allow(missing_docs)]
    FullWaveRectifiedSine,
    #[allow(missing_docs)]
    TriangularPulse,
    #[allow(missing_docs)]
    TrapezoidFixed,
    #[allow(missing_docs)]
    TrapezoidVariable,
}

/// Anti-aliased waveforms using the polyblep method
///
/// Ported from Martin Finke's C++ port of the PolyBLEP Waveform
/// generator from the Jesusonic code by Tale
/// (https://github.com/martinfinke/PolyBLEP)
#[derive(Debug)]
pub struct PolyBlep<F: Copy = f32> {
    waveform: Waveform,
    sample_rate: F,
    freq_in_hz: F,
    freq_in_seconds_per_sample: F,
    pulse_width: F, // [0.0..1.0]
    t: F,           // The current phase [0.0..1.0) of the oscillator.
}
#[impl_ugen]
impl<F: Float> PolyBlep<F> {
    #[allow(missing_docs)]
    pub fn new(waveform: Waveform, freq: F) -> Self {
        Self {
            waveform,
            sample_rate: F::ZERO,
            freq_in_hz: freq,
            freq_in_seconds_per_sample: F::ZERO,
            pulse_width: F::new(0.5),
            t: F::ZERO,
        }
    }
    #[allow(missing_docs)]
    pub fn init(&mut self, sample_rate: u32, _block_size: usize) {
        self.sample_rate = F::from(sample_rate).unwrap();
        if self.freq_in_seconds_per_sample == F::ZERO && self.freq_in_hz != F::ZERO {
            self.set_freq(self.freq_in_hz);
        }
    }
    #[allow(missing_docs)]
    pub fn process(&mut self) -> [F; 1] {
        [self.get_and_inc()]
    }

    /// Set the frequency in Hz
    #[param(kind = Frequency)]
    pub fn freq(&mut self, freq_in_hz: PFloat) {
        self.set_freq(F::new(freq_in_hz));
    }
    /// Set the pulse width (ignored for waveforms that don't use it)
    #[param]
    pub fn pulse_width(&mut self, pulse_width: PFloat) {
        self.pulse_width = F::new(pulse_width);
    }
    /// Set the waveform
    #[param]
    pub fn waveform(&mut self, waveform: PInteger) {
        self.waveform = Waveform::from(waveform);
    }
    /// Set the time per sample
    pub fn set_dt(&mut self, time: F) {
        self.freq_in_seconds_per_sample = time;
    }
    #[inline]
    fn set_freq(&mut self, freq_in_hz: F) {
        self.freq_in_hz = F::new(freq_in_hz);
        self.set_dt(freq_in_hz / self.sample_rate);
    }

    /// Set the sample rate to a new value, preserving the frequency.
    pub fn set_sample_rate(&mut self, sample_rate: F) {
        let freq_in_hz = self.get_freq_in_hz();
        self.sample_rate = sample_rate;
        self.set_freq(freq_in_hz);
    }

    #[allow(missing_docs)]
    pub fn get_freq_in_hz(&self) -> F {
        self.freq_in_seconds_per_sample * self.sample_rate
    }

    /// Set the phase of this oscillator
    pub fn sync(&mut self, phase: F) {
        self.t = phase;
        if self.t >= F::ZERO {
            self.t -= bitwise_or_zero(self.t);
        } else {
            self.t += F::ONE - bitwise_or_zero(self.t);
        }
    }

    #[allow(missing_docs)]
    pub fn next_sample(&mut self) -> F {
        if self.get_freq_in_hz() >= self.sample_rate / F::new(4.) {
            self.sin()
        } else {
            match self.waveform {
                Waveform::Sine => self.sin(),
                Waveform::Cosine => self.cos(),
                Waveform::Triangle => self.tri(),
                Waveform::Square => self.sqr(),
                Waveform::Rectangle => self.rect(),
                Waveform::Sawtooth => self.saw(),
                Waveform::Ramp => self.ramp(),
                Waveform::ModifiedTriangle => self.tri2(),
                Waveform::ModifiedSquare => self.sqr2(),
                Waveform::HalfWaveRectifiedSine => self.half(),
                Waveform::FullWaveRectifiedSine => self.full(),
                Waveform::TriangularPulse => self.trip(),
                Waveform::TrapezoidFixed => self.trap(),
                Waveform::TrapezoidVariable => self.trap2(),
            }
        }
    }

    fn inc(&mut self) {
        self.t += self.freq_in_seconds_per_sample;
        self.t -= bitwise_or_zero(self.t);
    }

    fn get_and_inc(&mut self) -> F {
        let sample = self.next_sample();
        self.inc();
        sample
    }

    fn sin(&mut self) -> F {
        (self.t * F::TAU).sin()
    }

    fn cos(&mut self) -> F {
        (self.t * F::TAU).cos()
    }

    fn half(&mut self) -> F {
        let mut t2 = self.t + F::new(0.5);
        t2 -= bitwise_or_zero(t2);

        let mut y = if self.t < F::new(0.5) {
            F::new(2.0) * (self.t * F::TAU).sin() - F::new(2.0) / F::PI
        } else {
            -F::new(2.0) / F::PI
        };
        y += F::TAU
            * self.freq_in_seconds_per_sample
            * (blamp(self.t, self.freq_in_seconds_per_sample)
                + blamp(t2, self.freq_in_seconds_per_sample));

        y
    }

    fn full(&mut self) -> F {
        let mut _t = self.t + F::new(0.25);
        _t -= bitwise_or_zero(_t);

        let mut y = F::new(2.0) * (_t * F::PI).sin() - F::new(4.) / F::PI;
        y += F::TAU * self.freq_in_seconds_per_sample * blamp(_t, self.freq_in_seconds_per_sample);

        y
    }

    fn tri(&mut self) -> F {
        let mut t1 = self.t + F::new(0.25);
        t1 -= bitwise_or_zero(t1);

        let mut t2 = self.t + F::new(0.75);
        t2 -= bitwise_or_zero(t2);

        let mut y = self.t * F::new(4.);

        if y >= F::new(3.) {
            y -= F::new(4.);
        } else if y > F::ONE {
            y = F::new(2.0) - y;
        }

        y += F::new(4.)
            * self.freq_in_seconds_per_sample
            * (blamp(t1, self.freq_in_seconds_per_sample)
                - blamp(t2, self.freq_in_seconds_per_sample));

        y
    }

    fn tri2(&mut self) -> F {
        let pulse_width = self.pulse_width.min(F::new(0.9999)).max(F::new(0.0001));

        let mut t1 = self.t + F::new(0.5) * pulse_width;
        t1 -= bitwise_or_zero(t1);

        let mut t2 = self.t + F::ONE - F::new(0.5) * pulse_width;
        t2 -= bitwise_or_zero(t2);

        let mut y = self.t * F::new(2.0);

        if y >= F::new(2.0) - pulse_width {
            y = (y - F::new(2.0)) / pulse_width;
        } else if y >= pulse_width {
            y = F::ONE - (y - pulse_width) / (F::ONE - pulse_width);
        } else {
            y /= pulse_width;
        }

        y += self.freq_in_seconds_per_sample / (pulse_width - pulse_width * pulse_width)
            * (blamp(t1, self.freq_in_seconds_per_sample)
                - blamp(t2, self.freq_in_seconds_per_sample));

        y
    }

    fn trip(&mut self) -> F {
        let mut t1 = self.t + F::new(0.75) + F::new(0.5) * self.pulse_width;
        t1 -= bitwise_or_zero(t1);

        let mut y;
        if t1 >= self.pulse_width {
            y = -self.pulse_width;
        } else {
            y = F::new(4.) * t1;
            y = if y >= F::new(2.0) * self.pulse_width {
                F::new(4.) - y / self.pulse_width - self.pulse_width
            } else {
                y / self.pulse_width - self.pulse_width
            }
        }

        if self.pulse_width > F::new(0.) {
            let mut t2 = t1 + F::ONE - F::new(0.5) * self.pulse_width;
            t2 -= bitwise_or_zero(t2);

            let mut t3 = t1 + F::ONE - self.pulse_width;
            t3 -= bitwise_or_zero(t3);
            y += F::new(2.0) * self.freq_in_seconds_per_sample / self.pulse_width
                * (blamp(t1, self.freq_in_seconds_per_sample)
                    - F::new(2.0) * blamp(t2, self.freq_in_seconds_per_sample)
                    + blamp(t3, self.freq_in_seconds_per_sample));
        }
        y
    }

    fn trap(&mut self) -> F {
        let mut y = F::new(4.) * self.t;
        if y >= F::new(3.) {
            y -= F::new(4.);
        } else if y > F::ONE {
            y = F::new(2.0) - y;
        }
        y = (F::new(2.0) * y).clamp(-F::ONE, F::ONE);

        let mut t1 = self.t + F::new(0.125);
        t1 -= bitwise_or_zero(t1);

        let mut t2 = t1 + F::new(0.5);
        t2 -= bitwise_or_zero(t2);

        // Triangle #1
        y += F::new(4.)
            * self.freq_in_seconds_per_sample
            * (blamp(t1, self.freq_in_seconds_per_sample)
                - blamp(t2, self.freq_in_seconds_per_sample));

        t1 = self.t + F::new(0.375);
        t1 -= bitwise_or_zero(t1);

        t2 = t1 + F::new(0.5);
        t2 -= bitwise_or_zero(t2);

        // Triangle #2
        y += F::new(4.)
            * self.freq_in_seconds_per_sample
            * (blamp(t1, self.freq_in_seconds_per_sample)
                - blamp(t2, self.freq_in_seconds_per_sample));

        y
    }

    fn trap2(&mut self) -> F {
        let pulse_width = self.pulse_width.min(F::new(0.9999));
        let scale = F::ONE / (F::ONE - pulse_width);

        let mut y = F::new(4.) * self.t;
        if y >= F::new(3.) {
            y -= F::new(4.);
        } else if y > F::ONE {
            y = F::new(2.0) - y;
        }
        y = (scale * y).clamp(-F::ONE, F::ONE);

        let mut t1 = self.t + F::new(0.25) - F::new(0.25) * pulse_width;
        t1 -= bitwise_or_zero(t1);

        let mut t2 = t1 + F::new(0.5);
        t2 -= bitwise_or_zero(t2);

        // Triangle #1
        y += scale
            * F::new(2.0)
            * self.freq_in_seconds_per_sample
            * (blamp(t1, self.freq_in_seconds_per_sample)
                - blamp(t2, self.freq_in_seconds_per_sample));

        t1 = self.t + F::new(0.25) + F::new(0.25) * pulse_width;
        t1 -= bitwise_or_zero(t1);

        t2 = t1 + F::new(0.5);
        t2 -= bitwise_or_zero(t2);

        // Triangle #2
        y += scale
            * F::new(2.0)
            * self.freq_in_seconds_per_sample
            * (blamp(t1, self.freq_in_seconds_per_sample)
                - blamp(t2, self.freq_in_seconds_per_sample));

        y
    }

    fn sqr(&mut self) -> F {
        let mut t2 = self.t + F::new(0.5);
        t2 -= bitwise_or_zero(t2);

        let mut y = if self.t < F::new(0.5) {
            F::ONE
        } else {
            -F::ONE
        };
        y += blep(self.t, self.freq_in_seconds_per_sample)
            - blep(t2, self.freq_in_seconds_per_sample);

        y
    }

    fn sqr2(&mut self) -> F {
        let mut t1 = self.t + F::new(0.875) + F::new(0.25) * (self.pulse_width - F::new(0.5));
        t1 -= bitwise_or_zero(t1);

        let mut t2 = self.t + F::new(0.375) + F::new(0.25) * (self.pulse_width - F::new(0.5));
        t2 -= bitwise_or_zero(t2);

        // Square #1
        let mut y = if t1 < F::new(0.5) { F::ONE } else { -F::ONE };

        y += blep(t1, self.freq_in_seconds_per_sample) - blep(t2, self.freq_in_seconds_per_sample);

        t1 += F::new(0.5) * (F::ONE - self.pulse_width);
        t1 -= bitwise_or_zero(t1);

        t2 += F::new(0.5) * (F::ONE - self.pulse_width);
        t2 -= bitwise_or_zero(t2);

        // Square #2
        y += if t1 < F::new(0.5) { F::ONE } else { -F::ONE };

        y += blep(t1, self.freq_in_seconds_per_sample) - blep(t2, self.freq_in_seconds_per_sample);

        F::new(0.5) * y
    }

    fn rect(&mut self) -> F {
        let mut t2 = self.t + F::ONE - self.pulse_width;
        t2 -= bitwise_or_zero(t2);

        let mut y = -F::new(2.0) * self.pulse_width;
        if self.t < self.pulse_width {
            y += F::new(2.0);
        }

        y += blep(self.t, self.freq_in_seconds_per_sample)
            - blep(t2, self.freq_in_seconds_per_sample);

        y
    }

    fn saw(&mut self) -> F {
        let mut _t = self.t + F::new(0.5);
        _t -= bitwise_or_zero(_t);

        let mut y = F::new(2.0) * _t - F::ONE;
        y -= blep(_t, self.freq_in_seconds_per_sample);

        y
    }

    fn ramp(&mut self) -> F {
        let mut _t = self.t;
        _t -= bitwise_or_zero(_t);

        let mut y = F::ONE - F::new(2.0) * _t;
        y += blep(_t, self.freq_in_seconds_per_sample);

        y
    }
}
