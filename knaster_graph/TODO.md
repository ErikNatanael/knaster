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
