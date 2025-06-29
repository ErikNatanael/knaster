//! Graph connection API using compile time type safety, providing a more ergonomic and less error
//! prone interface for hand written code.
//!
//! Since this interface requires types to be known as compile time, it is not as good for use
//! cases where the Graph is liberally changed at runtime.
//!
//! The interface is directional, starting at inputs and moving towards outputs.

use core::ops::Mul;

use knaster_core::{Float, Size, UGen, math::MathUGen, numeric_array::NumericArray, typenum::*};

use crate::{
    graph::Graph,
    graph::NodeOrGraph,
    handle::{Handle, HandleTrait},
};

// We need Sink and Source because some things such as binary op connections can't reasonably be
// have things connected to their inputs

pub trait Sink {
    type Inputs: Size;
    fn iter(&self) -> ChannelIter<Self::Inputs>;
}
pub struct ChannelIter<I: Size> {
    channels: NumericArray<(NodeOrGraph, u16), I>,
    current_index: usize,
}
impl<I: Size> Iterator for ChannelIter<I> {
    type Item = (NodeOrGraph, u16);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index < I::USIZE {
            let o = self.channels[self.current_index];
            self.current_index += 1;
            Some(o)
        } else {
            None
        }
    }
}
pub trait Source {
    type Outputs: Size;
    fn iter(&self) -> ChannelIter<Self::Outputs>;
}

pub trait Connection2GraphTrait<F: Float> {
    /// Connect from graph inputs
    fn from_inputs<I: Size>(&mut self) -> Conn<'_, F, Connection2GraphInputs<I>>;
    fn connect_from<S: Source>(&mut self, source: S) -> Conn<'_, F, S>;

    fn connect_from_new<S: UGen<Sample = F> + 'static>(
        &mut self,
        ugen: S,
    ) -> Conn<'_, F, Handle<S>>;
}
impl<F: Float> Connection2GraphTrait<F> for Graph<F> {
    fn from_inputs<I: Size>(&mut self) -> Conn<'_, F, Connection2GraphInputs<I>> {
        todo!()
    }

    fn connect_from<S: Source>(&mut self, source: S) -> Conn<'_, F, S> {
        Conn {
            graph: self,
            t: source,
        }
    }
    fn connect_from_new<S: UGen<Sample = F> + 'static>(
        &mut self,
        ugen: S,
    ) -> Conn<'_, F, Handle<S>> {
        let handle = self.push(ugen);

        Conn {
            graph: self,
            t: handle,
        }
    }
}

#[derive(Clone)]
pub struct Connection2GraphInputs<I: Size> {
    in_channels: NumericArray<usize, I>,
}

pub struct Conn<'a, F: Float, T> {
    graph: &'a mut Graph<F>,
    t: T,
}
#[derive(Clone)]
pub struct ConnectionNode<I: Size, O: Size> {
    node: NodeOrGraph,
    in_channels: NumericArray<usize, I>,
    out_channels: NumericArray<usize, O>,
}
impl<T: UGen> From<Handle<T>> for ConnectionNode<T::Inputs, T::Outputs> {
    fn from(handle: Handle<T>) -> Self {
        let node = NodeOrGraph::Node(handle.node_id());
        Self::from_node(node)
    }
}
impl<T: UGen> Source for Handle<T> {
    type Outputs = T::Outputs;

    fn iter(&self) -> ChannelIter<Self::Outputs> {
        let node = NodeOrGraph::Node(self.node_id());
        Source::iter(&ConnectionNode::<T::Inputs, T::Outputs>::from_node(node))
    }
}
impl<T: UGen> Sink for Handle<T> {
    type Inputs = T::Inputs;

    fn iter(&self) -> ChannelIter<Self::Inputs> {
        let node = NodeOrGraph::Node(self.node_id());
        Sink::iter(&ConnectionNode::<T::Inputs, T::Outputs>::from_node(node))
    }
}

impl<I: Size, O: Size> ConnectionNode<I, O> {
    pub fn from_node(node: NodeOrGraph) -> Self {
        let mut in_channels = NumericArray::default();
        for i in 0..I::USIZE {
            in_channels[i] = i;
        }
        let mut out_channels = NumericArray::default();
        for i in 0..O::USIZE {
            out_channels[i] = i;
        }
        Self {
            node,
            in_channels,
            out_channels,
        }
    }
}
impl<I: Size, O: Size> Source for ConnectionNode<I, O> {
    type Outputs = O;

    fn iter(&self) -> ChannelIter<Self::Outputs> {
        ChannelIter {
            channels: self
                .out_channels
                .iter()
                .map(|c| (self.node, *c as u16))
                .collect(),
            current_index: 0,
        }
    }
}
impl<I: Size, O: Size> Sink for ConnectionNode<I, O> {
    type Inputs = I;

