//! # Math
//!
//! This module contains UGens for basic maths. These are usually used through methods
//! or operators, rather than manually pushing and connecting UGens.
use core::marker::PhantomData;

use knaster_core::{
    Float, ParameterHint, ParameterValue, Size,
    numeric_array::NumericArray,
    typenum::{Double, U0, U1},
};

use knaster_core::UGen;
use knaster_core::{AudioCtx, UGenFlags};

/// A maths operation of the form 2 in 1 out e.g. addition, multiplication etc.
pub trait Operation<T> {
    /// Apply the operation
    fn apply(a: &[T], b: &[T], out: &mut [T]);
}
/// Addition operation
pub struct Add;
impl<T: crate::core::ops::Add<Output = T> + Float> Operation<T> for Add {
    #[inline(always)]
    fn apply(a: &[T], b: &[T], out: &mut [T]) {
        debug_assert!(a.len() == b.len() && a.len() == out.len());
        // Scalar implementation which auto-vectorizes well
        for ((a, b), out) in a.iter().zip(b.iter()).zip(out.iter_mut()) {
            *out = *a + *b;
        }
        // The auto-vectorisation is faster than the hand-written SIMD code in benchmarks, so I'm disabling it for now.
        // #[cfg(feature = "unstable")]
        // {
        //     T::simd_add(a, b, out);
        // }
    }
}
/// Multiplication operation
pub struct Mul;
impl<T: crate::core::ops::Mul<Output = T> + Float> Operation<T> for Mul {
    #[inline(always)]
    fn apply(a: &[T], b: &[T], out: &mut [T]) {
        debug_assert!(a.len() == b.len() && a.len() == out.len());
        // Scalar implementation
        for ((a, b), out) in a.iter().zip(b.iter()).zip(out.iter_mut()) {
            *out = *a * *b;
        }
    }
}
/// Division operation
pub struct Div;
impl<T: crate::core::ops::Div<Output = T> + Float> Operation<T> for Div {
    #[inline(always)]
    fn apply(a: &[T], b: &[T], out: &mut [T]) {
        debug_assert!(a.len() == b.len() && a.len() == out.len());
        // Scalar implementation
        for ((a, b), out) in a.iter().zip(b.iter()).zip(out.iter_mut()) {
            *out = *a / *b;
        }
    }
}
/// Subtraction operation
pub struct Sub;
impl<T: crate::core::ops::Sub<Output = T> + Float> Operation<T> for Sub {
    #[inline(always)]
    fn apply(a: &[T], b: &[T], out: &mut [T]) {
        debug_assert!(a.len() == b.len() && a.len() == out.len());
        // Scalar implementation
        for ((a, b), out) in a.iter().zip(b.iter()).zip(out.iter_mut()) {
            *out = *a - *b;
        }
    }
}
/// Power operation, i.e. `a.powf(b)`
pub struct Pow;
impl<T: Float> Operation<T> for Pow {
    #[inline(always)]
    fn apply(a: &[T], b: &[T], out: &mut [T]) {
        debug_assert!(a.len() == b.len() && a.len() == out.len());
        // Scalar implementation
        for ((a, b), out) in a.iter().zip(b.iter()).zip(out.iter_mut()) {
            *out = a.powf(*b);
        }
    }
}

/// Applies standard mathematical operations, selected by `Op`, on its inputs.
///
/// Inputs are arranged in the order a0, a1, a2, b0, b1, b2 such that a and b
/// are arguments (usually from different UGens). E.g. for addition: a0 + b0, a1
/// + b1, a2 + b2. `Channels` is the number of output channels or the number of
///
/// pairs of input channels.
pub struct MathUGen<F: Float, Channels: Size, Op: Operation<F>> {
    marker: PhantomData<(NumericArray<F, Channels>, Op)>,
}
impl<F: Float, Channels: Size, Op: Operation<F>> MathUGen<F, Channels, Op> {
    #[allow(clippy::new_without_default)]
    #[allow(missing_docs)]
    pub fn new() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}
