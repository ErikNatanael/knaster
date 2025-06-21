
use alloc::boxed::Box;

use crate::block::{AggregateBlockRead, RawBlock};
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

/// UGenEnum has concrete variants for the most common UGens, with a fallback to boxed trait
/// objects for the rest. This speeds up execution since both the heap allocation and the pointer
/// indirection of the trait object are eliminated.
///
/// Carefully benchmark code when chaning what is in this enum. Variants with structs much larger than 16 bytes
/// should be boxed or left to the default case if benchmarks show that their inclusion slows down execution.
pub enum UGenEnum<F: Float> {
    // Add(MathUGen<F, U1, knaster_core::math::Add>),
    // Mul(MathUGen<F, U1, knaster_core::math::Mul>),
    // Sub(MathUGen<F, U1, knaster_core::math::Sub>),
    // Div(MathUGen<F, U1, knaster_core::math::Div>),
    // Constant(Constant<F>),
    // SinWt(knaster_core::osc::SinWt<F>),
    Dyn(Box<dyn DynUGen<F> + 'static>),
    /// TakeFromTask is a placeholder for a UGen in a Task when its real UGen is already on the
    /// audio thread. It will then be taken from its previous task at TaskData initialisation.
    TakeFromTask(usize),
    /// None is used to move the UGenEnum between Tasks, while avaiding branching on an Option any
    /// time a UGenEnum is used.
    None,
}
impl<F: Float> UGenEnum<F> {
    pub fn take(&mut self) -> UGenEnum<F> {
        std::mem::replace(self, UGenEnum::None)
    }
    /// Convert a UGen to a UGenEnum, specializing to a UGenEnum variant when possible.
    pub fn from_ugen<T: DynUGen<F> + 'static>(t: T) -> Self {
        // This looks noisy, but in testing the casts are optimized away by the compiler. If specialization is
        // stablized, this can be written more cleanly.
        // if TypeId::of::<T>() == TypeId::of::<MathUGen<F, U1, knaster_core::math::Mul>>() {
        //     let boxed: Box<dyn Any> = Box::new(t);
        //     let typed = boxed
        //         .downcast::<MathUGen<F, U1, knaster_core::math::Mul>>()
        //         .unwrap();
        //     UGenEnum::Mul(*typed)
        // } else if TypeId::of::<T>() == TypeId::of::<MathUGen<F, U1, knaster_core::math::Add>>() {
        //     let boxed: Box<dyn Any> = Box::new(t);
        //     let typed = boxed
        //         .downcast::<MathUGen<F, U1, knaster_core::math::Add>>()
        //         .unwrap();
        //     UGenEnum::Add(*typed)
        // } else if TypeId::of::<T>() == TypeId::of::<MathUGen<F, U1, knaster_core::math::Sub>>() {
        //     let boxed: Box<dyn Any> = Box::new(t);
        //     let typed = boxed
        //         .downcast::<MathUGen<F, U1, knaster_core::math::Sub>>()
        //         .unwrap();
        //     UGenEnum::Sub(*typed)
        // } else if TypeId::of::<T>() == TypeId::of::<MathUGen<F, U1, knaster_core::math::Div>>() {
        //     let boxed: Box<dyn Any> = Box::new(t);
        //     let typed = boxed
        //         .downcast::<MathUGen<F, U1, knaster_core::math::Div>>()
        //         .unwrap();
        //     UGenEnum::Div(*typed)
        // } else if TypeId::of::<T>() == TypeId::of::<Constant<F>>() {
        //     let boxed: Box<dyn Any> = Box::new(t);
        //     let typed = boxed.downcast::<Constant<F>>().unwrap();
        //     UGenEnum::Constant(*typed)
        // } else if TypeId::of::<T>() == TypeId::of::<knaster_core::osc::SinWt<F>>() {
        //     let boxed: Box<dyn Any> = Box::new(t);
        //     let typed = boxed.downcast::<knaster_core::osc::SinWt<F>>().unwrap();
        //     UGenEnum::SinWt(*typed)
        // } else {
        UGenEnum::Dyn(Box::new(t))
        // }
    }
}