    fn iter(&self) -> ChannelIter<Self::Inputs> {
        ChannelIter {
            channels: self
                .out_channels
                .iter()
                .map(|c| (self.node, *c as u16))
                .collect(),
            current_index: 0,
        }
    }
}
impl<F: Float, T> Conn<'_, F, T> {
    pub fn inner(self) -> T {
        self.t
    }
}
impl<'a, F: Float, T: Source> Conn<'a, F, T> {
    pub fn to<S: Sink<Inputs = T::Outputs>>(self, sink: S) -> Conn<'a, F, S> {
        for ((source, source_channel), (sink, sink_channel)) in self.t.iter().zip(sink.iter()) {
            self.graph
                .connect2(source, source_channel, sink_channel, sink)
                .expect("type safe interface should eliminate graph connection errors");
        }
        Conn {
            graph: self.graph,
            t: sink,
        }
    }
    /// Connect to a new node, returning both a `Conn` for the [`Handle`] to the new node. The
    /// [`Handle`] can be extracted with [`Conn::handle`].
    pub fn to_new<S: UGen<Sample = F, Inputs = T::Outputs> + 'static>(
        self,
        sink: S,
    ) -> Conn<'a, F, Handle<S>> {
        let handle = self.graph.push(sink);
        let ugen = NodeOrGraph::Node(handle.node_id());
        let sink = ConnectionNode::<S::Inputs, S::Outputs>::from_node(ugen);

        for ((source, source_channel), (sink, sink_channel)) in self.t.iter().zip(Sink::iter(&sink))
        {
            self.graph
                .connect2(source, source_channel, sink_channel, sink)
                .expect("type safe interface should eliminate graph connection errors");
        }
        Conn {
            graph: self.graph,
            t: handle,
        }
    }
    pub fn mul_new<S: UGen<Sample = F, Outputs = T::Outputs> + 'static>(
        self,
        sink: S,
    ) -> Conn<'a, F, BinaryOpNodes<T::Outputs>>
    where
        T::Outputs: Same<S::Outputs>,
    {
        let handle = self.graph.push(sink);
        let ugen = NodeOrGraph::Node(handle.node_id());
        let sink = ConnectionNode::<S::Inputs, S::Outputs>::from_node(ugen);
        self * sink
    }
    pub fn to_graph_out<Outputs: Size, Channels: Into<NumericArray<usize, Outputs>>>(
        self,
        channels: Channels,
    ) {
        todo!()
    }
}
impl<F: Float, T: UGen<Sample = F>> Conn<'_, F, Handle<T>> {
    pub fn handle(&self) -> Handle<T> {
        self.t.clone()
    }
}
#[derive(Clone)]
pub struct BinaryOpNodes<O: Size> {
    out_channels: NumericArray<(NodeOrGraph, u16), O>,
}
// Copy workaround, see the `ArrayLength` docs for more info.
impl<O: Size> Copy for BinaryOpNodes<O> where
    <O as knaster_core::numeric_array::ArrayLength>::ArrayType<(NodeOrGraph, u16)>:
        core::marker::Copy
{
}
impl<O: Size> Source for BinaryOpNodes<O> {
    type Outputs = O;

    fn iter(&self) -> ChannelIter<Self::Outputs> {
        ChannelIter {
            channels: self.out_channels.clone(),
            current_index: 0,
        }
    }
}

impl<'a, F: Float, T: Source, T2: Source> Mul<T2> for Conn<'a, F, T>
where
    T::Outputs: Same<T2::Outputs>,
{
    type Output = Conn<'a, F, BinaryOpNodes<T::Outputs>>;

    fn mul(self, rhs: T2) -> Self::Output {
        let mut channels = NumericArray::default();
        for (i, ((source, source_channel), (source2, source_channel2))) in
            self.t.iter().zip(rhs.iter()).enumerate()
        {
            let mul = NodeOrGraph::Node(
                self.graph
                    .push(MathUGen::<F, U2, knaster_core::math::Mul>::new())
                    .node_id(),
            );
            self.graph
                .connect2(source, source_channel, 0, mul)
                .expect("type safe interface should eliminate graph connection errors");
            self.graph
                .connect2(source2, source_channel2, 1, mul)
                .expect("type safe interface should eliminate graph connection errors");
            channels[i] = (mul, i as u16);
        }
        Conn {
            graph: self.graph,
            t: BinaryOpNodes {
                out_channels: channels,
            },
        }
    }
}
impl<'a, F: Float, T: Source, T2: Source> core::ops::Add<T2> for Conn<'a, F, T>
where
    T::Outputs: Same<T2::Outputs>,
{
    type Output = Conn<'a, F, BinaryOpNodes<T::Outputs>>;

    fn add(self, rhs: T2) -> Self::Output {
        let mut channels = NumericArray::default();
        for (i, ((source, source_channel), (source2, source_channel2))) in
            self.t.iter().zip(rhs.iter()).enumerate()
        {
            let add = NodeOrGraph::Node(
                self.graph
                    .push_internal(MathUGen::<F, U2, knaster_core::math::Add>::new())
                    .node_id(),
            );
            self.graph
                .connect2(source, source_channel, 0, add)
                .expect("type safe interface should eliminate graph connection errors");
            self.graph
                .connect2(source2, source_channel2, 1, add)
                .expect("type safe interface should eliminate graph connection errors");
            channels[i] = (add, i as u16);
        }
        Conn {
            graph: self.graph,
            t: BinaryOpNodes {
                out_channels: channels,
            },
        }
    }
}

