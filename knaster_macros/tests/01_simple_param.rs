use knaster_core::{
    AudioCtx, Float, ParameterValue, UGen, UGenFlags, log::ArLogSender, typenum::*,
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
    #[param(default = 0.0)]
    pub fn number(&mut self, n: f64) {
        self.number = F::new(n);
    }
    fn process(&mut self, _ctx: &mut AudioCtx, _flags: &mut UGenFlags, input: [F; 1]) -> [F; 1] {
        [self.number + input[0]]
    }
}

fn main() {
    assert_eq!(TestInPlusParamGen::<f32>::param_descriptions()[0], "number");
    let mut ctx = AudioCtx::new(44100, 64, ArLogSender::non_rt());
    let mut flags = UGenFlags::new();
    let mut ugen = TestInPlusParamGen::<f32>::new();
    assert_eq!(ugen.process(&mut ctx, &mut flags, [7.]), [7.]);
    ugen.number(2.);
    assert_eq!(
        UGen::process(&mut ugen, &mut ctx, &mut flags, [17.].into())[0],
        17. + 2.
    );
    ugen.param_apply(&mut ctx, 0, ParameterValue::Float(3.));
    assert_eq!(
        UGen::process(&mut ugen, &mut ctx, &mut flags, [17.].into())[0],
        17. + 3.
    );
}
