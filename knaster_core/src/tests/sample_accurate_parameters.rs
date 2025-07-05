use float_cmp::approx_eq;
use knaster_primitives::{Block, StaticBlock, typenum::*};

use crate::{
    AudioCtx, UGen, UGenFlags,
    log::ArLogReceiver,
    tests::utils::TestInPlusParamGen,
    wrappers_core::{UGenWrapperCoreExt, WrPreciseTiming},
};

#[test]
fn sample_accurate_parameters_test() {
    const BLOCK_SIZE: usize = 16;
    const SR: u32 = 48000;

    let log_receiver = ArLogReceiver::new();
    let (logger, _log_receiver) = log_receiver.sender(100);
    let mut ctx = AudioCtx::new(SR, BLOCK_SIZE, logger);
    let ctx = &mut ctx;
    let mut flags = UGenFlags::new();
    let mut g = WrPreciseTiming::<10, _>::new(TestInPlusParamGen::new());
    g.set_delay_within_block_for_param(ctx, 0, 5);
    g.param(ctx, 0, 5.).unwrap();
    g.set_delay_within_block_for_param(ctx, 0, 6);
    g.param(ctx, 0, 6.).unwrap();
    g.set_delay_within_block_for_param(ctx, 0, 8);
    g.param(ctx, 0, 8.).unwrap();
    g.set_delay_within_block_for_param(ctx, 0, 9);
    g.param(ctx, 0, 9.).unwrap();
    g.set_delay_within_block_for_param(ctx, 0, 10);
    g.param(ctx, 0, 10.).unwrap();

    let in_block = StaticBlock::<f32, U2, U16>::new();
    let mut out_block = StaticBlock::<f32, U2, U16>::new();

    g.process_block(ctx, &mut flags, &in_block, &mut out_block);

    let o = out_block.channel_as_slice(0);
    assert_eq!(
        o,
        [
            0., 0., 0., 0., 0., 5., 6., 6., 8., 9., 10., 10., 10., 10., 10., 10.
        ]
    );
}
#[test]
fn sample_accurate_parameters_with_wrappers_test() {
    const BLOCK_SIZE: usize = 16;
    const SR: u32 = 48000;
    let log_receiver = ArLogReceiver::new();
    let (logger, _log_receiver) = log_receiver.sender(100);
    let mut ctx = AudioCtx::new(SR, BLOCK_SIZE, logger);
    let ctx = &mut ctx;

    let mut flags = UGenFlags::new();
    let g = WrPreciseTiming::<10, _>::new(TestInPlusParamGen::new());
    // Add arithmetic wrappers_graph that have no effect to test that they all pass the delay through properly
    let mut g = g
        .wr_add(0.0)
        .wr_sub(0.0)
        .wr_div(1.0)
        .wr_mul(1.0)
        .wr_powf(1.0)
        .wr_powi(1)
        .wr(|v| v);
    g.set_delay_within_block_for_param(ctx, 0, 5);
    g.param(ctx, 0, 5.).unwrap();
    g.set_delay_within_block_for_param(ctx, 0, 6);
    g.param(ctx, 0, 6.).unwrap();
    g.set_delay_within_block_for_param(ctx, 0, 8);
    g.param(ctx, 0, 8.).unwrap();
    g.set_delay_within_block_for_param(ctx, 0, 9);
    g.param(ctx, 0, 9.).unwrap();
    g.set_delay_within_block_for_param(ctx, 0, 10);
    g.param(ctx, 0, 10.).unwrap();

    let in_block = StaticBlock::<f32, U2, U16>::new();
    let mut out_block = StaticBlock::<f32, U2, U16>::new();

    g.process_block(ctx, &mut flags, &in_block, &mut out_block);

    let o = out_block.channel_as_slice(0);
    let expected_values = [
        0., 0., 0., 0., 0., 5., 6., 6., 8., 9., 10., 10., 10., 10., 10., 10.,
    ];
    for (sample, expected) in o.iter().zip(expected_values.iter()) {
        assert!(approx_eq!(
            f32,
            *sample,
            *expected,
            epsilon = 0.0002,
            ulps = 5
        ))
    }
}
