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

# [X] Feedback buffers

Buffers holding the output of nodes which have a feedback edge from them need to be available after a graph change where the buffers may have been reassigned.

Two ways of doing this:

1. Permanent buffers in separate allocations for feedback nodes, incl. bookkeeping for returning them when no longer needed and keeping them alive when the Graph is dropped.
2. Copying feedback buffer content whenever new task data is applied and buffers have been reassigned. This is simpler, and ensures that the buffer data is as close as possible in memory, but also incurrs the penalty of copying data on the audio thread.

We start with 2

Just thought of a third way that simplifies the graph: implement feedback by split node buffering. Since feedback is unusual this is probably good enough in terms of performance.
Two UGens are created, A and B, each holding a pointer to the same allocation. Until the data is overwritten from the input of B, the output of A gives the last block's data.
This requires copying a block of data every block, but since blocks for anything using feedback would ideally be small this isn't a big issue.

# [ ] Error reporting

- [ ] macros for warnings and errors, maybe different for control and audio thread.
- [ ] Many/most errors don't need Result, but can be logged instead. We never want to crash.
- [ ] Feature for enabling backtraces
- [ ] Feature for panic on error
- [ ] Remove Result for change(), connect(), send()

Parameter changes don't need to panic, especially if a backtrace can report the line number of the error. We never want our program to crash and parameter name/channel index/parameter type errors are not recoverable without human intervention so the benefits of Result are small.

## Non real-time logging

Use log and the existing ecosystem.

## Audio thread logging

Each thread in the real-time processing thread pool requires a RingBuffer for logging. Since we can't allocate, a log message consists of several messages of type
&'static str, f32, f64, and End.

### Providing the logger

- Move block info into its own struct in AudioCtx so that block info can be stored and replaced easily without duplicating the &mut.
- Provide a separate logger argument to the methods.treesitter.lua:177: attempt to call method 'range' (a nil value)

# [ ] Recording audio output

# [ ] Macro for graph building

# [ ] Parallel processing

- Real time high priority thread pool in Runner to which unprocessed chains with no dependencies can be dispatched.
- Requires buffer allocation changes to limit buffer reuse.

# Custom handle types

Macro for generating a handle type with methods for parameter names.

# Graph building iterations

To overview the development of graph building APIs, we will construct the same graph in different styles. The example Graph should include

- piping one node into another
- multiplication of a multi-channel node by a single channel node
- mixing a signal with a modulated version of itself

### Knyst

Approximately, untested code

```rust
let freq = var(200.);
let cutoff_freq = var(2000.);
let amp = var(0.5);
let sine = sine().freq(sine().freq(freq) * freq + freq);
let reverb = reverb().t(2.6).mix(0.5).left(sin).right(sine);
let lpf_l = lpf().cutoff_freq(cutoff_freq).sig(reverb.out(0)) + sine;
let lpf_r = lpf().cutoff_freq(cutoff_freq).sig(reverb.out(1)) + sine;
graph_output(0, lpf_l * amp);
graph_output(1, lpf_r * amp);

freq.set(0, 300.);
```

### Knaster v1

```rust
let freq = g.push(Constant::new(200.));
let cutoff_freq = g.push(Constant::new(2600.));
let amp = g.push(Constant::new(0.5));
let sine = g.push(SinWt::new(200.));
let sine_mod = g.push(SinWt::new(200.).ar_params());
g.connect_to_parameter(&freq, 0, "freq", &sine_mod)?;
let sine_mod = ugen_mul(&sine_mod, &freq, g);
let sine_mod = ugen_add(&sine_mod, &freq, g);
let reverb = g.push(Reverb::new());
let lpf_r = g.push(OnePoleLpf::new(2600.));
g.connect_to_parameter(&sine_mod, 0, "freq", &sine)?;
g.connect(&sine, [0, 0], [0, 1], &reverb)?;
let mul_rev = ugen_mul(&reverb, &amp.cycle(), g)?;
g.connect(&reverb, [0, 1], [0, 1], g.internal())?;

freq.change(0)?.value(300.);
```

### Knaster v2 (connectable2)

```rust

```

### Knaster v?

With a similar macro as Knyst, many ergonomic functions could be implemented:

- Arithmetic on UGens can automatically be applied as wrappers
- A type safe handle type could be generated with all parameters
- A function could be generated

```rust
let synth = g.subgraph::<U0, U2>([("freq", 200.), ("cutoff_freq", 2000.), ("amp", 0.5)], |e: GraphCreate| {
  let freq = e.var("freq", 200.);
  let cutoff_freq = e.var("cutoff_freq", 2000.);
  let amp = e.var("amp", 0.5);
   let c = e.chain(SinWt::new().ar_params()).set_param("freq", e.push(SinWt::new()) * freq + freq);
  let sine = sin_wt().freq(e.push(sin_wt() * freq + freq).freq(freq));
  let reverb = e.push(reverb()).t(2.6).mix(0.5);
  sine >> reverb;
  let lpf_l = e.push(one_pole_lpf()).cutoff_freq(cutoff_freq).sig(reverb.out(0)) + sine;
  let lpf_r = e.push(one_pole_lpf()).cutoff_freq(cutoff_freq).sig(reverb.out(1)) + sine;
});
synth.set("freq", 300.)?;
```

### Knaster v3 (connectable3)

```rust
graph.edit(|graph| {
let freq = g.push(Constant::new(200.));
let cutoff_freq = g.push(Constant::new(2600.));
let amp = g.push(Constant::new(0.5));
let sine = g.push(SinWt::new(200.));
let sine_mod = g.push(SinWt::new(200.).ar_params());
let reverb = g.push(Reverb::new());
let lpf_l = g.push(OnePoleLpf::new(2600.));
let lpf_r = g.push(OnePoleLpf::new(2600.));

sine_mod.link("freq", freq);
sine.link("freq", sine_mod * freq + freq);
(sine.out([0, 0]) >> reverb >> (lpf_l | lpf_r) * amp.out([0, 0])).to_graph_out();

freq.change(0)?.value(300.);
```
