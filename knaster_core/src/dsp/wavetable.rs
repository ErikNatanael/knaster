//! Contains an anti-aliasing wavetable implementation [`Wavetable`] with related utilities.

/// Decides the number of samples per [`Wavetable`] buffer, and therefore also
/// the number of high bits used for the phase indexing into the wavetable. With
/// the current u32 phase, this can be maximum 16.
pub const TABLE_POWER: u32 = 14;
/// TABLE_SIZE is 2^TABLE_POWER
pub const TABLE_SIZE: usize = 2_usize.pow(TABLE_POWER);
/// The high mask is used to 0 everything above the table size so that adding
/// further would have the same effect as wrapping.
pub const TABLE_HIGH_MASK: u32 = TABLE_SIZE as u32 - 1;
/// Max number of the fractional part of a integer phase. Currently, 16 bits are used for the fractional part.
pub const FRACTIONAL_PART: u32 = 65536;

/// Fixed point phase, making use of the TABLE_* constants; compatible with Wavetable
///
/// In benchmarks, fixed point phase was significantly faster than floating poing phase.
#[derive(Debug, Clone, Copy)]
pub struct WavetablePhase(pub u32);

impl WavetablePhase {
    /// Returns just the integer component of the phase.
    #[must_use]
    #[inline]
    pub fn integer_component(&self) -> usize {
        // This will fill with zeroes unless going above 31 bits of shift, in
        // which case it will overflow. The mask will remove anything above the
        // bits we use for our table size, so we don't need 2^16 size tables.
        ((self.0 >> 16) & TABLE_HIGH_MASK) as usize
    }
    /// Returns the fractional component, but as the lower bits of a u32.
    #[must_use]
    #[inline]
    pub fn fractional_component(&self) -> u32 {
        const FRACTIONAL_MASK: u32 = u16::MAX as u32;
        self.0 & FRACTIONAL_MASK
    }
    /// Returns the fractional component of the phase.
    #[must_use]
    #[inline]
    pub fn fractional_component_f32(&self) -> f32 {
        const FRACTIONAL_MASK: u32 = u16::MAX as u32;
        (self.0 & FRACTIONAL_MASK) as f32 / FRACTIONAL_MASK as f32
    }
    /// Increase the phase by the given step. The step should be the
    /// frequency/sample_rate * [`TABLE_SIZE`] * [`FRACTIONAL_PART`].
    #[inline]
    pub fn increase(&mut self, add: u32) {
        self.0 = self.0.wrapping_add(add);
    }
}
impl core::ops::Add for WavetablePhase {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.wrapping_add(rhs.0))
    }
}

#[cfg(any(feature = "alloc", feature = "std"))]
pub use wavetable_vec::*;
#[cfg(any(feature = "alloc", feature = "std"))]
mod wavetable_vec {
    use super::*;
    use crate::core::f64::consts::PI;
    use crate::dsp::xorrng::XOrShift32Rng;

    #[cfg(all(feature = "alloc", not(feature = "std")))]
    use alloc::vec;
    #[cfg(all(feature = "alloc", not(feature = "std")))]
    use alloc::vec::Vec;

    use knaster_primitives::Float;
    #[cfg(feature = "std")]
    use std::vec;
    #[cfg(feature = "std")]
    use std::vec::Vec;

    /// Non-anti-aliased wavetable.
    ///
    /// It is used in [`Wavetable`] to store some partial range of harmonics of
    /// the full waveform.
    #[derive(Debug, Clone)]
    pub struct NonAaWavetable<F> {
        buffer: Vec<F>,      // Box<[Sample; 131072]>,
        diff_buffer: Vec<F>, // Box<[Sample; 131072]>,
    }

