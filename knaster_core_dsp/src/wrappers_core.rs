//! Wrappers are composable type that wrap aroung a [`Gen`] + [`Parameterable`]
//! to provide some extra functionality.
//!
//! Wrapper types begin by "Wr" by convention to make it easier to spot what's what.

// TODO: Wrapper types that take one value per output channel

mod audio_rate;
mod closure;
mod precise_timing;
pub use closure::*;
pub use precise_timing::*;
mod math;
pub use math::*;

pub use audio_rate::*;
mod smooth_params;
pub use smooth_params::*;

use knaster_core::UGen;

/// Adds methods as shortcuts for adding a range of wrappers_graph to any [`UGen`]
///
/// The methods all take `self`, returning the new wrapper. Math operation
/// wrappers_graph start with `wr_` to disambiguate them from `std::ops::*`
pub trait UGenWrapperCoreExt<T: UGen> {
    /// Apply closure `c` to every sample of every channel of the wrapped UGen, i.e. `c(self)`.
    fn wr<C: FnMut(T::Sample) -> T::Sample + 'static>(self, c: C) -> WrClosure<T, C>;
    /// Multiply the output of the wrapped UGen by `v`, i.e. `self * v`.
    fn wr_mul(self, v: T::Sample) -> WrMul<T>;
    /// Add `v` to the output of the wrapped UGen, i.e. `self + v`.
    fn wr_add(self, v: T::Sample) -> WrAdd<T>;
    /// Subtract `v` from the output of the wrapped UGen, i.e. `self - v`.
    fn wr_sub(self, v: T::Sample) -> WrSub<T>;
    /// Subtract the output of the wrapped UGen from `v`, i.e. `v - self`.
    fn wr_v_sub_gen(self, v: T::Sample) -> WrVSub<T>;
    /// Divide the output of the wrapped UGen by `v`, i.e. `self / v`.
    fn wr_div(self, v: T::Sample) -> WrDiv<T>;
    /// Divide `v` by the output of the wrapped UGen, i.e. `v / self`.
    fn wr_v_div_gen(self, v: T::Sample) -> WrVDiv<T>;
    /// Raise the output of the wrapped UGen to the power of `v`, i.e. `self.powf(v)`.
    fn wr_powf(self, v: T::Sample) -> WrPowf<T>;
    /// Raise the output of the wrapped UGen to the power of `v`, i.e. `self.powi(v)`.
    fn wr_powi(self, v: i32) -> WrPowi<T>;
    /// Enable smoothing/easing functions for float parameters
    fn smooth_params(self) -> WrSmoothParams<T>;
    /// Enable setting a parameter to an audio rate signal
    fn ar_params(self) -> WrArParams<T>;
    /// Precise timing. Requires a generic parameter for how many changes can be handled per block.
    ///
    /// Unless you are sending very frequent changes or are using a huge block size, a low number
    /// will suffice.
    fn precise_timing<const MAX_CHANGES_PER_BLOCK: usize>(
        self,
    ) -> WrPreciseTiming<MAX_CHANGES_PER_BLOCK, T>;
}

impl<T: UGen> UGenWrapperCoreExt<T> for T {
    fn wr_mul(self, v: T::Sample) -> WrMul<T> {
        WrMul::new(self, v)
    }

    fn smooth_params(self) -> WrSmoothParams<T> {
        WrSmoothParams::new(self)
    }

    fn wr_add(self, v: <T as UGen>::Sample) -> WrAdd<T> {
        WrAdd::new(self, v)
    }

    fn wr_sub(self, v: <T as UGen>::Sample) -> WrSub<T> {
        WrSub::new(self, v)
    }

    fn wr_div(self, v: <T as UGen>::Sample) -> WrDiv<T> {
        WrDiv::new(self, v)
    }

    fn wr_powf(self, v: <T as UGen>::Sample) -> WrPowf<T> {
        WrPowf::new(self, v)
    }

    fn wr_v_sub_gen(self, v: <T as UGen>::Sample) -> WrVSub<T> {
        WrVSub::new(self, v)
    }

    fn wr_v_div_gen(self, v: <T as UGen>::Sample) -> WrVDiv<T> {
        WrVDiv::new(self, v)
    }

    fn wr<C: FnMut(<T as UGen>::Sample) -> <T as UGen>::Sample + 'static>(
        self,
        c: C,
    ) -> WrClosure<T, C> {
        WrClosure::new(self, c)
    }

    fn ar_params(self) -> WrArParams<T> {
        WrArParams::new(self)
    }

    fn wr_powi(self, v: i32) -> WrPowi<T> {
        WrPowi::new(self, v)
    }

    fn precise_timing<const MAX_CHANGES_PER_BLOCK: usize>(
        self,
    ) -> WrPreciseTiming<MAX_CHANGES_PER_BLOCK, T> {
        WrPreciseTiming::new(self)
    }
}
#[cfg(test)]
mod tests {
    use float_cmp::approx_eq;
    use knaster_core::typenum::{U2, U16};

    use super::UGenWrapperCoreExt;
    use crate::test_utils::{TestInPlusParamGen, TestNumUGen};
    use crate::wrappers_core::WrPreciseTiming;
    use knaster_core::log::{ArLogReceiver, ArLogSender};
    use knaster_core::{AudioCtx, Block, StaticBlock, UGen, UGenFlags};

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
        let sample = g.process(ctx, &mut flags, [].into())[0];
        assert!(
            (sample - 36.0f64).abs() < f32::EPSILON as f64 * 10.,
            "Expected 6^2 = 36, got {}, diff {}",
            sample,
            (sample - 36.0f64).abs(),
        );
        let mut g = TestNumUGen::new(6.0).wr_powi(2);
        let sample = g.process(ctx, &mut flags, [].into())[0];
        assert!(
            approx_eq!(
                f64,
                sample,
                36.,
                epsilon = f32::EPSILON as f64 * 2.,
                ulps = 2
            ),
            "Expected 6^2 = 36, got {}",
            sample
        );
        let mut g = TestNumUGen::new(6.0).wr(|s| s * 2.0 + 1.0);
        assert_eq!(g.process(ctx, &mut flags, [].into())[0], 13.);
    }

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
}
