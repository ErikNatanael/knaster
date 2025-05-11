use knaster_primitives::{
    Float, Size,
    numeric_array::{NumericArray, narr},
    typenum::*,
};

use crate::core::boxed::Box;

use super::{AudioCtx, UGen, UGenFlags};

pub type UGenC<F, I, O> =
    Box<dyn FnMut(&mut AudioCtx, &mut UGenFlags, NumericArray<F, I>) -> NumericArray<F, O> + Send>;

pub struct UGenClosure<F, I: Size, O: Size> {
    closure: UGenC<F, I, O>,
}
impl<F, I: Size, O: Size> UGenClosure<F, I, O> {
    pub fn new(closure: UGenC<F, I, O>) -> Self {
        Self { closure }
    }
}

impl<F: Float, I: Size, O: Size> UGen for UGenClosure<F, I, O> {
    type Sample = F;

    type Inputs = I;

    type Outputs = O;

    type Parameters = U0;

    fn process(
        &mut self,
        ctx: &mut AudioCtx,
        flags: &mut UGenFlags,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        (self.closure)(ctx, flags, input)
    }

    fn param_hints()
    -> knaster_primitives::numeric_array::NumericArray<crate::ParameterHint, Self::Parameters> {
        [].into()
    }

    fn param_apply(
        &mut self,
        _ctx: &mut super::AudioCtx,
        _index: usize,
        _value: crate::ParameterValue,
    ) {
    }
}

// impl<F> Into<UGenClosure<F, U1, U1>>
//     for dyn FnMut(&mut AudioCtx, &mut UGenFlags, F) -> F + Send + 'static + Sized
// where
//     F: Float,
// {
//     fn into(self) -> UGenClosure<F, U1, U1> {
//         UGenClosure::new(Box::new(move |ctx, flags, input| {
//             let mut input = input.into();
//             let output = self(ctx, flags, input[0]);
//             input[0] = output;
//             input
//         }))
//     }
// }

// pub trait IntoUGenClosure<F: Float, I: Size, O: Size> {
//     fn into_ugen(self) -> UGenClosure<F, I, O>;
// }
// impl<F: Float, T> IntoUGenClosure<F, U1, U1> for T
// where
//     T: FnMut(&mut AudioCtx, &mut UGenFlags, F) -> F + Send + 'static + Sized,
// {
//     fn into_ugen(mut self) -> UGenClosure<F, U1, U1> {
//         UGenClosure::new(Box::new(move |ctx, flags, input| {
//             let output = narr!(self(ctx, flags, input[0]));
//             output
//         }))
//     }
// }
// impl<F: Float, T> From<T> for UGenClosure<F, U1, U1>
// where
//     T: FnMut(&mut AudioCtx, &mut UGenFlags, F) -> F + Send + 'static + Sized,
// {
//     fn from(mut t: T) -> UGenClosure<F, U1, U1> {
//         UGenClosure::new(Box::new(move |ctx, flags, input| {
//             let output = narr!(t(ctx, flags, input[0]));
//             output
//         }))
//     }
// }
impl<F: Float, T> From<T> for UGenClosure<F, U0, U1>
where
    T: FnMut(&mut AudioCtx, &mut UGenFlags) -> F + Send + 'static + Sized,
{
    fn from(mut t: T) -> UGenClosure<F, U0, U1> {
        UGenClosure::new(Box::new(move |ctx, flags, _input| {
            let output = narr!(t(ctx, flags));
            output
        }))
    }
}
impl<F: Float, T> From<T> for UGenClosure<F, U1, U0>
where
    T: FnMut(&mut AudioCtx, &mut UGenFlags, F) + Send + 'static + Sized,
{
    fn from(mut t: T) -> UGenClosure<F, U1, U0> {
        UGenClosure::new(Box::new(move |ctx, flags, input| {
            narr!(t(ctx, flags, input[0]));
            [].into()
        }))
    }
}

macro_rules! impl_ugen_from_closure_for {
    ($i:ident, $($j:ident),*) => {
        $(
            impl<F: Float, T> From<T> for UGenClosure<F, $i, $j>
            where
                T: FnMut(&mut AudioCtx, &mut UGenFlags, [F; $i::USIZE]) -> [F; $j::USIZE]
                    + Send
                    + 'static
                    + Sized,
            {
                fn from(mut t: T) -> UGenClosure<F, $i, $j> {
                    UGenClosure::new(Box::new(move |ctx, flags, input| {
                        let input = crate::core::array::from_fn(|i| input[i]);
                        (t(ctx, flags, input)).into()
                    }))
                }
            }
        )*
    };
}

macro_rules! impl_ugen_from_closure {
    ($($i:ident),*) => {
        $(
            impl_ugen_from_closure_for!($i, U1, U2, U3, U4, U5, U6, U7, U8);
        )*
    };
}
// macro_rules! impl_ugen_from_closure_for {
//     ($i:ident, $($j:ident),*) => {
//         $(
//             impl<F: Float, T> IntoUGenClosure<F, $i, $j> for T
//             where
//                 T: FnMut(&mut AudioCtx, &mut UGenFlags, [F; $i::USIZE]) -> [F; $j::USIZE]
//                     + Send
//                     + 'static
//                     + Sized,
//             {
//                 fn into_ugen(mut t: T) -> UGenClosure<F, $i, $j> {
//                     UGenClosure::new(Box::new(move |ctx, flags, input| {
//                         let input = crate::core::array::from_fn(|i| input[i]);
//                         (t(ctx, flags, input)).into()
//                     }))
//                 }
//             }
//         )*
//     };
// }
//
// macro_rules! impl_ugen_from_closure {
//     ($($i:ident),*) => {
//         $(
//             impl_ugen_from_closure_for!($i, U1, U2, U3, U4, U5, U6, U7, U8);
//         )*
//     };
// }

// Kör makrot
impl_ugen_from_closure!(U1, U2, U3, U4, U5, U6, U7, U8);

pub fn ugen<F: Float, I: Size, O: Size>(
    c: impl Into<UGenClosure<F, I, O>>,
) -> UGenClosure<F, I, O> {
    c.into()
}

#[cfg(test)]
mod tests {
    use knaster_primitives::Frame;

    use crate::log::ArLogSender;

    use super::*;

    #[test]
    fn test_ugen() {
        let mut ctx = AudioCtx::new(44100, 64, ArLogSender::non_rt());
        let mut flags = UGenFlags::new();
        let mut ugen = ugen(|_ctx: &mut AudioCtx, _flags: &mut UGenFlags, s: [f32; 1]| {
            let s = s[0].tanh();
            let s = if s.is_nan() { 0.0 } else { s };
            [s]
        });
        let mut input = Frame::default();
        let output = ugen.process(&mut ctx, &mut flags, input);
        assert_eq!(output[0], 0.0);
        input[0] = 1.0;
        let output = ugen.process(&mut ctx, &mut flags, input);
        assert_eq!(output[0], 1.0_f32.tanh());
    }
}
