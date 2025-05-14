use knaster_core::{AudioCtx, Float, UGen, UGenFlags, log::ArLogSender, typenum::*};

/// Outputs a static number every frame
pub(crate) struct TestInPlusParamGen<F> {
    number: F,
}

#[knaster_macros::impl_ugen]
impl<F: Float> TestInPlusParamGen<F> {
    pub fn new() -> Self {
        Self { number: F::ZERO }
    }
    // #[param(default = 0.0)]
    // pub fn number(&mut self, n: PFloat) {
    //     self.number = F::new(n);
    // }
    fn process(&mut self, _ctx: &mut AudioCtx, _flags: &mut UGenFlags, input: [F; 1]) -> [F; 1] {
        [self.number + input[0]]
    }
}

fn main() {
    let mut ctx = AudioCtx::new(44100, 64, ArLogSender::non_rt());
    let mut flags = UGenFlags::new();
    let mut ugen = TestInPlusParamGen::<f32>::new();
    assert_eq!(ugen.process(&mut ctx, &mut flags, [7.]), [7.]);
    assert_eq!(
        UGen::process(&mut ugen, &mut ctx, &mut flags, [17.].into())[0],
        17.
    );
}
