use knaster_core::{AudioCtx, Float, PFloat, UGenFlags, impl_ugen};

#[derive(Debug, Clone, Copy)]
enum AsrState {
    Stopped,
    Attacking,
    Sustaining,
    Releasing,
}

/// Simple ASR envelope with a linear attack and a cubic release
#[derive(Debug, Clone)]
pub struct EnvAsr<F: Copy> {
    state: AsrState,
    t: F,
    attack_seconds: F,
    attack_rate: F,
    release_seconds: F,
    release_rate: F,
    /// On early release, we need to scale the release value by the value we were on because the curves are different
    release_scale: F,
}

#[impl_ugen]
impl<F: Float> EnvAsr<F> {
    pub fn new(attack_time: F, release_time: F) -> Self {
        Self {
            state: AsrState::Stopped,
            t: F::ZERO,
            attack_seconds: attack_time,
            attack_rate: F::ONE,
            release_rate: F::ONE,
            release_seconds: release_time,
            release_scale: F::ONE,
        }
    }
    pub fn trig_start(&mut self) {
        self.state = AsrState::Attacking;
    }
    #[inline(always)]
    pub fn next_sample(&mut self, flags: &mut UGenFlags, sample_in_block: u32) -> F {
        let out: F;
        match self.state {
            AsrState::Stopped => {
                out = F::ZERO;
            }
            AsrState::Attacking => {
                // *out = self.t.powi(2);
                out = self.t;
                self.t += self.attack_rate;

                if self.t >= F::ONE {
                    self.state = AsrState::Sustaining;
                }
            }
            AsrState::Sustaining => {
                out = F::ONE;
            }
            AsrState::Releasing => {
                out = self.t.powi(3) * self.release_scale;
                self.t -= self.release_rate;
                if self.t <= F::ZERO {
                    self.state = AsrState::Stopped;
                    self.t = F::ZERO;
                    flags.mark_done(sample_in_block);
                }
            }
        }
        out
    }

    fn init(&mut self, sample_rate: u32, _block_size: usize) {
        // Init rate based on the seconds if the rate hasn't already been set through a param
        if self.attack_rate == F::ONE {
            if self.attack_seconds == F::ZERO {
                self.attack_rate = F::ONE;
            } else {
                self.attack_rate = F::ONE / (self.attack_seconds * F::from(sample_rate).unwrap());
            }
        }
        if self.release_rate == F::ONE {
            if self.release_seconds == F::ZERO {
                self.release_rate = F::ONE;
            } else {
                self.release_rate = F::ONE / (self.release_seconds * F::from(sample_rate).unwrap());
            }
        }
    }

    fn process(&mut self, _ctx: &mut AudioCtx, flags: &mut UGenFlags) -> [F; 1] {
        let out = self.next_sample(flags, 0);
        [out]
    }

    fn process_block(&mut self, _ctx: &mut AudioCtx, flags: &mut UGenFlags, output: [&mut [F]; 1]) {
        for (i, out) in output[0].iter_mut().enumerate() {
            *out = self.next_sample(flags, i as u32);
        }
    }
}
fn main() {}
