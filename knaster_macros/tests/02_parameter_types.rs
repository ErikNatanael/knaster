use knaster_core::{
    AudioCtx, ControlRange, Float, PInteger, ParameterValue, UGen, UGenFlags,
    controls::ControlMetadata, log::ArLogSender,
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
    pub fn number_int(&mut self, n: PInteger) {
        self.number = F::new(n.0 as f32);
    }
    #[param]
    pub fn keep_number(&mut self, t: bool) {
        if !t {
            self.number = F::ZERO;
        }
    }
    #[param]
    pub fn t_reset(&mut self) {
        self.number = F::new(0.);
    }
    fn process(&mut self, _ctx: &mut AudioCtx, _flags: &mut UGenFlags, input: [F; 1]) -> [F; 1] {
        [self.number + input[0]]
    }
}

fn main() {
    assert_eq!(
        TestInPlusParamGen::<f32>::param_descriptions()[0],
        "number_int"
    );
    assert_eq!(
        TestInPlusParamGen::<f32>::param_descriptions()[1],
        "keep_number"
    );
    assert_eq!(
        TestInPlusParamGen::<f32>::param_descriptions()[2],
        "t_reset"
    );
    let mut ctx = AudioCtx::new(44100, 64, ArLogSender::non_rt());
    let mut flags = UGenFlags::new();
    let mut ugen = TestInPlusParamGen::<f32>::new();
    let cm = ugen.control_metadata();
    assert_eq!(
        cm[0],
        ControlMetadata::new("number", 0.0, ControlRange::Infinity)
    );
    assert_eq!(ugen.process(&mut ctx, &mut flags, [7.]), [7.]);
    ugen.number(2.);
    assert_eq!(
        UGen::process(&mut ugen, &mut ctx, &mut flags, [17.].into())[0],
        17. + 2.
    );
    ugen.param_apply(&mut ctx, 0, ParameterValue::Integer(PInteger(3)));
    assert_eq!(
        UGen::process(&mut ugen, &mut ctx, &mut flags, [17.].into())[0],
        17. + 3.
    );
    ugen.control(&mut ctx, 0, 8.);
    assert_eq!(
        UGen::process(&mut ugen, &mut ctx, &mut flags, [17.].into())[0],
        17. + 8.
    );
}
