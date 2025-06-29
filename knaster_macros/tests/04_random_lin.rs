use knaster_core::{AudioCtx, Float, PFloat, UGen, UGenFlags, typenum::*};
/// Random numbers 0..1 with linear interpolation with new values at some frequency. Freq is sampled at control rate only.
pub struct RandomLin<F: Copy = f32> {
    current_value: F,
    current_change_width: F,
    // when phase reaches 1 we choose a new value
    phase: F,
    phase_step: F,
    freq_to_phase_inc: F,
}

#[knaster_macros::impl_ugen]
impl<F: Float> RandomLin<F> {
    /// Create a new RandomLin, seeding it from the global atomic seed.
    pub fn new(freq: F) -> Self {
        Self {
            current_value: F::new(0.),
            phase: F::ZERO,
            freq_to_phase_inc: F::ZERO,
            current_change_width: F::ZERO,
            phase_step: freq,
        }
    }

    #[inline]
    fn new_value(&mut self) {
        let old_target = self.current_value + self.current_change_width;
        let new = F::new(1.0);
        self.current_value = old_target;
        self.current_change_width = new - old_target;
        self.phase = F::new(0.0);
    }
    fn init(&mut self, sample_rate: u32, _block_size: usize) {
        self.freq_to_phase_inc = F::ONE / F::from(sample_rate).unwrap();
        // freq is stored in phase_step until init
        self.phase_step *= self.freq_to_phase_inc;
        self.new_value();
    }
    fn process(&mut self, _ctx: &mut AudioCtx, _flags: &mut UGenFlags) -> [F; 1] {
        let out = self.current_value + self.phase * self.current_change_width;
        self.phase += self.phase_step;

        if self.phase >= F::ONE {
            self.new_value();
        }
        [out]
    }
    #[param]
    pub fn freq(&mut self, value: PFloat) {
        if self.freq_to_phase_inc == F::ZERO {
            // freq is stored in phase_step until init
            self.phase_step = F::new(value);
        } else {
            self.phase_step = F::new(value) * self.freq_to_phase_inc;
        }
    }
}

fn main() {
    assert_eq!(RandomLin::<f32>::param_descriptions()[0], "freq");
}
