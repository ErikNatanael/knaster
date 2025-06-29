use crate::log::ArLogReceiver;
use crate::math::{Add, Div, MathUGen, Mul, Sub};
use crate::typenum::{U1, U2};
use crate::{AudioCtx, UGen, UGenFlags};
use knaster_primitives::typenum::U4;
use knaster_primitives::{Block, StaticBlock};

#[test]
fn gen_arithmetics() {
    const SR: u32 = 48000;
    const BLOCK: usize = 4;
    let log_receiver = ArLogReceiver::new();
    let (logger, _log_receiver) = log_receiver.sender(100);
    let mut ctx = AudioCtx::new(SR, BLOCK, logger);
    let ctx = &mut ctx;
    let mut flags = UGenFlags::new();
    let mut b0 = StaticBlock::<f32, U2, U4>::new();
    let mut b1 = StaticBlock::<f32, U2, U4>::new();
    b0.channel_as_slice_mut(0).fill(3.0);
    b0.channel_as_slice_mut(1).fill(2.0);

    // Addition
    let mut m = MathUGen::<f32, U1, Add>::new();
    assert_eq!(m.process(ctx, &mut flags, [3.0, 2.0].into())[0], 5.0);
    m.process_block(ctx, &mut flags, &&b0, &mut b1);
    for &sample in b1.channel_as_slice(0) {
        assert_eq!(sample, 5.0);
    }
    //Sub
    let mut m = MathUGen::<f32, U1, Sub>::new();
    assert_eq!(m.process(ctx, &mut flags, [3.0, 2.0].into())[0], 1.0);
    m.process_block(ctx, &mut flags, &&b0, &mut b1);
    for &sample in b1.channel_as_slice(0) {
        assert_eq!(sample, 1.0);
    }
    // Div
    let mut m = MathUGen::<f32, U1, Div>::new();
    assert_eq!(m.process(ctx, &mut flags, [3.0, 2.0].into())[0], 1.5);
    m.process_block(ctx, &mut flags, &&b0, &mut b1);
    for &sample in b1.channel_as_slice(0) {
        assert_eq!(sample, 1.5);
    }
    // Mul
    let mut m = MathUGen::<f32, U1, Mul>::new();
    assert_eq!(m.process(ctx, &mut flags, [3.0, 2.0].into())[0], 6.0);
    m.process_block(ctx, &mut flags, &&b0, &mut b1);
    for &sample in b1.channel_as_slice(0) {
        assert_eq!(sample, 6.0);
    }
}
#[test]
fn gen_arithmetics_multichannel() {
    const SR: u32 = 48000;
    const BLOCK: usize = 4;
    let log_receiver = ArLogReceiver::new();
    let (logger, _log_receiver) = log_receiver.sender(100);
    let mut ctx = AudioCtx::new(SR, BLOCK, logger);
    let ctx = &mut ctx;
    let mut flags = UGenFlags::new();
    let mut b0 = StaticBlock::<f64, U4, U4>::new();
    let mut b1 = StaticBlock::<f64, U2, U4>::new();
    // Channels are laid out so that all the LHS come first, then all the RHS
    b0.channel_as_slice_mut(0).fill(3.0);
    b0.channel_as_slice_mut(1).fill(7.0);
    b0.channel_as_slice_mut(2).fill(2.0);
    b0.channel_as_slice_mut(3).fill(4.0);

    // Addition
    let mut m = MathUGen::<f64, U2, Add>::new();
    assert_eq!(
        m.process(ctx, &mut flags, [3.0, 7.0, 2.0, 4.0].into())[0..2],
        [5.0, 11.0]
    );
    m.process_block(ctx, &mut flags, &&b0, &mut b1);
    for &sample in b1.channel_as_slice(0) {
        assert_eq!(sample, 5.0);
    }
    for &sample in b1.channel_as_slice(1) {
        assert_eq!(sample, 11.0);
    }
}
