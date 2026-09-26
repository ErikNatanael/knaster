use knaster_core::{
    AudioCtx, Block, ControlRange, Float, PInteger, StaticBlock, UGen, UGenFlags,
    controls::ControlMetadata, log::ArLogSender, typenum::*,
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
    assert_eq!(
        TestInPlusParamGen::<f32>::param_descriptions()[0],
        "number_int"
    );
    let mut ctx = AudioCtx::new(44100, 64, ArLogSender::non_rt());
    let mut flags = UGenFlags::new();
    let mut ugen = TestInPlusParamGen::<f64>::new();
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
    ugen.control(&mut ctx, 0, 3.0);
    assert_eq!(
        UGen::process(&mut ugen, &mut ctx, &mut flags, [17.].into())[0],
        17. + 3.
    );
    let mut input = StaticBlock::<f64, U1, U64>::new();
    input.channel_as_slice_mut(0).fill(17.0);
    let mut output = StaticBlock::<f64, U1, U64>::new();
    UGen::process_block(&mut ugen, &mut ctx, &mut flags, &input, &mut output);
}
