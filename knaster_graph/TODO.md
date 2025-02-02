# knaster graph todos

- [X] Wrapper that removed self (around node or graph) by setting an atomic flag in Graph and also clearing the output buffer.
- [X] Tests for self freeing in various parameter wrappers that mess with BlockAudioCtx.
- [X] Envelope, Delay, SVF, one-pole
- [X] Test Gen based arithmetic
- [X] Parameter index type that other thing can implement a trait to convert values to/from. Useful for Done, filter types etc.
- [X] Metacrate knaster which exports everything that's needed
- [/] Prelude
- [X] Rename Gen because it collides with the gen keyword
- [X] Deprecate Connectable
- [ ] Try GraphEdit<'a> Mutex like edit guard for committing changes
- [ ] Test no_std on embedded (Daisy?)
- [ ] Feedback connections
- [x] Merge Source and Sink
- [x] Write an interface to get node_id and channel from a NodeSeries based on the channel requested.


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