impl<F: Float, Channels: Size, Op: Operation<F>> UGen for MathUGen<F, Channels, Op>
where
    Channels: crate::core::ops::Shl<knaster_core::typenum::B1> + Send,
    <Channels as crate::core::ops::Shl<knaster_core::typenum::B1>>::Output: Size,
{
    type Sample = F;
    type Inputs = Double<Channels>;
    type Outputs = Channels;

    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        // This is probably quite inefficient, but block processing will be the
        // standard way of running.
        //
        // TODO: Benchmark to see if this is excruciatingly slow and, if so, if
        // a specialized `Operation` fn that is applied per element is faster.
        let mut out = NumericArray::default();
        let mut temp_out = [F::ZERO; 1];
        for channel in 0..Channels::USIZE {
            Op::apply(
                &[input[channel]],
                &[input[channel + Channels::USIZE]],
                &mut temp_out,
            );
            out[channel] = temp_out[0];
        }
        out
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_core::BlockRead<Sample = Self::Sample> + ?Sized,
        OutBlock: knaster_core::Block<Sample = Self::Sample> + ?Sized,
    {
        for channel in 0..Channels::USIZE {
            Op::apply(
                input.channel_as_slice(channel),
                input.channel_as_slice(channel + Channels::USIZE),
                output.channel_as_slice_mut(channel),
            );
        }
        // dbg!(input.channel_as_slice(1));
    }
    type Parameters = U0;
    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        NumericArray::default()
    }
    fn param_hints() -> NumericArray<ParameterHint, Self::Parameters> {
        NumericArray::from([])
    }
    fn param_apply(&mut self, _ctx: &mut AudioCtx, _index: usize, _value: ParameterValue) {}
}

/// Mathematical operation of the form 1 in 1 out (e.g. sqrt, fract, ceil)
pub trait Operation1<T> {
    /// Apply the operation
    fn apply(a: &[T], out: &mut [T]);
}
/// Ceiling operation
pub struct Ceil;
impl<T: Float> Operation1<T> for Ceil {
    #[inline(always)]
    fn apply(a: &[T], out: &mut [T]) {
        debug_assert!(a.len() == out.len());
        // Scalar implementation
        for (a, out) in a.iter().zip(out.iter_mut()) {
            *out = a.ceil();
        }
    }
}
/// Square root operation
pub struct Sqrt;
impl<T: Float> Operation1<T> for Sqrt {
    #[inline(always)]
    fn apply(a: &[T], out: &mut [T]) {
        debug_assert!(a.len() == out.len());
        // Scalar implementation
        for (a, out) in a.iter().zip(out.iter_mut()) {
            *out = a.sqrt();
        }
    }
}
/// Floor operation
pub struct Floor;
impl<T: Float> Operation1<T> for Floor {
    #[inline(always)]
    fn apply(a: &[T], out: &mut [T]) {
        debug_assert!(a.len() == out.len());
        // Scalar implementation
        for (a, out) in a.iter().zip(out.iter_mut()) {
            *out = a.floor();
        }
    }
}
/// Truncation operation, removes the non-integer part of a number
pub struct Trunc;
impl<T: Float> Operation1<T> for Trunc {
    #[inline(always)]
    fn apply(a: &[T], out: &mut [T]) {
        debug_assert!(a.len() == out.len());
        // Scalar implementation
        for (a, out) in a.iter().zip(out.iter_mut()) {
            *out = a.trunc();
        }
    }
}
/// Fractional operation
pub struct Fract;
impl<T: Float> Operation1<T> for Fract {
    #[inline(always)]
    fn apply(a: &[T], out: &mut [T]) {
        debug_assert!(a.len() == out.len());
        // Scalar implementation
        for (a, out) in a.iter().zip(out.iter_mut()) {
            *out = a.fract();
        }
    }
}
/// Exp operation, i.e. `e^a`
pub struct Exp;
impl<T: Float> Operation1<T> for Exp {
    #[inline(always)]
    fn apply(a: &[T], out: &mut [T]) {
        debug_assert!(a.len() == out.len());
        // Scalar implementation
        for (a, out) in a.iter().zip(out.iter_mut()) {
            *out = a.exp();
        }
    }
}