    impl<F: Float> Default for NonAaWavetable<F> {
        fn default() -> Self {
            let buffer = vec![F::ZERO; TABLE_SIZE];
            let diff_buffer = vec![F::ZERO; TABLE_SIZE];
            Self {
                buffer,
                diff_buffer,
            }
        }
    }
    impl<F: Float> NonAaWavetable<F> {
        pub fn new() -> Self {
            Self::default()
        }
        /// Recalculate the difference between samples in the buffer.
        ///
        /// The [`PartialTable`] contains a buffer with the difference between each
        /// sample of the buffer for efficiency reason.
        pub fn update_diff_buffer(&mut self) {
            // let diff_buffer: Vec<Sample> = self
            //     .buffer
            //     .iter()
            //     .zip(self.buffer.iter().skip(1).cycle())
            //     .map(|(&a, &b)| a - b)
            //     .collect();
            let mut diff_buffer = vec![F::ZERO; self.buffer.len()];
            for (i, diff) in diff_buffer.iter_mut().enumerate() {
                *diff = self.buffer[(i + 1) % self.buffer.len()] - self.buffer[i];
            }
            assert_eq!(self.buffer[1] - self.buffer[0], diff_buffer[0]);
            assert_eq!(
                self.buffer[0] - *self.buffer.iter().last().unwrap(),
                *diff_buffer.iter().last().unwrap()
            );
            self.diff_buffer = diff_buffer;
        }
        /// Create a [`PartialTable`] from an existing buffer.
        ///
        /// # Errors
        /// The buffer has to be of [`TABLE_SIZE`] length, otherwise an error will be returned.
        pub fn set_from_buffer(&mut self, buffer: Vec<F>) {
            // TODO: FFT of the buffer for anti-aliasing
            self.buffer = buffer;
            self.update_diff_buffer();
        }
        /// Create a new wavetable containing a sine wave. For audio, you often want a cosine instead since it starts at 0 to avoid discontinuities.
        #[must_use]
        pub fn sine() -> Self {
            let wavetable_size = TABLE_SIZE;
            let mut wt = Self::new();
            // Fill buffer with a sine
            for i in 0..wavetable_size {
                wt.buffer[i] = F::new(((i as f64 / TABLE_SIZE as f64) * PI * 2.0).sin());
            }
            wt.update_diff_buffer();
            wt
        }
        /// Create a new wavetable containing a cosine wave.
        #[must_use]
        pub fn cosine() -> Self {
            let wavetable_size = TABLE_SIZE;
            let mut wt = Self::new();
            // Fill buffer with a sine
            for i in 0..wavetable_size {
                wt.buffer[i] = F::new(((i as f64 / TABLE_SIZE as f64) * PI * 2.0).cos());
            }
            wt.update_diff_buffer();
            wt
        }
        /// Create a new wavetable containing an aliasing sawtooth wave
        #[must_use]
        pub fn aliasing_saw() -> Self {
            let wavetable_size = TABLE_SIZE;
            let mut wt = Self::new();
            // Fill buffer with a sine
            let per_sample = 2.0 / wavetable_size as f64;
            for i in 0..wavetable_size {
                wt.buffer[i] = F::new(-1. + per_sample * i as f64);
            }
            wt.update_diff_buffer();
            wt
        }
        // #[must_use]
        // pub fn crazy(seed: u32) -> Self {
        //     let wavetable_size = TABLE_SIZE;
        //     let mut wt = Wavetable::new();
        //     let mut xorrng = XOrShift32Rng::new(seed);
        //     wt.fill_sine(16, 1.0);
        //     for _ in 0..(xorrng.gen_u32() % 3 + 1) {
        //         wt.fill_sine(16, (xorrng.gen_f32() * 32.0).floor());
        //     }
        //     wt.add_noise(1.0 - xorrng.gen_f64() * 0.05, seed + wavetable_size as u32);
        //     wt.normalize();
        //     wt.update_diff_buffer();
        //     wt
        // }
        #[must_use]
        /// Produces a Hann window
        pub fn hann_window() -> Self {
            let mut wt = Self::new();
            // This approach was heavily influenced by the SuperCollider Signal implementation
            wt.fill(F::new(0.5));
            wt.add_sine(1.0, 0.5, -0.5 * PI);
            wt.update_diff_buffer();
            wt
        }
        /// Produces a Hamming window
        #[must_use]
        pub fn hamming_window() -> Self {
            let mut wt = Self::new();
            // This approach was heavily influenced by the SuperCollider Signal implementation
            wt.fill(F::new(0.53836));
            wt.add_sine(1.0, 0.46164, -0.5 * PI);
            wt.update_diff_buffer();
            wt
        }
        /// Produces a Sine window
        #[must_use]
        pub fn sine_window() -> Self {
            let mut wt = Self::new();
            // This approach was heavily influenced by the SuperCollider Signal implementation
            wt.add_sine(0.5, 1.0, 0.0);
            wt.update_diff_buffer();
            wt
        }
        /// Fill the wavetable buffer with some value
        pub fn fill(&mut self, value: F) {
            for sample in &mut self.buffer {
                *sample = value;
            }
            self.update_diff_buffer();
        }
        /// Add a sine wave with the given parameters to the wavetable. Note that
        /// the frequency is relative to the wavetable. If adding a sine wave of
        /// frequency 2.0 Hz and then playing the wavetable at frequency 200 Hz that
        /// sine wave will sound at 400 Hz.
        pub fn add_sine(&mut self, freq: f64, amplitude: f64, mut phase: f64) {
            let step = (freq * PI * 2.0) / TABLE_SIZE as f64;
            for sample in &mut self.buffer {
                *sample += F::new(phase.sin() * amplitude);
                phase += step;
            }
            self.update_diff_buffer();
        }
        /// Add a number of harmonics to the wavetable, starting at frequency `freq`.
        pub fn fill_sine(&mut self, num_harmonics: usize, freq: f64) {
            for n in 0..num_harmonics {
                let start_phase = 0.0;
                let harmonic_amp = match n {
                    0 => 1.0,
                    _ => ((num_harmonics - n) as f64 / (num_harmonics) as f64) * 0.5,
                };
                let harmonic_freq = freq * (n + 1) as f64;
                for i in 0..TABLE_SIZE {
                    self.buffer[i] += F::new(
                        ((i as f64 / TABLE_SIZE as f64) * PI * 2.0 * harmonic_freq + start_phase)
                            .sin()
                            * harmonic_amp,
                    );
                }
            }
            self.update_diff_buffer();
        }
        /// Add a naive sawtooth wave to the wavetable.
        pub fn add_saw(&mut self, start_harmonic: usize, end_harmonic: usize, amp: f64) {
            for i in start_harmonic..=end_harmonic {
                let start_phase = 0.0;
                let harmonic_amp = 1.0 / ((i + 1) as f64 * PI as f64);
                let len = self.buffer.len() as f64;
                for k in 0..self.buffer.len() {
                    self.buffer[k] += F::new(
                        ((k as f64 / len * PI as f64 * 2.0 * (i + 1) as f64 + start_phase).sin()
                            * harmonic_amp)
                            * amp,
                    );
                }
            }
            self.update_diff_buffer();
        }
        /// Add a number of odd harmonics to the wavetable. `amp_falloff` is the
        /// exponential falloff as we go to higher harmonics, a value of 0.0 is no
        /// falloff.
        pub fn add_odd_harmonics(&mut self, num_harmonics: usize, amp_falloff: f64) {
            for i in 0..num_harmonics {
                let start_phase = match i {
                    0 => 0.0,
                    _ => (-1.0 as f64).powi(i as i32 + 2),
                };
                // an amp_falloff of 2.0 gives triangle wave approximation
                let harmonic_amp = 1.0 / ((i * 2 + 1) as f64).powf(amp_falloff);
                // Add this odd harmonic to the buffer
                let len = self.buffer.len() as f64;
                for k in 0..self.buffer.len() {
                    self.buffer[k] += F::new(
                        (k as f64 / len * PI * 2.0 * ((i * 2) as f64 + 1.0) + start_phase).sin()
                            * harmonic_amp,
                    );
                }
            }
            self.update_diff_buffer();
        }
        /// Add noise to the wavetable using [`XOrShift32Rng`], keeping the wavetable within +/- 1.0
        /// TODO: anti-aliasing
        pub fn add_noise(&mut self, probability: f64, seed: u32) {
            let mut xorrng = XOrShift32Rng::new(seed);
            for sample in &mut self.buffer {
                if xorrng.gen_f64() > probability {
                    *sample += F::new(xorrng.gen_f32() - 0.5);
                    if *sample > F::ONE {
                        *sample -= F::ONE;
                    }
                    if *sample < F::new(-1.0) {
                        *sample += F::ONE;
                    }
                }
            }
            self.update_diff_buffer();
        }
        /// Multiply all values of the wavetable by a given amount.
        pub fn multiply(&mut self, mult: F) {
            for sample in &mut self.buffer {
                *sample *= mult;
            }
            self.update_diff_buffer();
        }

