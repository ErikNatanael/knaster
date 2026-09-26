use knaster_core::{
    AudioCtx, Block, Float, StaticBlock, UGen, UGenFlags, log::ArLogSender, typenum::*,
};

/// Outputs a static number every frame
pub(crate) struct TestInPlusParamGen<F> {
    number: F,
}

#[knaster_macros::impl_ugen]
impl<F: Float> TestInPlusParamGen<F> {
    pub fn new() -> Self {
        Self { number: F::ZERO }
    }
    #[control(default = 0.0)]
    pub fn number(&mut self, n: F) {
        self.number = n;
    }
    #[param]
    pub fn t_reset(&mut self) {
        self.number = F::new(0.);
    }
    fn process(&mut self, _ctx: &mut AudioCtx, _flags: &mut UGenFlags) -> [F; 1] {
        [self.number]
    }
    fn process_block(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        outputs: [&mut [F]; 1],
    ) {
        for out in outputs[0].iter_mut() {
            *out = self.number;
        }
    }
}

fn main() {
    let mut ctx = AudioCtx::new(44100, 64, ArLogSender::non_rt());
    let mut flags = UGenFlags::new();
    let mut ugen = TestInPlusParamGen::<f64>::new();
    ugen.number(2.);
    ugen.control(&mut ctx, 0, 3.);
    assert_eq!(
        UGen::process(&mut ugen, &mut ctx, &mut flags, [].into())[0],
        3.
    );
    let mut input = StaticBlock::<f64, U1, U64>::new();
    input.channel_as_slice_mut(0).fill(17.0);
    let mut output = StaticBlock::<f64, U1, U64>::new();
    UGen::process_block(&mut ugen, &mut ctx, &mut flags, &input, &mut output);
}
