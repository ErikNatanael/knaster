# Knaster

Knaster is a real time sound synthesis framework focused on a balance between performance, flexibility and ergonomics.

## Features

- Creative coding friendly framework for audio synthesis in Rust.
- Dynamic graph that can be changed while it's running, similar to SuperCollider.
- `no_std` compatible (knaster_graph requires `alloc`)
- Fast: Knaster aims to be as performant as possible on all major platforms. PRs to improve performance are always welcome.
  - Pay the performance cost only for what you use: most features that are only required sometimes are implemented as wrappers around the signal generators that need them.
  - Audio buffers are reused within a Graph.
- Parameters
  - Opt in sample accurate parameter changes, automatically splitting block processing for that UGen.
  - Parameter smoothing of manual parameter changes.
  - Audio rate parameter changes, automatically switching to sample by sample processing for only the destination UGen.
- Support for multiple backends, currently CPAL and JACK, and non-realtime processing.
- f32 or f64 audio sample type.
- Block size and sample rate agnostic.

## Goals

- Automatic GUI for editing live graphs (in progress).
- CLAP host support to use CLAP plugins as UGens in Knaster.
- Opt-in multi-threaded parallel processing.
- Static graph using the same UGen trait from `knaster_core`.

## Example

### Play a sine wave

The following example starts a stereo audio graph with the default backend, adds a simple sine wave with a constant amplitude of 0.2, and plays it in stereo for 2 seconds.

```rust
use knaster::preludef32::*;
fn main() {
    // Start a new stereo audio graph with the default backend
    let mut graph = knaster::knaster().start()?;
    graph.edit(|g| {
        // Create a sine wave with a frequency of 440 Hz
        let sine = g.push(SinWt::new(440.0));
        // Multiply the sine wave by a constant amplitude of 0.2
        let sig = sine * 0.2;
        // Send the same sine wave signal to the left and right channels of the output
        sig.out([0, 0]).to_graph_out();
    });
    // Wait for 2 seconds
    std::thread::sleep(std::time::Duration::from_secs(2));
    Ok(())
}
```

For more examples, see `knaster/examples`.

### Implement a custom UGen

The following is a simplified version of Knaster's `SinNumeric` UGen, which is used to generate a sine wave with a given frequency.

```rust
pub struct SinNumeric<F> {
    phase: F,
    phase_offset: F,
    phase_increment: F,
}

#[impl_ugen]
impl<F: Float> SinNumeric<F> {
    pub fn new(freq: F) -> Self {
        Self {
            phase: freq,
            phase_offset: F::ZERO,
            phase_increment: F::ZERO,
        }
    }
    // The param attribute marks the function as a parameter setter,
    // creating a parameter with the same name as the function.
    #[param]
    pub fn freq(&mut self, freq: PFloat, ctx: &AudioCtx) {
        self.phase_increment = F::new(freq) / F::new(ctx.sample_rate() as f32);
    }
    #[param]
    pub fn phase_offset(&mut self, phase_offset: PFloat) {
        self.phase_offset = F::new(phase_offset);
    }
    #[param]
    pub fn reset_phase(&mut self) {
        self.phase = F::ZERO;
    }
    // `process` is a frame by frame processing function. The number of output samples determines the
    // number of output channels that this UGen has. When relevant for performance, the `process_block`
    // function can be used to process an entire block or sub-block at once.
    pub fn process(&mut self) -> [F; 1] {
        let out = ((self.phase + self.phase_offset) * F::TAU).sin();
        self.phase += self.phase_increment;
        if self.phase > F::ONE {
            self.phase -= F::ONE;
        }
        [out]
    }
}
```

## Project structure

As a user of the framework, you only need to depend on `knaster`, which re-exports the other crates.

Knaster is split into several crates for modularity and to allow for more flexibility.
The crates are defined in the following chain, where each crate imports and re-exports the previous to reduce the risk of dependency version mismatches.

- `knaster_primitives`: Contains basic types that may be useful even to crates that otherwise have little to do with Knaster. Does not need `alloc`.
- `knaster_core`: The core building blocks of the structure of DSP in Knaster, and all of the fundamental DSP that doesn't depend on the internals of the graph. `alloc` is optional, unlocking some DSP and convenient data structures e.g. `VecBlock`.
- `knaster_graph`: The `Graph` and everything that directly relates to it. Requires `alloc`.
- `knaster`: Preludes, convenience functions and re-exports.

Additionally, there are crates which only implement new UGens, and are not required for the core functionality of Knaster. These typically only depend on `knaster_core`.

## History

Knaster is the result of a complete rewrite of [Knyst](https://github.com/ErikNatanael/knyst).

## Contributions

Any contributions will, unless otherwise explicitly stated, be submitted and licensed under the same license as the crate they pertain to.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Knaster by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
