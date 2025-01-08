# knaster graph todos

- [X] Wrapper that removed self (around node or graph) by setting an atomic flag in Graph and also clearing the output buffer.
- [X] Tests for self freeing in various parameter wrappers that mess with BlockAudioCtx.
- [X] Envelope, Delay, SVF, one-pole
- [X] Test Gen based arithmetic
- [X] Parameter index type that other thing can implement a trait to convert values to/from. Useful for Done, filter types etc.
- [ ] Metacrate knaster which exports everything that's needed
- [ ] Prelude
- [ ] Deprecate Connectable
- [ ] Try GraphEdit<'a> Mutex like edit guard for committing changes
