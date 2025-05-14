use core::marker::PhantomData;

use knaster_primitives::Float;
use knaster_primitives::typenum::{U1, U2};

use super::{AudioCtx, UGen};

/// Pan a mono signal to stereo using the cos/sine pan law. Pan value should be
/// between -1 and 1, 0 being in the center.
///
// TODO: Implement multiple different pan laws, maybe as a generic.
pub struct Pan2<F: Copy> {
    pan: f32,
    _phantom: PhantomData<F>,
}
impl<F: Float> Pan2<F> {
    pub fn new(pan: f32) -> Self {
        Self {
            pan: pan * 0.5 + 0.5,
            _phantom: PhantomData,
        }
    }
}
impl<F: Float> UGen for Pan2<F> {
    type Sample = F;

    type Inputs = U1;

    type Outputs = U2;

    type Parameters = U1;

    fn process(
        &mut self,
        _ctx: &mut AudioCtx,
        _flags: &mut super::UGenFlags,
        input: knaster_primitives::Frame<Self::Sample, Self::Inputs>,
    ) -> knaster_primitives::Frame<Self::Sample, Self::Outputs> {
        let signal = input[0];
        let pan_pos_radians = self.pan * crate::core::f32::consts::FRAC_PI_2;
        let left_gain = F::new(fastapprox::fast::cos(pan_pos_radians));
        let right_gain = F::new(fastapprox::fast::sin(pan_pos_radians));
        [signal * left_gain, signal * right_gain].into()
    }

    fn param_hints()
    -> knaster_primitives::numeric_array::NumericArray<crate::ParameterHint, Self::Parameters> {
        [crate::ParameterHint::new_float(|h| h.minmax(-1., 1.))].into()
    }

    fn param_descriptions()
    -> knaster_primitives::numeric_array::NumericArray<&'static str, Self::Parameters> {
        ["pan"].into()
    }

    fn param_apply(&mut self, _ctx: &mut AudioCtx, index: usize, value: crate::ParameterValue) {
        match index {
            0 => self.pan = value.float().unwrap() as f32 * 0.5 + 0.5,
            _ => (),
        }
    }
}
