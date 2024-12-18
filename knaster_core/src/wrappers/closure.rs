use crate::{Gen, Parameterable};

/// Applies the closure to every sample of every channel in the [`Gen`] output
///
/// This is almost certainly not as performant as using wrappers dedicated to a specific
/// math operation, but good for prototyping and for when performance is not so important.
pub struct WrClosure<T: Gen + Parameterable<T::Sample>, C: FnMut(T::Sample) -> T::Sample + 'static>
{
    gen: T,
    closure: C,
}
impl<T: Gen + Parameterable<T::Sample>, C: FnMut(T::Sample) -> T::Sample + 'static>
    WrClosure<T, C>
{
    pub fn new(gen: T, closure: C) -> Self {
        Self { gen, closure }
    }
}
impl<T: Gen + Parameterable<T::Sample>, C: FnMut(T::Sample) -> T::Sample + 'static> Gen
    for WrClosure<T, C>
{
    type Sample = T::Sample;
    type Inputs = T::Inputs;
    type Outputs = T::Outputs;

    fn process(
        &mut self,
        ctx: &mut crate::AudioCtx,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        let mut out = self.gen.process(ctx, input);
        for sample in &mut out {
            *sample = (self.closure)(*sample);
        }
        out
    }
}

impl<T: Gen + Parameterable<T::Sample>, C: FnMut(T::Sample) -> T::Sample + 'static>
    Parameterable<T::Sample> for WrClosure<T, C>
{
    type Parameters = T::Parameters;

    fn param_descriptions(
    ) -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        T::param_descriptions()
    }

    fn param_default_values(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterValue, Self::Parameters>
    {
        T::param_default_values()
    }

    fn param_range(
    ) -> knaster_primitives::numeric_array::NumericArray<crate::ParameterRange, Self::Parameters>
    {
        T::param_range()
    }

    fn param_apply(&mut self, ctx: &crate::AudioCtx, index: usize, value: crate::ParameterValue) {
        T::param_apply(&mut self.gen, ctx, index, value)
    }
}
