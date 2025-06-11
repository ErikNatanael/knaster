use core::marker::PhantomData;

use knaster_primitives::Float;

pub struct SafetyLimiter<F: Float> {
    _float: PhantomData<F>,
}
#[knaster_macros::impl_ugen]
impl<F: Float> SafetyLimiter<F> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            _float: PhantomData,
        }
    }
    fn process(
        &mut self,
        _ctx: &mut super::AudioCtx,
        _flags: &mut super::UGenFlags,
        input: [F; 1],
    ) -> [F; 1] {
        let s = input[0];
        let s = s.clamp(F::new(-1.0), F::new(1.0));
        let s = if s.is_nan() { F::new(0.0) } else { s };
        [s]
    }
}