        /// Linearly interpolate between the value in between which the phase points.
        /// The phase is assumed to be 0 <= phase < 1
        #[inline]
        #[must_use]
        pub fn get_linear_interp(&self, phase: WavetablePhase) -> F {
            let index = phase.integer_component();
            let mix = F::new(phase.fractional_component_f32());
            self.buffer[index] + self.diff_buffer[index] * mix
        }

        /// Get the closest sample with no interpolation
        #[inline]
        #[must_use]
        pub fn get(&self, phase: WavetablePhase) -> F {
            unsafe { *self.buffer.get_unchecked(phase.integer_component()) }
        }
    }

    const TABLE_AA_SPACING: f32 = 1.5;
    /// Converts a certain frequency to the corresponding wavetable
    fn freq_to_table_index(freq: f32) -> usize {
        // let mut index = 0;
        // let mut freq = freq;
        // loop {
        //     if freq < 32. {
        //         return index;
        //     }
        //     freq /= TABLE_AA_SPACING;
        //     index += 1;
        // }

        // For TABLE_AA_SPACING == 1.5
        let f = freq;
        if f <= 32.0 {
            0
        } else if f <= 48.0 {
            1
        } else if f <= 72.0 {
            2
        } else if f <= 108.0 {
            3
        } else if f <= 162.0 {
            4
        } else if f <= 243.0 {
            5
        } else if f <= 364.5 {
            6
        } else if f <= 546.75 {
            7
        } else if f <= 820.125 {
            8
        } else if f <= 1230.1875 {
            9
        } else if f <= 1845.2813 {
            10
        } else if f <= 2767.9219 {
            11
        } else if f <= 4151.883 {
            12
        } else if f <= 6227.824 {
            13
        } else if f <= 9341.736 {
            14
        } else if f <= 14012.6045 {
            15
        } else {
            16
        }
    }
    fn table_index_to_max_freq_produced(index: usize) -> f32 {
        32. * TABLE_AA_SPACING.powi(index as i32)
    }
    fn table_index_to_max_harmonic(index: usize) -> usize {
        // The higher this freq, the lower the number of harmonics
        let max_freq_produced = table_index_to_max_freq_produced(index);
        let max_harmonic_freq = 20000.0;
        (max_harmonic_freq / max_freq_produced) as usize
    }