impl<F: Float> DynUGen<F> for UGenEnum<F> {
    fn init(&mut self, sample_rate: u32, block_size: usize) {
        match self {
            UGenEnum::None => {}
            UGenEnum::TakeFromTask(_) => {}
            // UGenEnum::Mul(ugen) => UGen::init(ugen, sample_rate, block_size),
            // UGenEnum::Add(ugen) => UGen::init(ugen, sample_rate, block_size),
            // UGenEnum::Sub(ugen) => UGen::init(ugen, sample_rate, block_size),
            // UGenEnum::Div(ugen) => UGen::init(ugen, sample_rate, block_size),
            // UGenEnum::Constant(ugen) => UGen::init(ugen, sample_rate, block_size),
            // UGenEnum::SinWt(ugen) => UGen::init(ugen, sample_rate, block_size),
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
        if let UGenEnum::Dyn(ugen) = self {
            DynUGen::process_block(&mut (**ugen), ctx, flags, input, output)
        } else {
            unreachable!("process_block called on a non-DynUGen")
        }
        // match self {
        //     // UGenEnum::Add(ugen) => {
        //     //     // let a = input.channel_as_slice(0);
        //     //     // let b = input.channel_as_slice(1);
        //     //     // let out = output.channel_as_slice_mut(0);
        //     //     // for ((a, b), out) in a.iter().zip(b.iter()).zip(out.iter_mut()) {
        //     //     //     *out = *a + *b;
        //     //     // }
        //     //
        //     //     UGen::process_block(ugen, ctx, flags, input, output)
        //     // }
        //     // UGenEnum::Mul(ugen) => {
        //     //     // let a = input.channel_as_slice(0);
        //     //     // let b = input.channel_as_slice(1);
        //     //     // let out = output.iter_mut().next().unwrap();
        //     //     // for ((a, b), out) in a.iter().zip(b.iter()).zip(out.iter_mut()) {
        //     //     //     *out = *a * *b;
        //     //     // }
        //     //     UGen::process_block(ugen, ctx, flags, input, output)
        //     // }
        //     // UGenEnum::Sub(ugen) => UGen::process_block(ugen, ctx, flags, input, output),
        //     // UGenEnum::Div(ugen) => UGen::process_block(ugen, ctx, flags, input, output),
        //     // UGenEnum::Constant(ugen) => UGen::process_block(ugen, ctx, flags, input, output),
        //     // UGenEnum::SinWt(ugen) => UGen::process_block(ugen, ctx, flags, input, output),
        //     UGenEnum::Dyn(ugen) => DynUGen::process_block(&mut (**ugen), ctx, flags, input, output),
        //     UGenEnum::TakeFromTask(_) => {}
        //     UGenEnum::None => {}
        // }
    }

    fn inputs(&self) -> u16 {
        match self {
            UGenEnum::None => 0,
            UGenEnum::TakeFromTask(_) => 0,
            // UGenEnum::Mul(ugen) => DynUGen::inputs(ugen),
            // UGenEnum::Add(ugen) => DynUGen::inputs(ugen),
            // UGenEnum::Sub(ugen) => DynUGen::inputs(ugen),
            // UGenEnum::Div(ugen) => DynUGen::inputs(ugen),
            // UGenEnum::Constant(ugen) => DynUGen::inputs(ugen),
            // UGenEnum::SinWt(ugen) => DynUGen::inputs(ugen),
            UGenEnum::Dyn(ugen) => DynUGen::inputs(&(**ugen)),
        }
    }

    fn outputs(&self) -> u16 {
        match self {
            UGenEnum::None => 0,
            UGenEnum::TakeFromTask(_) => 0,
            // UGenEnum::Mul(ugen) => DynUGen::outputs(ugen),
            // UGenEnum::Add(ugen) => DynUGen::outputs(ugen),
            // UGenEnum::Sub(ugen) => DynUGen::outputs(ugen),
            // UGenEnum::Div(ugen) => DynUGen::outputs(ugen),
            // UGenEnum::Constant(ugen) => DynUGen::outputs(ugen),
            // UGenEnum::SinWt(ugen) => DynUGen::outputs(ugen),
            UGenEnum::Dyn(ugen) => DynUGen::outputs(&(**ugen)),
        }
    }

    fn parameters(&self) -> u16 {
        match self {
            UGenEnum::None => 0,
            UGenEnum::TakeFromTask(_) => 0,
            // UGenEnum::Mul(ugen) => DynUGen::parameters(ugen),
            // UGenEnum::Add(ugen) => DynUGen::parameters(ugen),
            // UGenEnum::Sub(ugen) => DynUGen::parameters(ugen),
            // UGenEnum::Div(ugen) => DynUGen::parameters(ugen),
            // UGenEnum::Constant(ugen) => DynUGen::parameters(ugen),
            // UGenEnum::SinWt(ugen) => DynUGen::parameters(ugen),
            UGenEnum::Dyn(ugen) => DynUGen::parameters(&(**ugen)),
        }
    }

    unsafe fn set_ar_param_buffer(&mut self, ctx: &mut AudioCtx, index: usize, buffer: *const F) {
        unsafe {
            match self {
                UGenEnum::None => {}
                UGenEnum::TakeFromTask(_) => {}
                // UGenEnum::Mul(ugen) => UGen::set_ar_param_buffer(ugen, ctx, index, buffer),
                // UGenEnum::Add(ugen) => UGen::set_ar_param_buffer(ugen, ctx, index, buffer),
                // UGenEnum::Sub(ugen) => UGen::set_ar_param_buffer(ugen, ctx, index, buffer),
                // UGenEnum::Div(ugen) => UGen::set_ar_param_buffer(ugen, ctx, index, buffer),
                // UGenEnum::Constant(ugen) => UGen::set_ar_param_buffer(ugen, ctx, index, buffer),
                // UGenEnum::SinWt(ugen) => UGen::set_ar_param_buffer(ugen, ctx, index, buffer),
                UGenEnum::Dyn(ugen) => {
                    DynUGen::set_ar_param_buffer(&mut (**ugen), ctx, index, buffer)
                }
            }
        }
    }

    fn set_delay_within_block_for_param(&mut self, ctx: &mut AudioCtx, index: usize, delay: u16) {
        match self {
            UGenEnum::None => {}
            UGenEnum::TakeFromTask(_) => {}
            // UGenEnum::Mul(ugen) => UGen::set_delay_within_block_for_param(ugen, ctx, index, delay),
            // UGenEnum::Add(ugen) => UGen::set_delay_within_block_for_param(ugen, ctx, index, delay),
            // UGenEnum::Sub(ugen) => UGen::set_delay_within_block_for_param(ugen, ctx, index, delay),
            // UGenEnum::Div(ugen) => UGen::set_delay_within_block_for_param(ugen, ctx, index, delay),
            // UGenEnum::Constant(ugen) => {
            //     UGen::set_delay_within_block_for_param(ugen, ctx, index, delay)
            // }
            // UGenEnum::SinWt(ugen) => {
            //     UGen::set_delay_within_block_for_param(ugen, ctx, index, delay)
            // }
            UGenEnum::Dyn(ugen) => {
                DynUGen::set_delay_within_block_for_param(&mut (**ugen), ctx, index, delay)
            }
        }
    }

    fn param_apply(&mut self, ctx: &mut AudioCtx, parameter: usize, value: ParameterValue) {
        match self {
            UGenEnum::None => {}
            UGenEnum::TakeFromTask(_) => {}
            // UGenEnum::Mul(ugen) => UGen::param_apply(ugen, ctx, parameter, value),
            // UGenEnum::Add(ugen) => UGen::param_apply(ugen, ctx, parameter, value),
            // UGenEnum::Sub(ugen) => UGen::param_apply(ugen, ctx, parameter, value),
            // UGenEnum::Div(ugen) => UGen::param_apply(ugen, ctx, parameter, value),
            // UGenEnum::Constant(ugen) => UGen::param_apply(ugen, ctx, parameter, value),
            // UGenEnum::SinWt(ugen) => UGen::param_apply(ugen, ctx, parameter, value),
            UGenEnum::Dyn(ugen) => DynUGen::param_apply(&mut (**ugen), ctx, parameter, value),
        }
    }

    fn param_description_fn(&self) -> fn(usize) -> Option<&'static str> {
        match self {
            UGenEnum::None => |_| None,
            UGenEnum::TakeFromTask(_) => |_| None,
            // UGenEnum::Mul(ugen) => DynUGen::param_description_fn(ugen),
            // UGenEnum::Add(ugen) => DynUGen::param_description_fn(ugen),
            // UGenEnum::Sub(ugen) => DynUGen::param_description_fn(ugen),
            // UGenEnum::Div(ugen) => DynUGen::param_description_fn(ugen),
            // UGenEnum::Constant(ugen) => DynUGen::param_description_fn(ugen),
            // UGenEnum::SinWt(ugen) => DynUGen::param_description_fn(ugen),
            UGenEnum::Dyn(ugen) => DynUGen::param_description_fn(&(**ugen)),
        }
    }

    fn param_hints_fn(&self) -> fn(usize) -> Option<ParameterHint> {
        match self {
            UGenEnum::None => |_| None,
            UGenEnum::TakeFromTask(_) => |_| None,
            // UGenEnum::Mul(ugen) => DynUGen::param_hints_fn(ugen),
            // UGenEnum::Add(ugen) => DynUGen::param_hints_fn(ugen),
            // UGenEnum::Sub(ugen) => DynUGen::param_hints_fn(ugen),
            // UGenEnum::Div(ugen) => DynUGen::param_hints_fn(ugen),
            // UGenEnum::Constant(ugen) => DynUGen::param_hints_fn(ugen),
            // UGenEnum::SinWt(ugen) => DynUGen::param_hints_fn(ugen),
            UGenEnum::Dyn(ugen) => DynUGen::param_hints_fn(&(**ugen)),
        }
    }
}
