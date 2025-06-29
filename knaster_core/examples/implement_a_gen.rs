//! We implement a naive implementation of a cosine wave oscillator, similar to `CosMath`

use anyhow::Result;
use knaster_core::{
    AudioCtx, Block, EmptyBlock, Float, Frame, Param, ParameterError, ParameterHint, ParameterType,
    ParameterValue, StaticBlock, UGen, UGenFlags,
    log::ArLogSender,
    numeric_array::NumericArray,
    typenum::{U0, U1, U3, U64, Unsigned},
};
fn main() -> Result<()> {
    // Let's pretend we're running an audio backend at 48kHz with a block size of 64.
    let mut ctx = AudioCtx::new(48000, 64, ArLogSender::non_rt());
    let mut flags = UGenFlags::new();
    let mut osc = Osc::new();
    // Since we own the Osc directly, and it isn't wrapped in anything, we can
    // set the frequency directly:
    osc.freq(200., ctx.sample_rate() as f32);
    // We can also use the Parameterable trait interface
    osc.param(&mut ctx, "freq", 200.)?;

    // # Generating audio
    // We can generate frames one by one:
    let output = osc.process(&mut ctx, &mut flags, [].into());
    assert_eq!(output[0], 0.0);
    // Or in blocks
    let mut output_block = StaticBlock::<_, U1, U64>::new();
    // The double referece is needed for
    osc.process_block(&mut ctx, &mut flags, &EmptyBlock::new(), &mut output_block);
    assert!(
        (output_block.read(0, 63)
            - ((200.0 / ctx.sample_rate() as f32) * core::f32::consts::TAU * 64.).sin())
        .abs()
            < f32::EPSILON
    );
    Ok(())
}

pub struct Osc<F> {
    phase: F,
    phase_offset: F,
    phase_increment: F,
}
impl<F: Float> Default for Osc<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: Float> Osc<F> {
    pub fn new() -> Self {
        Self {
            phase: F::ZERO,
            phase_offset: F::ZERO,
            phase_increment: F::ZERO,
        }
    }
    pub fn freq(&mut self, freq: F, sample_rate: F) {
        self.phase_increment = freq / sample_rate;
    }
    pub fn phase_offset(&mut self, phase_offset: F) {
        self.phase_offset = phase_offset;
    }
}

impl<F: Float> UGen for Osc<F> {
    type Sample = F;
    type Inputs = U0;
    type Outputs = U1;

    fn init(&mut self, _sample_rate: u32, _block_size: usize) {}

    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        _input: Frame<Self::Sample, Self::Inputs>,
    ) -> Frame<Self::Sample, Self::Outputs> {
        let out = (self.phase * F::from(std::f32::consts::TAU).unwrap()).sin();
        self.phase += self.phase_increment;
        if self.phase > F::ONE {
            self.phase -= F::ONE;
        }
        NumericArray::from([out])
    }
    type Parameters = U3;

    fn param_types() -> NumericArray<ParameterType, Self::Parameters> {
        NumericArray::from([
            ParameterType::Float,
            ParameterType::Float,
            ParameterType::Trigger,
        ])
    }

    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        NumericArray::from(["freq", "phase_offset", "reset_phase"])
    }

    fn param_hints() -> NumericArray<ParameterHint, Self::Parameters> {
        todo!()
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, index: usize, value: ParameterValue) {
        match index {
            0 => self.freq(
                F::from(value.float().unwrap()).unwrap(),
                F::from(ctx.sample_rate()).unwrap(),
            ),
            1 => self.phase_offset(F::from(value.float().unwrap()).unwrap()),
            2 => self.phase_offset(F::ZERO),
            _ => (),
        }
    }

    fn param(
        &mut self,
        ctx: &mut AudioCtx,
        param: impl Into<Param>,
        value: impl Into<ParameterValue>,
    ) -> Result<(), ParameterError> {
        let ctx = ctx.into();
        let var_name = match param.into() {
            Param::Index(i) => {
                if i >= Self::Parameters::USIZE {
                    return Err(ParameterError::ParameterIndexOutOfBounds);
                }
                self.param_apply(ctx, i, value.into());
                Ok(())
            }
            Param::Desc(desc) => {
                for (i, d) in Self::param_descriptions().into_iter().enumerate() {
                    if d == desc {
                        self.param_apply(ctx, i, value.into());
                        return Ok(());
                    }
                }
                Err(ParameterError::DescriptionNotFound(desc))
            }
        };
        var_name
    }
}
