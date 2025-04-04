use crate::numeric_array::NumericArray;
use crate::{AudioCtx,  ParameterHint, ParameterValue, UGen, UGenFlags};
use knaster_primitives::typenum::{U0, U1, U3, U4};
use knaster_primitives::{Block, BlockRead, Float, Frame};

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
impl<F: Float> EnvAsr<F> {
    pub const ATTACK_TIME: usize = 0;
    pub const RELEASE_TIME: usize = 1;
    pub const T_RELEASE: usize = 2;
    pub const T_RESTART: usize = 3;
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
}

impl<F: Float> UGen for EnvAsr<F> {
    type Sample = F;
    type Inputs = U0;
    type Outputs = U1;
    type Parameters = U4;

    fn init(&mut self, sample_rate: u32, _block_size: usize) {
        // Init rate based on the seconds if the rate hasn't already been set through a param
        if self.attack_rate == F::ONE {
            if self.attack_seconds == F::ZERO {
                self.attack_rate = F::ONE;
            } else {
                self.attack_rate =
                    F::ONE / (self.attack_seconds * F::from(sample_rate).unwrap());
            }
        }
        if self.release_rate == F::ONE {
            if self.release_seconds == F::ZERO {
                self.release_rate = F::ONE;
            } else {
                self.release_rate =
                    F::ONE / (self.release_seconds * F::from(sample_rate).unwrap());
            }
        }
    }

    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        _input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        let out = self.next_sample(flags, 0);
        [out].into()
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        _ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        _input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: BlockRead<Sample = Self::Sample>,
        OutBlock: Block<Sample = Self::Sample>,
    {
        for (i, out) in output.channel_as_slice_mut(0).iter_mut().enumerate() {
            *out = self.next_sample(flags, i as u32);
        }
    }

    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        ["attack_time", "release_time", "t_release", "t_restart"].into()
    }
    fn param_hints() -> NumericArray<ParameterHint, Self::Parameters> {
        [
            ParameterHint::positive_infinite_float(),
            ParameterHint::positive_infinite_float(),
            ParameterHint::Trigger,
            ParameterHint::Trigger,
        ]
        .into()
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
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
#[derive(Debug, Clone, Copy)]
enum ArState {
    Stopped,
    Attacking,
    Releasing,
}

/// Simple ASR envelope with a linear attack and a cubic release
#[derive(Debug, Clone)]
pub struct EnvAr<F: Copy> {
    state: ArState,
    t: F,
    attack_seconds: F,
    attack_rate: F,
    release_seconds: F,
    release_rate: F,
    /// On early release, we need to scale the release value by the value we were on because the curves are different
    release_scale: F,
}
impl<F: Float> EnvAr<F> {
    pub const ATTACK_TIME: usize = 0;
    pub const RELEASE_TIME: usize = 1;
    pub const T_RESTART: usize = 2;
    pub fn new(attack_time: F, release_time: F) -> Self {
        Self {
            state: ArState::Stopped,
            t: F::ZERO,
            attack_seconds: attack_time,
            attack_rate: F::ONE,
            release_rate: F::ONE,
            release_seconds: release_time,
            release_scale: F::ONE,
        }
    }
    pub fn trig_start(&mut self) {
        self.state = ArState::Attacking;
    }
    #[inline(always)]
    pub fn next_sample(&mut self, flags: &mut UGenFlags, sample_in_block: u32) -> F {
        let out: F;
        match self.state {
            ArState::Stopped => {
                out = F::ZERO;
            }
            ArState::Attacking => {
                // *out = self.t.powi(2);
                out = self.t;
                self.t += self.attack_rate;

                if self.t >= F::ONE {
                    self.release_scale = F::ONE;
                    self.state = ArState::Releasing;
                    self.t = F::ONE;
                }
            }
            ArState::Releasing => {
                out = self.t.powi(3) * self.release_scale;
                self.t -= self.release_rate;
                if self.t <= F::ZERO {
                    self.state = ArState::Stopped;
                    self.t = F::ZERO;
                    flags.mark_done(sample_in_block);
                }
            }
        }
        out
    }
}
impl<F: Float> UGen for EnvAr<F> {
    type Sample = F;
    type Inputs = U0;
    type Outputs = U1;
    type Parameters = U3;

    fn init(&mut self, sample_rate: u32, _block_size: usize) {
        // Init rate based on the seconds if the rate hasn't already been set through a param
        if self.attack_rate == F::ONE {
            if self.attack_seconds == F::ZERO {
                self.attack_rate = F::ONE;
            } else {
                self.attack_rate =
                    F::ONE / (self.attack_seconds * F::from(sample_rate).unwrap());
            }
        }
        if self.release_rate == F::ONE {
            if self.release_seconds == F::ZERO {
                self.release_rate = F::ONE;
            } else {
                self.release_rate =
                    F::ONE / (self.release_seconds * F::from(sample_rate).unwrap());
            }
        }
    }

    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        _input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        let out = self.next_sample(flags, 0);
        [out].into()
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        _ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        _input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: BlockRead<Sample = Self::Sample>,
        OutBlock: Block<Sample = Self::Sample>,
    {
        for (i, out) in output.channel_as_slice_mut(0).iter_mut().enumerate() {
            *out = self.next_sample(flags, i as u32);
        }
    }

    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        ["attack_time", "release_time", "t_restart"].into()
    }
    fn param_hints() -> NumericArray<ParameterHint, Self::Parameters> {
        [
            ParameterHint::float(|h| h.logarithmic(true).minmax(0.0, 20.0)),
            ParameterHint::float(|h| h.logarithmic(true).minmax(0.0, 20.0)),
            ParameterHint::Trigger,
        ]
        .into()
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
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
            Self::T_RESTART => {
                // match self.state {
                //     ArState::Attacking | ArState::Releasing => {
                //         self.release_scale = F::new(self.t);
                //         self.state = ArState::Releasing;
                //         self.t = F::ONE;
                //     }
                // }
                self.trig_start();
            }
            _ => (),
        }
    }
}
