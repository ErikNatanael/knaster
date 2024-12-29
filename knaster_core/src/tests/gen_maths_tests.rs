use knaster_primitives::{Block, VecBlock};
use crate::{AudioCtx, BlockAudioCtx, Gen, GenFlags};
use crate::math::{Add, Div, MathGen, Mul, Sub};
use crate::typenum::{U1, U2};

#[test]
fn gen_arithmetics() {
    const SR:u32 = 48000;
    const BLOCK: usize = 4;
    let ctx = AudioCtx::new(SR, BLOCK);
    let bctx = BlockAudioCtx::new(ctx);
    let mut flags = GenFlags::new();
    let mut b0 = VecBlock::new(2, BLOCK);
    let mut b1 = VecBlock::new(2, BLOCK);
    b0.channel_as_slice_mut(0).fill(3.0);
    b0.channel_as_slice_mut(1).fill(2.0);

    // Addition
    let mut m = MathGen::<f32, U1, Add>::new();
    assert_eq!(m.process(ctx, &mut flags, [3.0, 2.0].into())[0], 5.0);
    m.process_block(bctx, &mut flags, &&b0, &mut b1);
    for &sample in b1.channel_as_slice(0) {
        assert_eq!(sample, 5.0);
    }
    //Sub
    let mut m = MathGen::<f32, U1, Sub>::new();
    assert_eq!(m.process(ctx, &mut flags, [3.0, 2.0].into())[0], 1.0);
    m.process_block(bctx, &mut flags, &&b0, &mut b1);
    for &sample in b1.channel_as_slice(0) {
        assert_eq!(sample, 1.0);
    }
    // Div
    let mut m = MathGen::<f32, U1, Div>::new();
    assert_eq!(m.process(ctx, &mut flags, [3.0, 2.0].into())[0], 1.5);
    m.process_block(bctx, &mut flags, &&b0, &mut b1);
    for &sample in b1.channel_as_slice(0) {
        assert_eq!(sample, 1.5);
    }
    // Mul
    let mut m = MathGen::<f32, U1, Mul>::new();
    assert_eq!(m.process(ctx, &mut flags, [3.0, 2.0].into())[0], 6.0);
    m.process_block(bctx, &mut flags, &&b0, &mut b1);
    for &sample in b1.channel_as_slice(0) {
        assert_eq!(sample, 6.0);
    }
}
#[test]
fn gen_arithmetics_multichannel() {
    const SR: u32 = 48000;
    const BLOCK: usize = 4;
    let ctx = AudioCtx::new(SR, BLOCK);
    let bctx = BlockAudioCtx::new(ctx);
    let mut flags = GenFlags::new();
    let mut b0 = VecBlock::new(4, BLOCK);
    let mut b1 = VecBlock::new(2, BLOCK);
    // Channels are laid out so that all the LHS come first, then all the RHS
    b0.channel_as_slice_mut(0).fill(3.0);
    b0.channel_as_slice_mut(1).fill(7.0);
    b0.channel_as_slice_mut(2).fill(2.0);
    b0.channel_as_slice_mut(3).fill(4.0);

    // Addition
    let mut m = MathGen::<f64, U2, Add>::new();
    assert_eq!(m.process(ctx, &mut flags, [3.0, 7.0, 2.0, 4.0].into())[0..2], [5.0, 11.0]);
    m.process_block(bctx, &mut flags, &&b0, &mut b1);
    for &sample in b1.channel_as_slice(0) {
        assert_eq!(sample, 5.0);
    }
    for &sample in b1.channel_as_slice(1) {
        assert_eq!(sample, 11.0);
    }
}