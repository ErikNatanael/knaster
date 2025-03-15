This document contains notes on certain aspects of the implementation(s) in knaster.

# Denormals

Terminology:

- denormals/subnormals
- flush to zero (FTZ)
- denormals are zero (DAZ)

Previously, it was suggested that setting the flag to enable DAZ and FTZ could be done at the start of an application, e.g.
https://gist.github.com/GabrielMajeri/545042ee4f956d5b2141105eb6a505a9
but it's UB so

https://github.com/dimforge/rapier/issues/776

Solution that works, but is apparently UB: https://lib.rs/crates/no_denormals

The suggested solution is to use inline assembly for all the code using FTZ and/or DAZ. There is some movement for a better solution recently:
https://github.com/rust-lang/rust/issues/123123

So FTZ and DAZ are broken in Rust, unless we want to write manual assembly. There's another way to save ourselves from denormals
https://www.earlevel.com/main/2012/12/03/a-note-about-de-normalization/

More solutions:
https://www.musicdsp.org/en/latest/_downloads/d81fc9af8a9fa63332b248772fbb4a54/denormal.pdf

## Suggested implementation(s) of denormal mitigation in knaster

A constant `Float::ANTI_DENORMAL` is defined which can be added in algorithms that may produce denormals.

According to <https://www.earlevel.com/main/2019/04/19/floating-point-denormals/>, the lowest value to avoid denormals is 1e-38, but even much higher value will be inaudible.

### When there is a high-pass filter and/or DC-blocker in the algorithm

- Add the DC value after the filter, possibly both before and after the filter, or
- Add the DC value to the input every N samples, creating an impulse train

### Add the DC value to the first input value in a block

Makes the denormal mitigation change depending on block size, but very cheap.

# Feedback buffers

Buffers holding the output of nodes which have a feedback edge from them need to be available after a graph change where the buffers may have been reassigned.

Two ways of doing this:

1. Permanent buffers in separate allocations for feedback nodes, incl. bookkeeping for returning them when no longer needed and keeping them alive when the Graph is dropped.
2. Copying feedback buffer content whenever new task data is applied and buffers have been reassigned. This is simpler, and ensures that the buffer data is as close as possible in memory, but also incurrs the penalty of copying data on the audio thread.