    /// Wavetable is a standardised wavetable with a buffer of samples, as well as a
    /// separate buffer with the difference between the current sample and the next.
    /// The wavetable is of size [`TABLE_SIZE`] and can be indexed using a [`WavetablePhase`].
    ///
    /// It is not safe to modify the wavetable while it is being used on the audio
    /// thread, even if no Node is currently reading from it, because most modifying
    /// operations may allocate.
    #[derive(Debug, Clone)]
    pub struct Wavetable<F> {
        partial_tables: Vec<NonAaWavetable<F>>,
    }

    impl<F: Float> Default for Wavetable<F> {
        fn default() -> Self {
            let num_tables = freq_to_table_index(20000.0) + 1;
            Wavetable {
                partial_tables: vec![NonAaWavetable::default(); num_tables],
            }
        }
    }

    impl<F: Float> Wavetable<F> {
        /// Create an empyu wavetable
        #[must_use]
        pub fn new() -> Self {
            Self::default()
        }
        /// Recalculate the difference between samples in the buffer.
        ///
        /// The [`Wavetable`] contains a buffer with the difference between each
        /// sample of the buffer for efficiency reason.
        pub fn update_diff_buffer(&mut self) {
            for table in &mut self.partial_tables {
                table.update_diff_buffer()
            }
        }
        /// Create a [`Wavetable`] from an existing buffer. TODO: anti-aliasing
        ///
        /// # Errors
        /// The buffer has to be of [`TABLE_SIZE`] length, otherwise an error will be returned.
        pub fn from_buffer(buffer: Vec<F>) -> Result<Self, String> {
            if buffer.len() != TABLE_SIZE {
                return Err(format!(
                    "Invalid size buffer for a wavetable: {}. Wavetables must be of size {}",
                    buffer.len(),
                    TABLE_SIZE,
                ));
            }
            // TODO: FFT of the buffer for anti-aliasing
            let mut s = Self::default();
            for table in &mut s.partial_tables {
                table.set_from_buffer(buffer.clone());
            }
            Ok(s)
        }
        /// Create a new [`Wavetable`] and populate it using the closure/function provided. TODO: anti-aliasing
        #[must_use]
        pub fn from_closure<C>(c: C) -> Self
        where
            C: FnOnce(&mut [F]),
        {
            let mut w = Self::default();
            let mut buffer = vec![F::ZERO; TABLE_SIZE];
            c(&mut buffer);
            // TODO: FFT of buffer for anti-aliasing
            for table in &mut w.partial_tables {
                table.set_from_buffer(buffer.clone());
            }
            w
        }
        /// Create a new wavetable containing a sine wave. For audio, you often want a cosine instead since it starts at 0 to avoid discontinuities.
        #[must_use]
        pub fn sine() -> Self {
            let mut wt = Wavetable::new();
            for table in &mut wt.partial_tables {
                *table = NonAaWavetable::sine();
            }
            wt
        }
        /// Create a new wavetable containing a cosine wave.
        #[must_use]
        pub fn cosine() -> Self {
            let mut wt = Wavetable::new();
            for table in &mut wt.partial_tables {
                *table = NonAaWavetable::cosine();
            }
            wt
        }
        /// Create a new wavetable containing an aliasing sawtooth wave
        #[must_use]
        pub fn aliasing_saw() -> Self {
            let mut wt = Wavetable::new();
            for table in &mut wt.partial_tables {
                *table = NonAaWavetable::aliasing_saw();
            }
            wt
        }
        #[must_use]
        /// Produces a Hann window
        pub fn hann_window() -> Self {
            let mut wt = Wavetable::new();
            for table in &mut wt.partial_tables {
                *table = NonAaWavetable::hann_window();
            }
            wt
        }
        /// Produces a Hamming window
        #[must_use]
        pub fn hamming_window() -> Self {
            let mut wt = Wavetable::new();
            for table in &mut wt.partial_tables {
                *table = NonAaWavetable::hamming_window();
            }
            wt
        }
        /// Produces a Sine window
        #[must_use]
        pub fn sine_window() -> Self {
            let mut wt = Wavetable::new();
            for table in &mut wt.partial_tables {
                *table = NonAaWavetable::sine_window();
            }
            wt
        }
        /// Fill the wavetable buffer with some value
        pub fn fill(&mut self, value: F) {
            for table in &mut self.partial_tables {
                table.fill(value);
            }
        }
        /// Add a sine wave with the given parameters to the wavetable. Note that
        /// the frequency is relative to the wavetable. If adding a sine wave of
        /// frequency 2.0 Hz and then playing the wavetable at frequency 200 Hz that
        /// sine wave will sound at 400 Hz.
        pub fn add_sine(&mut self, freq: f64, amplitude: f64, phase: f64) {
            for (i, table) in self.partial_tables.iter_mut().enumerate() {
                if freq.ceil() as usize <= table_index_to_max_harmonic(i) {
                    table.add_sine(freq, amplitude, phase);
                }
            }
        }
        /// Add a number of harmonics to the wavetable, starting at frequency `freq`.
        pub fn fill_sine(&mut self, num_harmonics: usize, freq: f64) {
            for (i, table) in self.partial_tables.iter_mut().enumerate() {
                table.fill_sine(
                    num_harmonics.min((table_index_to_max_harmonic(i) as f64 * freq) as usize),
                    freq,
                );
            }
        }
        /// Add a naive sawtooth wave to the wavetable.
        pub fn add_aliasing_saw(&mut self, num_harmonics: usize, amp: f64) {
            for (i, table) in self.partial_tables.iter_mut().enumerate() {
                table.add_saw(0, num_harmonics.min(table_index_to_max_harmonic(i)), amp);
            }
        }
        /// Add a sawtooth wave starting from a specific harmonic
        pub fn add_saw(&mut self, start_harmonic: usize, end_harmonic: usize, amp: f64) {
            for (i, table) in self.partial_tables.iter_mut().enumerate() {
                let end_harmonic = end_harmonic.min(table_index_to_max_harmonic(i));
                if end_harmonic > start_harmonic {
                    table.add_saw(start_harmonic, end_harmonic, amp);
                }
            }
        }
        /// Add a number of odd harmonics to the wavetable. `amp_falloff` is the
        /// exponential falloff as we go to higher harmonics, a value of 0.0 is no
        /// falloff.
        pub fn add_odd_harmonics(&mut self, num_harmonics: usize, amp_falloff: f64) {
            for (i, table) in self.partial_tables.iter_mut().enumerate() {
                table.add_odd_harmonics(
                    num_harmonics.min(table_index_to_max_harmonic(i)),
                    amp_falloff,
                );
            }
        }
        /// Add noise to the wavetable using [`XOrShift32Rng`], keeping the wavetable within +/- 1.0
        ///
        /// TODO: Anti-alias by FFT
        pub fn add_noise(&mut self, probability: f64, seed: u32) {
            for table in self.partial_tables.iter_mut() {
                table.add_noise(probability, seed);
            }
        }
        /// Multiply all values of the wavetable by a given amount.
        pub fn multiply(&mut self, mult: F) {
            for table in self.partial_tables.iter_mut() {
                table.multiply(mult);
            }
        }
        /// Normalize the amplitude of the wavetable to 1.0 based on the wavetable most rich in harmonics. Interference from high partials out of phase could normalize high pitch tables to more or less than 1.0.
        pub fn normalize(&mut self) {
            // Find highest absolute value
            let mut loudest_sample = F::ZERO;
            for sample in &self.partial_tables[0].buffer {
                if sample.abs() > loudest_sample {
                    loudest_sample = sample.abs();
                }
            }
            // Scale all tables by the same amount
            let scaler = F::ONE / loudest_sample;
            for table in self.partial_tables.iter_mut() {
                table.multiply(scaler);
            }
        }