pub enum Op {
    Mul,
    Add,
}

// #[cfg(test)]
// mod tests {
//     use core::ops::Mul;
//
//     use crate::{
//         connectable2::Connection2GraphTrait,
//         runner::{Runner, RunnerOptions},
//     };
//     use knaster_core::{
//         noise::WhiteNoise, onepole::OnePoleLpf, osc::SinWt, pan::Pan2, typenum::*, util::Constant,
//         wrappers_core::UGenWrapperCoreExt,
//     };
//
//     #[test]
//     fn connectable2() {
//         let block_size = 16;
//         let (mut graph, mut runner) = Runner::<f32>::new::<U0, U2>(RunnerOptions {
//             block_size,
//             sample_rate: 48000,
//             ring_buffer_size: 50,
//         });
//         let sine = graph.push(SinWt::new(200.));
//         let c = graph
//             .connect_from(sine)
//             .mul_new(SinWt::new(3.))
//             .to_new(Pan2::new(0.0));
//         let pan_handle = c.handle();
//         c.to_graph_out([0, 1]);
//         graph.commit_changes().unwrap();
//
//         // Remake the following into something easier to read:
//         /*
//         let exciter_amp = g.push(Constant::new(0.5));
//         let exciter = g.push(HalfSineWt::new(2000.).wr_mul(0.1));
//         let noise_mix = g.push(Constant::new(0.25));
//         let noise = g.push(WhiteNoise::new());
//         let exciter_lpf = g.push(OnePoleLpf::new(2600.));
//         let en = ugen_mul(&exciter, &exciter_amp, g)?;
//         let en2 = ugen_mul(&noise, &noise_mix, g)?;
//         let en3 = ugen_mul(&en, &en2, g)?;
//         let add = ugen_add(&en, &en3, g)?;
//         g.connect(&add, 0, 0, &exciter_lpf)?;
//         */
//         // Could nodes be named inline, e.g. with a `name("exciter_amp")`. We need named nodes for
//         // two reasons:
//         // 1 connecting between them, audio and parameters
//         // 2 manually changing parameter values later
//         //
//         // For 1, the connections can almost always be made in a chain with some parallel tracks.
//         // Sometimes we need to add nodes later, especially whole sub-graphs going into effects.
//         //
//         // For 2, this would be neater in a larger Synth like interface since there is no type
//         // safety for parameters anyway.
//         //
//         // graph.edit().push(Constant::new(0.5)).name("exciter_amp_node").store_param(0, "exciter_amp");
//         // graph.edit().push(Constant::new(0.5)).name("exciter_amp_node").store_param(0, "exciter_amp");
//         // let exciter_amp = graph.node("exciter_amp_node")?;
//         // graph.edit().connect(exciter_amp).to(SinWt::new(2000.)).name("sine").store_param("freq",
//         // "freq");
//         // graph.set("exciter_amp")?.value(0.2).smoothing(Linear(0.5));
//         // graph.set("freq")?.value().smoothing(Linear(0.5));
//
//         let exciter_amp = graph.push(Constant::new(0.5));
//         let noise_mix = graph.push(Constant::new(0.25));
//         let exc = (graph.connect_from_new(SinWt::new(2000.).wr_mul(0.1)) * exciter_amp).inner();
//
//         let noise = graph.connect_from_new(WhiteNoise::new()) * noise_mix;
//         let c = ((noise * exc) + exc).to_new(OnePoleLpf::new(2600.));
//         let lpf = c.handle();
//         let c = c.to_new(Pan2::new(0.0));
//         let pan = c.handle();
//         c.to_graph_out([0, 1]);
//
//         graph.commit_changes().unwrap();
//         assert_eq!(graph.inspection().nodes.len(), 1);
//         for _ in 0..10 {
//             unsafe {
//                 runner.run(&[]);
//             }
//         }
//     }
// }
