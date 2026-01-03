//! # Envelopes
//!
//! Various envelop [`UGen`]s
use knaster_core::impl_ugen;
#[allow(unused)]
use knaster_core::{AudioCtx, UGen, UGenFlags};
use knaster_core::{Block, BlockRead, Float, PFloat};

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
    /// Create a new ASR envelope with the given attack and release time.
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
    /// Start the envelope from the current value
    // TODO: Take the current value of self.t from Self::next_sample because the release value is
    // not linear.
    fn trig_start(&mut self) {
        self.state = AsrState::Attacking;
    }
    /// Progress the envelope and return the next sample value
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

    /// Set the attack time in seconds
    #[param(kind = Seconds)]
    pub fn attack_time(&mut self, ctx: &mut AudioCtx, seconds: PFloat) {
        let atk = F::new(seconds);
        if self.attack_seconds != atk {
            self.attack_seconds = atk;
            if atk == F::ZERO {
                self.attack_rate = F::ONE;
            } else {
                self.attack_rate =
                    F::ONE / (self.attack_seconds * F::from(ctx.sample_rate()).unwrap());
            }
        }
    }
    /// Set the release time in seconds
    #[param(kind = Seconds)]
    pub fn release_time(&mut self, ctx: &mut AudioCtx, seconds: PFloat) {
        let rel = F::new(seconds);
        if self.release_seconds != rel {
            self.release_seconds = rel;
            if rel == F::ZERO {
                self.release_rate = F::ONE;
            } else {
                self.release_rate =
                    F::ONE / (self.release_seconds * F::from(ctx.sample_rate()).unwrap());
            }
        }
    }
    /// Trigger the release of the envelope
    #[param]
    pub fn t_release(&mut self) {
        match self.state {
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
        }
    }
    /// Trigger the (re)start of the envelope
    #[param]
    pub fn t_restart(&mut self) {
        self.trig_start();
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
#[impl_ugen]
impl<F: Float> EnvAr<F> {
    /// Create a new AR envelope
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
    /// (Re)start the envelope from the current value
    // TODO: Take the relase curve into account, e.g. by taking the current value of `next_sample`
    fn trig_start(&mut self) {
        self.state = ArState::Attacking;
    }
    /// Progress the envelope one frame and return the value
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
    /// Set the attack time in seconds
    #[param(kind = Seconds)]
    pub fn attack_time(&mut self, ctx: &mut AudioCtx, seconds: PFloat) {
        let atk = F::new(seconds);
        if self.attack_seconds != atk {
            self.attack_seconds = atk;
            if atk == F::ZERO {
                self.attack_rate = F::ONE;
            } else {
                self.attack_rate =
                    F::ONE / (self.attack_seconds * F::from(ctx.sample_rate()).unwrap());
            }
        }
    }
    /// Set the release time in seconds
    #[param(kind = Seconds)]
    pub fn release_time(&mut self, ctx: &mut AudioCtx, seconds: PFloat) {
        let rel = F::new(seconds);
        if self.release_seconds != rel {
            self.release_seconds = rel;
            if rel == F::ZERO {
                self.release_rate = F::ONE;
            } else {
                self.release_rate =
                    F::ONE / (self.release_seconds * F::from(ctx.sample_rate()).unwrap());
            }
        }
    }
    /// Trigger the (re)start of the envelope
    #[param]
    pub fn t_restart(&mut self) {
        self.trig_start();
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
}

#[cfg(any(feature = "std", feature = "alloc"))]
pub use alloc_envelopes::*;

#[cfg(any(feature = "std", feature = "alloc"))]
mod alloc_envelopes {

    use core::marker::PhantomData;

    use knaster_core::{AudioCtx, PInteger, ParameterHint, ParameterValue, UGen, UGenFlags};
    use knaster_core::{
        Float, Frame,
        numeric_array::NumericArray,
        typenum::{U0, U1, U4},
    };

    /// An envelope segment for an [`Envelope`]
    #[derive(Copy, Clone, Debug)]
    pub struct EnvelopeSegment {
        reciprocal_duration: f64,
        duration: f64,
        value: f64,
    }
    impl EnvelopeSegment {
        /// Duration is in seconds the time it takes to reach the value
        pub fn new(duration: f64, value: f64) -> Self {
            Self {
                reciprocal_duration: 1.0 / duration,
                duration,
                value,
            }
        }
    }
    /// The shape of an envelope segment
    #[derive(Copy, Clone, Debug)]
    pub enum EnvelopeShape {
        #[allow(missing_docs)]
        Linear,
        #[allow(missing_docs)]
        Exponential,
        #[allow(missing_docs)]
        Sinusoidal,
        #[allow(missing_docs)]
        Step,
    }
    #[derive(Copy, Clone, Debug)]
    enum EnvelopeState {
        Stopped,
        Running {
            current_segment: usize,
            current_time: f64,
        },
    }

    /// A flexible envelope UGen with a variable number of segments.
    pub struct Envelope<F: Float> {
        state: EnvelopeState,
        segments: crate::core::vec::Vec<EnvelopeSegment>,
        start_value: f64,
        from_value: f64,
        current_segment: usize,
        time_scale: f64,
        base_scale: f64,
        looping: bool,
        _float: PhantomData<F>,
    }
    impl<F: Float> Envelope<F> {
        /// Create a new envelope with the given segments
        pub fn new(start_value: f64, segments: crate::core::vec::Vec<EnvelopeSegment>) -> Self {
            Self {
                state: EnvelopeState::Stopped,
                segments,
                start_value,
                from_value: start_value,
                current_segment: 0,
                time_scale: 1.0,
                base_scale: 0.0,
                looping: false,
                _float: PhantomData,
            }
        }
        /// Set the scaling for the time of progressing through the segments
        pub fn time_scale(mut self, time_scale: f64) -> Self {
            self.time_scale = time_scale;
            self
        }
        /// Set the `looping` state of the envelope, i.e. whether it should go back to the start
        /// once it has reached the end.
        pub fn looping(mut self, looping: bool) -> Self {
            self.looping = looping;
            self
        }
    }
    impl<F: Float> UGen for Envelope<F> {
        type Sample = F;
        type Inputs = U0;
        type Outputs = U1;
        type Parameters = U4;

        fn init(&mut self, sample_rate: u32, _block_size: usize) {
            self.base_scale = 1.0 / sample_rate as f64;
        }

        fn process(
            &mut self,
            _ctx: &mut AudioCtx,
            flags: &mut UGenFlags,
            _input: Frame<Self::Sample, Self::Inputs>,
        ) -> Frame<Self::Sample, Self::Outputs> {
            let out: F;
            match self.state {
                EnvelopeState::Stopped => {
                    out = F::new(self.from_value);
                }
                EnvelopeState::Running {
                    current_segment,
                    current_time,
                } => {
                    let t = current_time;
                    if t < self.segments[current_segment].duration {
                        let segment = &self.segments[current_segment];
                        out = F::new(
                            self.from_value
                                + (t * segment.reciprocal_duration)
                                    * (segment.value - self.from_value),
                        );
                        self.state = EnvelopeState::Running {
                            current_segment,
                            current_time: t + (self.time_scale * self.base_scale),
                        };
                    } else if current_segment + 1 < self.segments.len() {
                        self.from_value = self.segments[current_segment].value;
                        let segment = &self.segments[current_segment];
                        out = F::new(
                            self.from_value
                                + (t * segment.reciprocal_duration)
                                    * (segment.value - self.from_value),
                        );
                        self.state = EnvelopeState::Running {
                            current_segment: current_segment + 1,
                            current_time: current_time - segment.duration
                                + (self.time_scale * self.base_scale),
                        };
                    } else {
                        self.from_value = self.segments[current_segment].value;
                        out = F::new(self.from_value);
                        if self.looping {
                            self.state = EnvelopeState::Running {
                                current_segment: 0,
                                current_time: 0.,
                            };
                        } else {
                            self.state = EnvelopeState::Stopped;
                            flags.mark_done(0);
                        }
                    }
                }
            }
            [out].into()
        }
        fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
            ["time_scale", "jump_to_segment", "t_restart", "t_stop"].into()
        }
        fn param_hints() -> NumericArray<ParameterHint, Self::Parameters> {
            [
                ParameterHint::new_float(|h| h.logarithmic(true).minmax(0.0, 20.0)),
                ParameterHint::new_integer((PInteger::ZERO, PInteger::MAX), |h| h),
                ParameterHint::Trigger,
                ParameterHint::Trigger,
            ]
            .into()
        }
        fn param_apply(&mut self, _ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
            match index {
                0 => {
                    self.time_scale = F::new(value.float().unwrap()).to_f64();
                }
                1 => {
                    let mut jump_to_segment = value.integer().unwrap().0;
                    if jump_to_segment >= self.segments.len() {
                        jump_to_segment = self.segments.len() - 1;
                    }
                    match &mut self.state {
                        EnvelopeState::Stopped => {
                            self.state = EnvelopeState::Running {
                                current_segment: jump_to_segment,
                                current_time: 0.0,
                            };
                        }
                        EnvelopeState::Running {
                            current_segment,
                            current_time,
                        } => {
                            *current_segment = jump_to_segment;
                            *current_time = 0.0;
                        }
                    }

                    self.current_segment = jump_to_segment;
                }
                2 => {
                    self.state = EnvelopeState::Running {
                        current_segment: 0,
                        current_time: 0.0,
                    };
                    self.from_value = self.start_value;
                }
                3 => {
                    if let EnvelopeState::Running {
                        current_segment,
                        current_time,
                    } = self.state
                    {
                        let t = current_time;
                        let segment = &self.segments[current_segment];
                        self.from_value = self.from_value
                            + (t * segment.reciprocal_duration) * (segment.value - self.from_value);
                    }
                    self.state = EnvelopeState::Stopped;
                }
                _ => (),
            }
        }
    }
}
