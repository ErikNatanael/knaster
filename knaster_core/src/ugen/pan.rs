//! # Panning
//!
//! UGens related to panning
use core::marker::PhantomData;

use knaster_macros::impl_ugen;
use knaster_primitives::Float;

/// Pan a mono signal to stereo using the cos/sine pan law. Pan value should be
/// between -1 and 1, 0 being in the center.
///
// TODO: Implement multiple different pan laws, maybe as a generic.
pub struct Pan2<F: Copy> {
    pan: f32,
    _phantom: PhantomData<F>,
}
#[impl_ugen]
impl<F: Float> Pan2<F> {
    #[allow(missing_docs)]
    pub fn new(pan: f32) -> Self {
        Self {
            pan: pan * 0.5 + 0.5,
            _phantom: PhantomData,
        }
    }
    /// Set the pan value in the range (-1.0, 1.0)
    #[param]
    pub fn pan(&mut self, pan: f32) {
        self.pan = pan * 0.5 + 0.5;
    }
    #[allow(missing_docs)]
    pub fn process(&mut self, input: [F; 1]) -> [F; 2] {
        let signal = input[0];
        let pan_pos_radians = self.pan * crate::core::f32::consts::FRAC_PI_2;
        let left_gain = F::new(fastapprox::fast::cos(pan_pos_radians));
        let right_gain = F::new(fastapprox::fast::sin(pan_pos_radians));
        [signal * left_gain, signal * right_gain]
    }
}
