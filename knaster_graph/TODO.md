# knaster graph todos

- [x] Wrapper that removed self (around node or graph) by setting an atomic flag in Graph and also clearing the output buffer.
- [x] Tests for self freeing in various parameter wrappers that mess with BlockAudioCtx.
- [x] Envelope, Delay, SVF, one-pole
- [x] Test Gen based arithmetic
- [x] Parameter index type that other thing can implement a trait to convert values to/from. Useful for Done, filter types etc.
- [x] Metacrate knaster which exports everything that's needed
- [/] Prelude
- [x] Rename Gen because it collides with the gen keyword
- [x] Deprecate Connectable
- [x] Try GraphEdit<'a> Mutex like edit guard for committing changes
- [ ] Test no_std on embedded (Daisy?)
- [x] Feedback connections
- [x] Merge Source and Sink
- [x] Write an interface to get node_id and channel from a NodeSeries based on the channel requested.
- [x] Make Inputs and Outputs u16 everywhere to save memory?
- [x] Removing edges from a Graph
- [ ] Special case float parameters from other parameters
- [ ] Deprecate Graph node edits in favour of GraphEdit
- [x] macro: parameter ranges
- [ ] macro: Move all UGen impls to use macro
- [ ] Control rate UGens
- [ ] UGen -> parameter change
- [ ] Parameter change chains
- [ ] Any Parameter value

UGen
UnitGenerator
Processor
SignalProcessor
SigProc
Generator
Operator
Opcode
Object
Signal
Flow
FlowUnit

Operator (FM synthesis)
Processor

## connection API

### Functions on Graph

```rust
    fn connect_nodes_internal(
        &mut self,
        source: impl Into<NodeId>,
        sink: impl Into<NodeId>,
        source_channel: u16,
        sink_channel: u16,
        additive: bool,
        feedback: bool,
    ) -> Result<(), GraphError> {
    fn connect_input_to_output(
        &mut self,
        source_channel: u16,
        sink_channel: u16,
        additive: bool,
    ) -> Result<(), GraphError> {
    fn connect_node_to_output(
        &mut self,
        source: impl Into<NodeId>,
        source_channel: u16,
        sink_channel: u16,
        additive: bool,
    ) -> Result<(), GraphError> {
    pub fn connect_to_parameter(
        &mut self,
        source: impl Into<NodeId>,
        source_channel: u16,
        parameter: impl Into<Param>,
        sink: impl Into<NodeId>,
    ) -> Result<(), GraphError> {
    pub fn connect_replace_to_parameter(
        &mut self,
        source: impl Into<NodeId>,
        source_channel: u16,
        parameter: impl Into<Param>,
        sink: impl Into<NodeId>,
    ) -> Result<(), GraphError> {
    fn connect_node_to_parameter(
        &mut self,
        source: impl Into<NodeId>,
        source_channel: u16,
        parameter: impl Into<Param>,
        sink: impl Into<NodeId>,
        additive: bool,
    ) -> Result<(), GraphError> {
    fn connect_input_to_node(
        &mut self,
        sink: impl Into<NodeId>,
        source_channel: u16,
        sink_channel: u16,
        additive: bool,
    ) -> Result<(), GraphError> {
    fn connect_to_node_internal(
        &mut self,
        mut source: NodeKeyOrGraph,
        sink: NodeKey,
        so_channel: u16,
        si_channel: u16,
        additive: bool,
        feedback: bool,
    ) {
    fn connect_to_output_internal(
        &mut self,
        source: NodeKeyOrGraph,
        so_channel: u16,
        si_channel: u16,
        additive: bool,
    ) {
    // fn connect<N: Size>(
    //     &mut self,
    //     source: impl Into<Connectable>,
    //     source_channels: impl Into<Channels<N>>,
    //     sink_channels: impl Into<Channels<N>>,
    //     sink: impl Into<Connectable>,
    // ) -> Result<(), GraphError> {
    pub fn connect2(
        &mut self,
        source: impl Into<NodeOrGraph>,
        source_channel: u16,
        sink_channel: u16,
        sink: impl Into<NodeOrGraph>,
    ) -> Result<(), GraphError> {
    pub fn connect2_replace(
        &mut self,
        source: NodeOrGraph,
        source_channel: u16,
        sink_channel: u16,
        sink: NodeOrGraph,
    ) -> Result<(), GraphError> {
    pub fn connect2_feedback(
        &mut self,
        source: NodeOrGraph,
        source_channel: u16,
        sink_channel: u16,
        sink: NodeOrGraph,
    ) -> Result<(), GraphError> {
    pub fn connect2_feedback_replace(
        &mut self,
        source: NodeOrGraph,
        source_channel: u16,
        sink_channel: u16,
        sink: NodeOrGraph,
    ) -> Result<(), GraphError> {

```

Sources:

- graph input channel
- node output channel

Sinks:

- graph output channel
- node input channel
- node float parameter

Connection options:

- replace (default: add)
- feedback (default: false, graph input cannot be connected via feedback)

Simplification:

```rust
    pub fn connect2(
        &mut self,
        source: impl Into<NodeOrGraph>,
        source_channel: u16,
        sink_channel: u16,
        sink: impl Into<NodeOrGraph>,
        flags: impl Into<ConnectionFlags>,
    ) -> Result<(), GraphError>;
    // OR
    pub fn connect2(
        &mut self,
        source: impl Into<NodeOrGraphChannel>,
        sink: impl Into<NodeOrGraphOrParamChannel>,
        flags: impl Into<ConnectionFlags>,
    ) -> Result<(), GraphError>;
    graph.connect2(source.out(0), sine.in_param("freq"), ConnectionFlags::default());
    graph.connect2(Source::node(source, 0), Sink::Graph(0), ConnectionFlags::default());
    graph.connect2(Source::node(source, 1), Sink::node(reverb, 0), ConnectionFlags::default());
    graph.connect2(Source::node(source, 1), Sink::node(reverb, 0), ConnectionOptions::default());
    graph.connect2(Source::graph(1), Sink::param(reverb, "mix"), ConnectionOptions::default());
    graph.connect2(Source::node(source, 1), Sink::node(reverb, 0), None);
    graph.connect2().from(Source::node(source, 1)).to(Sink::node(reverb, 0)).call();
```
