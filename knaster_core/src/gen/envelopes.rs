use crate::numeric_array::NumericArray;
use crate::typenum::U0;
use crate::{AudioCtx, BlockAudioCtx, Gen, GenFlags, PFloat, ParameterRange, ParameterValue};
use knaster_primitives::typenum::{U1, U4};
use knaster_primitives::{Block, BlockRead, Float, Frame};

#[derive(Debug, Clone, Copy)]
enum AsrState {
    Stopped,
    Attacking,
    Sustaining,
    Releasing,
}

/// Simple ASR envelope with a linear attack and a cubic release
pub struct Asr<F: Copy> {
    state: AsrState,
    t: F,
    attack_seconds: F,
    attack_rate: F,
    release_seconds: F,
    release_rate: F,
    /// On early release, we need to scale the release value by the value we were on because the curves are different
    release_scale: F,
}
impl<F: Float> Asr<F> {
    const ATTACK_TIME: usize = 0;
    const RELEASE_TIME: usize = 1;
    const T_RELEASE: usize = 2;
    const T_RESTART: usize = 3;
    pub fn new() -> Self {
        Self {
            state: AsrState::Stopped,
            t: F::ZERO,
            attack_seconds: F::ZERO,
            attack_rate: F::ONE,
            release_rate: F::ONE,
            release_seconds: F::ZERO,
            release_scale: F::ONE,
        }
    }
    pub fn trig_start(&mut self) {
        self.state = AsrState::Attacking;
    }
    #[inline(always)]
    pub fn next_sample(&mut self, flags: &mut GenFlags, sample_in_block: u32) -> F {
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
}
impl<F: Float> Gen for Asr<F> {
    type Sample = F;
    type Inputs = U0;
    type Outputs = U1;
    type Parameters = U4;

    fn process(
        &mut self,
        ctx: AudioCtx,
        flags: &mut GenFlags,
        input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        let out = self.next_sample(flags, 0);
        [out].into()
    }
    fn process_block<InBlock, OutBlock>(&mut self, ctx: BlockAudioCtx, flags: &mut GenFlags, input: &InBlock, output: &mut OutBlock)
    where
        InBlock: BlockRead<Sample=Self::Sample>,
        OutBlock: Block<Sample=Self::Sample>,
    {
        for (i, out) in output.channel_as_slice_mut(0).iter_mut().enumerate() {
            *out = self.next_sample(flags, i as u32);
        }
    }

    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        ["attack_time", "release_time", "t_release", "t_restart"].into()
    }
    fn param_range() -> NumericArray<ParameterRange, Self::Parameters> {
        [
            ParameterRange::Float(0.0, PFloat::INFINITY),
            ParameterRange::Float(0.0, PFloat::INFINITY),
            ParameterRange::Trigger,
            ParameterRange::Trigger,
        ]
        .into()
    }

    fn param_apply(&mut self, ctx: AudioCtx, index: usize, value: ParameterValue) {
        match index {
            Self::ATTACK_TIME => {
                let atk = F::new(value.float().unwrap());
                if self.attack_seconds != atk {
                    self.attack_seconds = atk;
                    if atk == F::ZERO {
                        self.attack_rate = F::ONE;
                    } else {
                        self.attack_rate =
                            F::ONE / (self.attack_seconds * F::from(ctx.sample_rate).unwrap());
                    }
                }
            }
            Self::RELEASE_TIME => {
                let rel = F::new(value.float().unwrap());
                if self.release_seconds != rel {
                    self.release_seconds = rel;
                    if rel == F::ZERO {
                        self.release_rate = F::ONE;
                    } else {
                        self.release_rate =
                            F::ONE / (self.release_seconds * F::from(ctx.sample_rate).unwrap());
                    }
                }
            }
            Self::T_RELEASE => match self.state {
                AsrState::Stopped => {}
                AsrState::Attacking => {
                    self.release_scale = F::new(self.t);
                    self.state = AsrState::Releasing;
                    self.t = F::ONE;
                }
                AsrState::Sustaining => {
                    self.release_scale = F::ONE;
                    self.state = AsrState::Releasing;
                    self.t = F::ONE;
                }
                AsrState::Releasing => {}
            },
            Self::T_RESTART => {
                self.trig_start();
            }
            _ => (),
        }
    }
}
