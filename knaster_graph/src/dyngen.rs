use core::any::{Any, TypeId};

use alloc::boxed::Box;

use crate::block::{AggregateBlockRead, RawBlock};
use knaster_core::math::MathUGen;
use knaster_core::{AudioCtx, Float, ParameterValue, UGen, UGenFlags};
use knaster_core::{ParameterHint, typenum::*};

/// Type erasing trait to allow us to store [`UGen`]s as trait objects. It
/// requires all nodes that are added to the [`Graph`] to implement both [`UGen`]
/// and [`Parameterable`].
///
///
/// For type erasure, we cannot be generic over the types of blocks. This is not
/// a problem since this interface is essentially Graph internal. A different
/// graph implementation can make a different tradeoff with different types.
pub trait DynUGen<F: Float> {
    fn init(&mut self, sample_rate: u32, block_size: usize);
    fn process_block(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: &AggregateBlockRead<F>,
        output: &mut RawBlock<F>,
    ) where
        F: Float;
    fn inputs(&self) -> u16;
    fn outputs(&self) -> u16;
    fn parameters(&self) -> u16;

    unsafe fn set_ar_param_buffer(&mut self, ctx: &mut AudioCtx, index: usize, buffer: *const F);
    fn set_delay_within_block_for_param(&mut self, ctx: &mut AudioCtx, index: usize, delay: u16);
    fn param_apply(&mut self, ctx: &mut AudioCtx, parameter: usize, value: ParameterValue);
    fn param_description_fn(&self) -> fn(usize) -> Option<&'static str>;
    fn param_hints_fn(&self) -> fn(usize) -> Option<ParameterHint>;
}
impl<F: Float, T: UGen<Sample = F> + 'static> DynUGen<F> for T {
    fn init(&mut self, sample_rate: u32, block_size: usize) {
        self.init(sample_rate, block_size)
    }

    fn process_block(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: &AggregateBlockRead<F>,
        output: &mut RawBlock<F>,
    ) where
        F: Float,
    {
        self.process_block(ctx, flags, input, output)
    }

    fn inputs(&self) -> u16 {
        T::Inputs::U16
    }
    fn outputs(&self) -> u16 {
        T::Outputs::U16
    }
    fn parameters(&self) -> u16 {
        T::Parameters::U16
    }

    unsafe fn set_ar_param_buffer(&mut self, ctx: &mut AudioCtx, index: usize, buffer: *const F) {
        unsafe { self.set_ar_param_buffer(ctx, index, buffer) };
    }

    fn set_delay_within_block_for_param(&mut self, ctx: &mut AudioCtx, index: usize, delay: u16) {
        self.set_delay_within_block_for_param(ctx, index, delay);
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, parameter: usize, value: ParameterValue) {
        self.param_apply(ctx, parameter, value);
    }

    fn param_description_fn(&self) -> fn(usize) -> Option<&'static str> {
        |index: usize| T::param_descriptions().get(index).copied()
    }

    fn param_hints_fn(&self) -> fn(usize) -> Option<ParameterHint> {
        |index: usize| T::param_hints().get(index).copied()
    }
}

/// Convert a UGen to a UGenEnum
///
/// This looks noisy, but in testing the casts are optimized away by the compiler. If specialization is
/// stablized, this can be written more cleanly.
fn convert_to_ugen_enum<F: Float, T: DynUGen<F> + 'static>(t: T) -> UGenEnum<F> {
    if TypeId::of::<T>() == TypeId::of::<MathUGen<F, U1, knaster_core::math::Mul>>() {
        let boxed: Box<dyn Any> = Box::new(t);
        let typed = boxed
            .downcast::<MathUGen<F, U1, knaster_core::math::Mul>>()
            .unwrap();
        UGenEnum::Mul(*typed)
    } else {
        UGenEnum::Dyn(Box::new(t))
    }
}

/// UGenEnum has concrete variants for the most common UGens, with a fallback to boxed trait
/// objects for the rest. This speeds up execution since both the heap allocation and the pointer
/// indirection of the trait object are eliminated.
///
/// Carefully benchmark code when chaning what is in this enum. Variants with structs much larger than 16 bytes
/// should be boxed or left to the default case if benchmarks show that their inclusion slows down execution.
pub enum UGenEnum<F: Float> {
    /// None is used to move the UGenEnum between Tasks, while avaiding branching on an Option any
    /// time a UGenEnum is used.
    None,
    Mul(MathUGen<F, U1, knaster_core::math::Mul>),
    Add(MathUGen<F, U1, knaster_core::math::Add>),
    Sub(MathUGen<F, U1, knaster_core::math::Sub>),
    Div(MathUGen<F, U1, knaster_core::math::Div>),
    Dyn(Box<dyn DynUGen<F> + 'static>),
}

impl<F: Float> DynUGen<F> for UGenEnum<F> {
    fn init(&mut self, sample_rate: u32, block_size: usize) {
        match self {
            UGenEnum::None => {}
            UGenEnum::Mul(ugen) => UGen::init(ugen, sample_rate, block_size),
            UGenEnum::Add(ugen) => UGen::init(ugen, sample_rate, block_size),
            UGenEnum::Sub(ugen) => UGen::init(ugen, sample_rate, block_size),
            UGenEnum::Div(ugen) => UGen::init(ugen, sample_rate, block_size),
            UGenEnum::Dyn(ugen) => DynUGen::init(&mut (**ugen), sample_rate, block_size),
        }
    }

    fn process_block(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: &AggregateBlockRead<F>,
        output: &mut RawBlock<F>,
    ) where
        F: Float,
    {
        todo!()
    }

    fn inputs(&self) -> u16 {
        todo!()
    }

    fn outputs(&self) -> u16 {
        todo!()
    }

    fn parameters(&self) -> u16 {
        todo!()
    }

    unsafe fn set_ar_param_buffer(&mut self, ctx: &mut AudioCtx, index: usize, buffer: *const F) {
        todo!()
    }

    fn set_delay_within_block_for_param(&mut self, ctx: &mut AudioCtx, index: usize, delay: u16) {
        todo!()
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, parameter: usize, value: ParameterValue) {
        todo!()
    }

    fn param_description_fn(&self) -> fn(usize) -> Option<&'static str> {
        todo!()
    }

    fn param_hints_fn(&self) -> fn(usize) -> Option<ParameterHint> {
        todo!()
    }
}
