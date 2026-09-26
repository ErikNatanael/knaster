use std::marker::PhantomData;

// A UGen with a generic number of channels. This requires overriding the associated type.
use knaster_core::{
    AudioCtx, Block, BlockRead, ControlPurpose, ControlRange, Float, Frame,
    KnasterIntegerParameter, PFloat, PInteger, Size, StaticBlock, UGen, UGenFlags,
    controls::ControlMetadata,
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
    #[control(default = 0.0)]
    pub fn number(&mut self, n: F) {
        self.number = F::new(n);
    }
    #[control(default = 0.1, range = 0.5..=1.0)]
    pub fn number2(&mut self, ctx: &mut AudioCtx, n: F) {
        self.number = n + F::new(ctx.block_size() as f64);
    }
    #[control(default = 100.0, purpose = Frequency)]
    pub fn number3(&mut self, ctx: &AudioCtx, n: F) {
        self.number = n * F::new(ctx.sample_rate() as f64);
    }
    #[param(default = 0, from = NumberEnum)]
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
        InBlock: BlockRead<Sample = Self::Sample> + ?Sized,
        OutBlock: Block<Sample = Self::Sample> + ?Sized,
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
    let mut ctx = AudioCtx::new(44100, 64, ArLogSender::non_rt());
    let mut flags = UGenFlags::new();
    let mut ugen = TestInPlusParamGen::<f64, U1, U1>::new();
    let cm = ugen.control_metadata();
    assert_eq!(
        cm[0],
        ControlMetadata::new("number", 0.0, ControlRange::Infinity)
    );
    assert_eq!(
        cm[1],
        ControlMetadata::new("number2", 0.1, ControlRange::RangeInclusive(0.5, 1.0))
    );
    assert_eq!(
        cm[2],
        ControlMetadata::new("number3", 100., ControlRange::Infinity)
            .with_purpose(ControlPurpose::Frequency)
    );

    let mut input = StaticBlock::<f64, knaster_core::typenum::U1, U64>::new();
    input.channel_as_slice_mut(0).fill(17.0);
    let mut output = StaticBlock::<f64, knaster_core::typenum::U1, U64>::new();
    UGen::process_block(&mut ugen, &mut ctx, &mut flags, &input, &mut output);
}
