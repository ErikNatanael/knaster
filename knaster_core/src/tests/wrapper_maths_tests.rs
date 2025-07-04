use crate::log::ArLogSender;
use crate::tests::utils::TestNumUGen;
use crate::wrappers_core::UGenWrapperCoreExt;
use crate::{AudioCtx, UGen, UGenFlags};

#[test]
fn wrapper_arithmetic() {
    let logger = ArLogSender::non_rt();
    let mut ctx = AudioCtx::new(48000, 4, logger);
    let ctx = &mut ctx;
    let mut flags = UGenFlags::new();
    let mut g = TestNumUGen::new(2.5).wr_add(2.5);
    assert_eq!(g.process(ctx, &mut flags, [].into())[0], 5.0);
    let mut g = TestNumUGen::new(2.5).wr_mul(3.);
    assert_eq!(g.process(ctx, &mut flags, [].into())[0], 7.5);
    let mut g = TestNumUGen::new(2.5).wr_div(5.);
    assert_eq!(g.process(ctx, &mut flags, [].into())[0], 0.5);
    let mut g = TestNumUGen::new(2.5).wr_v_div_gen(5.);
    assert_eq!(g.process(ctx, &mut flags, [].into())[0], 2.);
    let mut g = TestNumUGen::new(6.0).wr_sub(7.);
    assert_eq!(g.process(ctx, &mut flags, [].into())[0], -1.0);
    let mut g = TestNumUGen::new(6.0).wr_v_sub_gen(7.);
    assert_eq!(g.process(ctx, &mut flags, [].into())[0], 1.0);
    let mut g = TestNumUGen::new(6.0).wr_powf(2.);
    assert_eq!(g.process(ctx, &mut flags, [].into())[0], 36.0);
    let mut g = TestNumUGen::new(6.0).wr_powi(2);
    assert_eq!(g.process(ctx, &mut flags, [].into())[0], 36.);
    let mut g = TestNumUGen::new(6.0).wr(|s| s * 2.0 + 1.0);
    assert_eq!(g.process(ctx, &mut flags, [].into())[0], 13.);
}
