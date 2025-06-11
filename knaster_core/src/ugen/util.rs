use crate::core::marker::PhantomData;
use crate::rt_log;

use knaster_macros::impl_ugen;
use knaster_primitives::Float;
use knaster_primitives::PFloat;

use crate::{AudioCtx, UGenFlags};

/// Sets the done flag when it receives a trigger. Use in combination with `Graph::push_with_done_action` or [`WrDone`] and a [`Done`] which frees more than the current node.
pub struct DoneOnTrig<F> {
    triggered: bool,
    _phantom: PhantomData<F>,
}
#[knaster_macros::impl_ugen]
impl<F: Float> DoneOnTrig<F> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            triggered: false,
            _phantom: PhantomData,
        }
    }
    #[param]
    pub fn t_done(&mut self) {
        self.triggered = true;
    }
    pub fn process(&mut self, flags: &mut UGenFlags, _input: [F; 0]) -> [F; 0] {
        if self.triggered {
            flags.mark_done(0);
        }
        []
    }
}

pub struct Constant<F: Float> {
    value: F,
}
#[impl_ugen]
impl<F: Float> Constant<F> {
    pub fn new(value: F) -> Self {
        Self { value }
    }
    #[param]
    pub fn value(&mut self, value: PFloat) {
        self.value = F::new(value);
    }
    pub fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        _input: [F; 0],
    ) -> [F; 1] {
        [self.value]
    }
    pub fn process_block(&mut self, output: [&mut [F]; 1]) {
        output[0].fill(self.value);
    }
}

pub struct LogProbe<F: Float> {
    samples_between_logs: usize,
    sample_counter: usize,
    name: &'static str,
    _phantom: PhantomData<F>,
}
#[knaster_macros::impl_ugen]
impl<F: Float> LogProbe<F> {
    pub fn new(name: &'static str) -> Self {
        Self {
            samples_between_logs: 44100,
            sample_counter: 0,
            name,
            _phantom: PhantomData,
        }
    }
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
