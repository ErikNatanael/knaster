use knaster_core::{
    AudioCtx, Block, Float, KnasterIntegerParameter, PFloat, PFloatHint, PInteger, ParameterValue,
    StaticBlock, UGen, UGenFlags, log::ArLogSender, num_derive::FromPrimitive,
    num_derive::ToPrimitive, num_traits, typenum::U64,
};

/// Outputs a static number every frame
pub(crate) struct TestInPlusParamGen<F> {
    number: F,
}

#[derive(
    Default, Debug, PartialEq, Eq, Copy, Clone, FromPrimitive, ToPrimitive, KnasterIntegerParameter,
)]
#[num_traits = "num_traits"]
#[repr(u8)]
pub enum NumberEnum {
    #[default]
    Zero = 0,
    One,
    Two,
    Three,
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
    #[param(default = 0.1, range = 0.5..=1.0)]
    pub fn number2(&mut self, ctx: &mut AudioCtx, n: f32) {
        self.number = F::new(n + ctx.block_size() as f32);
    }
    #[param(default = 100.0, kind = Frequency)]
    pub fn number3(&mut self, ctx: &AudioCtx, n: PFloat) {
        self.number = F::new(n * ctx.sample_rate() as PFloat);
    }
    #[param(default = 0.0, from = NumberEnum)]
    pub fn number4(&mut self, n: PInteger) {
        self.number = F::new(n.0 as PFloat);
    }
    #[param]
    pub fn t_reset(&mut self) {
        self.number = F::new(0.);
    }
    fn process(&mut self, _ctx: &mut AudioCtx, _flags: &mut UGenFlags, input: [F; 1]) -> [F; 1] {
        [self.number + input[0]]
    }
    fn process_block(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        inputs: [&[F]; 1],
        outputs: [&mut [F]; 1],
    ) {
        for (&inp, out) in inputs[0].iter().zip(outputs[0].iter_mut()) {
            *out = inp + self.number;
        }
    }
}

fn main() {
    assert_eq!(TestInPlusParamGen::<f32>::param_descriptions()[0], "number");
    assert_eq!(
        TestInPlusParamGen::<f32>::param_descriptions()[1],
        "number2"
    );
    assert_eq!(
        TestInPlusParamGen::<f32>::param_hints()[1].float_hint(),
        Some(&PFloatHint::new().minmax(0.5, 1.0).default(0.1))
    );
    assert_eq!(
        TestInPlusParamGen::<f32>::param_descriptions()[2],
        "number3"
    );
    assert_eq!(
        TestInPlusParamGen::<f32>::param_hints()[2].float_hint(),
        Some(
            &PFloatHint::new()
                .kind(knaster_core::FloatParameterKind::Frequency)
                .default(100.)
        )
    );
    assert_eq!(
        TestInPlusParamGen::<f32>::param_descriptions()[3],
        "number4"
    );
    let p3_descriptions = TestInPlusParamGen::<f32>::param_hints()[3]
        .integer_hint()
        .unwrap()
        .descriptions()
        .unwrap();
    assert_eq!(p3_descriptions[0].1, "Zero");
    assert_eq!(p3_descriptions[3].1, "Three");
    let mut ctx = AudioCtx::new(44100, 64, ArLogSender::non_rt());
    let mut flags = UGenFlags::new();
    let mut ugen = TestInPlusParamGen::<f64>::new();
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
    let mut input = StaticBlock::<f64, knaster_core::typenum::U1, U64>::new();
    input.channel_as_slice_mut(0).fill(17.0);
    let mut output = StaticBlock::<f64, knaster_core::typenum::U1, U64>::new();
    UGen::process_block(&mut ugen, &mut ctx, &mut flags, &input, &mut output);
}
