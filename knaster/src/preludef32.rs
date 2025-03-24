pub use crate::subprelude_fundamental_types::*;

pub type OscWt = knaster_graph::osc::OscWt<f32>;
pub type SinWt = knaster_graph::osc::SinWt<f32>;
pub type Constant = knaster_graph::util::Constant<f32>;

pub type MathUGen<N, Op> = knaster_graph::math::MathUGen<f32, N, Op>;

pub type Graph = knaster_graph::graph::Graph<f32>;

pub type EnvAr = knaster_graph::envelopes::EnvAr<f32>;
pub type EnvAsr = knaster_graph::envelopes::EnvAsr<f32>;

pub type Pan2 = knaster_graph::pan::Pan2<f32>;

pub type WhiteNoise = knaster_graph::noise::WhiteNoise<f32>;
pub type PinkNoise = knaster_graph::noise::PinkNoise<f32>;
pub type BrownNoise = knaster_graph::noise::BrownNoise<f32>;

pub type PolyBlep = knaster_graph::polyblep::PolyBlep<f32>;

pub type SvfFilter = knaster_graph::svf::SvfFilter<f32>;

pub type OnePoleLpf = knaster_graph::onepole::OnePoleLpf<f32>;
pub type OnePoleHpf = knaster_graph::onepole::OnePoleHpf<f32>;

pub type AllpassDelay = knaster_graph::delay::AllpassDelay<f32>;
pub type AllpassFeedbackDelay = knaster_graph::delay::AllpassFeedbackDelay<f32>;

pub type DoneOnTrig = knaster_graph::util::DoneOnTrig<f32>;