        /// Linearly interpolate between the value in between which the phase points.
        /// The phase is assumed to be 0 <= phase < 1
        #[inline]
        #[must_use]
        pub fn get_linear_interp(&self, phase: WavetablePhase, freq: F) -> F {
            let table_index = freq_to_table_index(freq.to_f32());
            self.partial_tables[table_index.min(self.partial_tables.len())].get_linear_interp(phase)
        }

        /// Get the closest sample with no interpolation
        #[inline]
        #[must_use]
        pub fn get(&self, phase: WavetablePhase, freq: F) -> F {
            let table_index = freq_to_table_index(freq.to_f32());
            self.partial_tables[table_index.min(self.partial_tables.len())].get(phase)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::table_index_to_max_freq_produced;

        use super::freq_to_table_index;

        #[test]
        fn table_nr_from_freq() {
            freq_to_table_index(0.0);
            freq_to_table_index(20.0);
            freq_to_table_index(20000.0);
            // dbg!(freq_to_table_index(0.0));
            // dbg!(freq_to_table_index(20.0));
            // dbg!(freq_to_table_index(20000.0));
            // let max_index = freq_to_table_index(20050.) + 1;
            // println!("Max freq produced:");
            // for i in 0..max_index {
            //     dbg!(table_index_to_max_freq_produced(i));
            // }
            // println!("Num harmonics for table:");
            // for i in 0..max_index {
            //     println!(
            //         "{i}: max_harmonics: {} max_freq: {}",
            //         table_index_to_max_harmonic(i),
            //         table_index_to_max_freq_produced(i)
            //     );
            // }
            assert!(table_index_to_max_freq_produced(freq_to_table_index(20000.)) >= 20000.);
            assert!(table_index_to_max_freq_produced(freq_to_table_index(20.)) >= 20.);
            assert!(table_index_to_max_freq_produced(freq_to_table_index(200.)) >= 200.);
        }
    }
}
