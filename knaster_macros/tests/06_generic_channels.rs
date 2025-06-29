use std::marker::PhantomData;

// A UGen with a generic number of channels. This requires overriding the associated type.
use knaster_core::{
    AudioCtx, Block, BlockRead, Float, Frame, KnasterIntegerParameter, PFloat, PFloatHint,
    PInteger, Size, StaticBlock, UGen, UGenFlags,
    log::ArLogSender,
    num_derive::{FromPrimitive, ToPrimitive},
    num_traits,
    typenum::*,
};

/// Outputs a static number every frame
pub(crate) struct TestInPlusParamGen<F, InputChannels, OutputChannels> {
    number: F,
    _in: PhantomData<InputChannels>,
    _out: PhantomData<OutputChannels>,
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
impl<F: Float, InputChannels: Size, OutputChannels: Size>
    TestInPlusParamGen<F, InputChannels, OutputChannels>
{
    type Inputs = InputChannels;
    type Outputs = OutputChannels;

    pub fn new() -> Self {
        Self {
            number: F::ZERO,
            _in: PhantomData,
            _out: PhantomData,
        }
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
    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        let mut outp = Frame::default();
        for chan in 0..Self::Inputs::USIZE.min(Self::Outputs::USIZE) {
            outp[chan] = input[chan] + self.number;
        }
        outp
    }

    fn process_block<InBlock, OutBlock>(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: BlockRead<Sample = Self::Sample>,
        OutBlock: Block<Sample = Self::Sample>,
    {
        for chan in 0..Self::Inputs::USIZE.min(Self::Outputs::USIZE) {
            for (inp, outp) in input
                .channel_as_slice(chan)
                .iter()
                .zip(output.channel_as_slice_mut(chan))
            {
                *outp = *inp + self.number;
            }
        }
    }
}

fn main() {
    assert_eq!(
        TestInPlusParamGen::<f32, U0, U1>::param_descriptions()[0],
        "number"
    );
    assert_eq!(
        TestInPlusParamGen::<f32, U3, U9>::param_descriptions()[1],
        "number2"
    );
    assert_eq!(
        TestInPlusParamGen::<f32, U0, U0>::param_hints()[1].float_hint(),
        Some(&PFloatHint::new().minmax(0.5, 1.0).default(0.1))
    );
    assert_eq!(
        TestInPlusParamGen::<f64, U2, U2>::param_descriptions()[2],
        "number3"
    );
    assert_eq!(
        TestInPlusParamGen::<f32, U2, U2>::param_hints()[2].float_hint(),
        Some(
            &PFloatHint::new()
                .kind(knaster_core::FloatParameterKind::Frequency)
                .default(100.)
        )
    );
    assert_eq!(
        TestInPlusParamGen::<f32, U1, U1>::param_descriptions()[3],
        "number4"
    );
    let p3_descriptions = TestInPlusParamGen::<f32, U100, U0>::param_hints()[3]
        .integer_hint()
        .unwrap()
        .descriptions()
        .unwrap();
    assert_eq!(p3_descriptions[0].1, "Zero");
    assert_eq!(p3_descriptions[3].1, "Three");
    let mut ctx = AudioCtx::new(44100, 64, ArLogSender::non_rt());
    let mut flags = UGenFlags::new();
    let mut ugen = TestInPlusParamGen::<f64, U1, U1>::new();
    let mut input = StaticBlock::<f64, knaster_core::typenum::U1, U64>::new();
    input.channel_as_slice_mut(0).fill(17.0);
    let mut output = StaticBlock::<f64, knaster_core::typenum::U1, U64>::new();
    UGen::process_block(&mut ugen, &mut ctx, &mut flags, &input, &mut output);
}
