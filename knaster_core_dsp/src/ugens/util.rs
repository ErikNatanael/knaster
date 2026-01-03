//! # Util
//!
//! Utility UGens
use crate::core::marker::PhantomData;
use knaster_core::{AudioCtx, Float, PFloat, UGenFlags, impl_ugen, rt_log};

/// Sets the done flag when it receives a trigger. Use in combination with `Graph::push_with_done_action` or [`WrDone`] and a [`Done`] which frees more than the current node.
pub struct DoneOnTrig<F> {
    triggered: bool,
    _phantom: PhantomData<F>,
}
#[impl_ugen]
impl<F: Float> DoneOnTrig<F> {
    #[allow(clippy::new_without_default)]
    #[allow(missing_docs)]
    pub fn new() -> Self {
        Self {
            triggered: false,
            _phantom: PhantomData,
        }
    }
    /// Trigger to set the done flag on the next sample
    #[param]
    pub fn t_done(&mut self) {
        self.triggered = true;
    }
    #[allow(missing_docs)]
    pub fn process(&mut self, flags: &mut UGenFlags, _input: [F; 0]) -> [F; 0] {
        if self.triggered {
            flags.mark_done(0);
        }
        []
    }
}

/// UGen producing a constant value
pub struct Constant<F: Float> {
    value: F,
}
#[impl_ugen]
impl<F: Float> Constant<F> {
    #[allow(missing_docs)]
    pub fn new(value: F) -> Self {
        Self { value }
    }
    /// Set the value of the constant
    #[param]
    pub fn value(&mut self, value: PFloat) {
        self.value = F::new(value);
    }
    #[allow(missing_docs)]
    pub fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        _input: [F; 0],
    ) -> [F; 1] {
        [self.value]
    }
    #[allow(missing_docs)]
    pub fn process_block(&mut self, output: [&mut [F]; 1]) {
        output[0].fill(self.value);
    }
}

#[allow(unused)]
use knaster_core::log::ArLogReceiver;
/// Log the input to this UGen to the audio rate log every N samples. See [`ArLogReceiver`] for how
/// to receive the log messages.
pub struct LogProbe<F: Float> {
    samples_between_logs: usize,
    sample_counter: usize,
    name: &'static str,
    _phantom: PhantomData<F>,
}
#[impl_ugen]
impl<F: Float> LogProbe<F> {
    #[allow(missing_docs)]
    pub fn new(name: &'static str) -> Self {
        Self {
            samples_between_logs: 44100,
            sample_counter: 0,
            name,
            _phantom: PhantomData,
        }
    }
    #[allow(missing_docs)]
    pub fn init(&mut self, sample_rate: u32, _block_size: usize) {
        self.samples_between_logs = sample_rate as usize;
    }

    fn process(&mut self, _ctx: &mut AudioCtx, _flags: &mut UGenFlags, input: [F; 1]) -> [F; 0] {
        if self.sample_counter == 0 {
            rt_log!(_ctx.logger(); "Probe", self.name, input[0].to_f64());
            self.sample_counter = self.samples_between_logs;
        } else {
            self.sample_counter -= 1;
        }
        []
    }
}
