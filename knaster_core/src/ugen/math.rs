use core::marker::PhantomData;

use knaster_primitives::{
    numeric_array::NumericArray,
    typenum::{Double, U0, U1},
    Float, Size,
};

use crate::UGen;

use super::{AudioCtx, UGenFlags};

pub trait Operation<T> {
    #[cfg(feature = "unstable")]
    const SIMD_WIDTH: usize =
        if let Some(size) = target_features::CURRENT_TARGET.suggested_simd_width::<f32>() {
            size
        } else {
            // If SIMD isn't supported natively, we use a vector of 1 element.
            // This is effectively a scalar value.
            1
        };
    fn apply(a: &[T], b: &[T], out: &mut [T]);
}
// TODO: Implement SIMD operations for different architectures using portable-simd or intrinsics
pub struct Add;
impl<T: crate::core::ops::Add<Output = T> + Float> Operation<T> for Add {
    #[inline(always)]
    fn apply(a: &[T], b: &[T], out: &mut [T]) {
        debug_assert!(a.len() == b.len() && a.len() == out.len());
        // Scalar implementation
        for ((a, b), out) in a.iter().zip(b.iter()).zip(out.iter_mut()) {
            *out = *a + *b;
        }
    }
}
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
/// pairs of input channels.
pub struct MathUGen<F: Float, Channels: Size, Op: Operation<F>> {
    marker: PhantomData<(NumericArray<F, Channels>, Op)>,
}
impl<F: Float, Channels: Size, Op: Operation<F>> MathUGen<F, Channels, Op> {
    pub fn new() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}
impl<F: Float, Channels: Size, Op: Operation<F>> UGen for MathUGen<F, Channels, Op>
where
    Channels: crate::core::ops::Shl<knaster_primitives::typenum::B1> + Send,
    <Channels as crate::core::ops::Shl<knaster_primitives::typenum::B1>>::Output: Size,
{
    type Sample = F;
    type Inputs = Double<Channels>;
    type Outputs = Channels;

    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut UGenFlags,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
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
        InBlock: knaster_primitives::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_primitives::Block<Sample = Self::Sample>,
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
    fn param_hints() -> NumericArray<crate::ParameterHint, Self::Parameters> {
        NumericArray::from([])
    }
    fn param_apply(&mut self, _ctx: &mut AudioCtx, _index: usize, _value: crate::ParameterValue) {
    }
}

/// Mathematical operation applied to a single number (e.g. sqrt, fract, ceil)
pub trait Operation1<T> {
    #[cfg(feature = "unstable")]
    const SIMD_WIDTH: usize =
        if let Some(size) = target_features::CURRENT_TARGET.suggested_simd_width::<f32>() {
            size
        } else {
            // If SIMD isn't supported natively, we use a vector of 1 element.
            // This is effectively a scalar value.
            1
        };
    fn apply(a: &[T], out: &mut [T]);
}
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
/// are arguments (usually from different UGens). E.g. for addition: a0 + b0, a1
/// + b1, a2 + b2. `Channels` is the number of output channels or the number of
/// pairs of input channels.
pub struct Math1UGen<F: Float, Op: Operation1<F>> {
    marker: PhantomData<(F, Op)>,
}
impl<F: Float, Op: Operation1<F>> Math1UGen<F, Op> {
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
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
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
        InBlock: knaster_primitives::BlockRead<Sample = Self::Sample>,
        OutBlock: knaster_primitives::Block<Sample = Self::Sample>,
    {
        Op::apply(input.channel_as_slice(0), output.channel_as_slice_mut(0));
    }
    type Parameters = U0;
    fn param_descriptions() -> NumericArray<&'static str, Self::Parameters> {
        NumericArray::default()
    }
    fn param_hints() -> NumericArray<crate::ParameterHint, Self::Parameters> {
        NumericArray::from([])
    }
    fn param_apply(&mut self, _ctx: &mut AudioCtx, _index: usize, _value: crate::ParameterValue) {
    }
}
