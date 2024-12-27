use knaster_primitives::{Block, VecBlock};

use crate::{tests::utils::TestInPlusParamGen, wrappers::{GenWrapperExt, WrHiResParams}, AudioCtx, BlockAudioCtx, Gen, GenFlags, Parameterable};


#[test]
fn sample_accurate_parameters_test() {
    const BLOCK_SIZE: usize = 16;
    let ctx = BlockAudioCtx::new(AudioCtx::new(48000, BLOCK_SIZE));
    let mut flags = GenFlags::new();
    let mut g = WrHiResParams::<10, _>::new(TestInPlusParamGen::new());
    g.set_delay_within_block_for_param(0, 5);
    g.param(ctx.into(), 0, 5.).unwrap();
    g.set_delay_within_block_for_param(0, 6);
    g.param((ctx).into(), 0, 6.).unwrap();
    g.set_delay_within_block_for_param(0, 8);
    g.param((ctx).into(), 0, 8.).unwrap();
    g.set_delay_within_block_for_param(0, 9);
    g.param((ctx).into(), 0, 9.).unwrap();
    g.set_delay_within_block_for_param(0, 10);
    g.param((ctx).into(), 0, 10.).unwrap();

    let in_block= VecBlock::<f32>::new(2, 16);
    let mut out_block = VecBlock::new(2, 16);

    g.process_block(ctx, &mut flags, &&in_block, &mut out_block);

    let o = out_block.channel_as_slice(0);
    assert_eq!(o, [0., 0., 0., 0., 0., 5., 6., 6., 8., 9., 10., 10., 10., 10., 10., 10.]);

}
#[test]
fn sample_accurate_parameters_with_wrappers_test() {
    const BLOCK_SIZE: usize = 16;
    let ctx = BlockAudioCtx::new(AudioCtx::new(48000, BLOCK_SIZE));

    let mut flags = GenFlags::new();
    let g = WrHiResParams::<10, _>::new(TestInPlusParamGen::new());
    // Add arithmetic wrappers that have no effect to test that they all pass the delay through properly
    let mut g = g.wr_add(0.0); //.wr_sub(0.0).wr_div(0.0).wr_mul(1.0).wr_powf(1.0).wr_powi(1).wr(|v| v);
    g.set_delay_within_block_for_param(0, 5);
    g.param((ctx).into(), 0, 5.).unwrap();
    g.set_delay_within_block_for_param(0, 6);
    g.param((ctx).into(), 0, 6.).unwrap();
    g.set_delay_within_block_for_param(0, 8);
    g.param((ctx).into(), 0, 8.).unwrap();
    g.set_delay_within_block_for_param(0, 9);
    g.param((ctx).into(), 0, 9.).unwrap();
    g.set_delay_within_block_for_param(0, 10);
    g.param((ctx).into(), 0, 10.).unwrap();

    let in_block= VecBlock::<f32>::new(2, 16);
    let mut out_block = VecBlock::new(2, 16);

    g.process_block(ctx,&mut flags, &&in_block, &mut out_block);

    let o = out_block.channel_as_slice(0);
    assert_eq!(o, [0., 0., 0., 0., 0., 5., 6., 6., 8., 9., 10., 10., 10., 10., 10., 10.]);

}