/// Applies standard mathematical operations, selected by `Op`, on its inputs.
///
/// Inputs are arranged in the order a0, a1, a2, b0, b1, b2 such that a and b
/// are arguments (usually from different UGens). E.g. for addition: a0 + b0, a1 + b1, a2 + b2. `Channels` is the number of output channels or the number of
/// pairs of input channels.
///
pub struct Math1UGen<F: Float, Op: Operation1<F>> {
    marker: PhantomData<(F, Op)>,
}
impl<F: Float, Op: Operation1<F>> Math1UGen<F, Op> {
    #[allow(clippy::new_without_default)]
    #[allow(missing_docs)]
    pub fn new() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}
impl<F: Float, Op: Operation1<F>> UGen for Math1UGen<F, Op> {
    type Sample = F;
    type Inputs = U1;
    type Outputs = U1;

    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        input: knaster_core::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_core::Frame<Self::Sample, Self::Outputs> {
        // This is probably quite inefficient, but block processing will be the
        // standard way of running.
        //
        // TODO: Benchmark to see if this is excruciatingly slow and, if so, if
        // a specialized `Operation` fn that is applied per element is faster.
        let mut out = NumericArray::default();
        let mut temp_out = [F::ZERO; 1];
        Op::apply(&[input[0]], &mut temp_out);
        out[0] = temp_out[0];
        out
    }
    fn process_block<InBlock, OutBlock>(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        input: &InBlock,
        output: &mut OutBlock,
    ) where
        InBlock: knaster_core::BlockRead<Sample = Self::Sample> + ?Sized,
        OutBlock: knaster_core::Block<Sample = Self::Sample> + ?Sized,
    {
        Op::apply(input.channel_as_slice(0), output.channel_as_slice_mut(0));
    }
    type Parameters = U0;
    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        NumericArray::default()
    }
    fn param_hints() -> NumericArray<ParameterHint, Self::Parameters> {
        NumericArray::from([])
    }
    fn param_apply(&mut self, _ctx: &mut AudioCtx, _index: usize, _value: ParameterValue) {}
}

#[cfg(test)]
mod tests {
    use crate::math::{Add, Div, MathUGen, Mul, Sub};
    use knaster_core::{
        AudioCtx, Block, StaticBlock, UGen, UGenFlags,
        log::ArLogReceiver,
        typenum::{U1, U2, U4},
    };

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
        m.process_block(ctx, &mut flags, &b0, &mut b1);
        for &sample in b1.channel_as_slice(0) {
            assert_eq!(sample, 5.0);
        }
        //Sub
        let mut m = MathUGen::<f32, U1, Sub>::new();
        assert_eq!(m.process(ctx, &mut flags, [3.0, 2.0].into())[0], 1.0);
        m.process_block(ctx, &mut flags, &b0, &mut b1);
        for &sample in b1.channel_as_slice(0) {
            assert_eq!(sample, 1.0);
        }
        // Div
        let mut m = MathUGen::<f32, U1, Div>::new();
        assert_eq!(m.process(ctx, &mut flags, [3.0, 2.0].into())[0], 1.5);
        m.process_block(ctx, &mut flags, &b0, &mut b1);
        for &sample in b1.channel_as_slice(0) {
            assert_eq!(sample, 1.5);
        }
        // Mul
        let mut m = MathUGen::<f32, U1, Mul>::new();
        assert_eq!(m.process(ctx, &mut flags, [3.0, 2.0].into())[0], 6.0);
        m.process_block(ctx, &mut flags, &b0, &mut b1);
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
        m.process_block(ctx, &mut flags, &b0, &mut b1);
        for &sample in b1.channel_as_slice(0) {
            assert_eq!(sample, 5.0);
        }
        for &sample in b1.channel_as_slice(1) {
            assert_eq!(sample, 11.0);
        }
    }
}
