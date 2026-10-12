pub use crate::subprelude_fundamental_types::*;

pub type OscWt = knaster_graph::osc::OscWt<f64>;
pub type SinWt = knaster_graph::osc::SinWt<f64>;
pub type Constant = knaster_graph::util::Constant<f64>;

pub type MathUGen<N, Op> = knaster_graph::math::MathUGen<f64, N, Op>;

pub type Graph = knaster_graph::graph::Graph<f64>;

pub type EnvAr = knaster_graph::envelopes::EnvAr<f64>;
pub type EnvAsr = knaster_graph::envelopes::EnvAsr<f64>;
pub type Envelope = knaster_graph::envelopes::Envelope<f64>;

pub type Pan2 = knaster_graph::pan::Pan2<f64>;

pub type WhiteNoise = knaster_graph::noise::WhiteNoise<f64>;
pub type PinkNoise = knaster_graph::noise::PinkNoise<f64>;
pub type BrownNoise = knaster_graph::noise::BrownNoise<f64>;

pub type PolyBlep = knaster_graph::polyblep::PolyBlep<f64>;

pub type SvfFilter = knaster_graph::svf::SvfFilter<f64>;

pub type OnePoleLpf = knaster_graph::onepole::OnePoleLpf<f64>;
pub type OnePoleHpf = knaster_graph::onepole::OnePoleHpf<f64>;

pub type AllpassDelay = knaster_graph::delay::AllpassDelay<f64>;
pub type AllpassFeedbackDelay = knaster_graph::delay::AllpassFeedbackDelay<f64>;

pub type DoneOnTrig = knaster_graph::util::DoneOnTrig<f64>;
