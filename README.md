# Knaster

Knaster is a real time sound synthesis framework focused on a balance between performance, flexibility and ergonomics.

## Features

- Dynamic graph that can be changed while it's running, similar to SuperCollider.
- `no_std` compatible (knaster_graph requires `alloc`)
- Fast: knaster aims to be as performant as possible on all major platforms. PRs to improve performance are always welcome.
  - Pay the performance cost only for what you use: most features that are only required sometimes are implemented as wrappers around the signal generators that need them.
  - Audio buffers are reused within a Graph.
- Parameters
  - Opt in sample accurate parameter changes, automatically splitting block processing for that UGen.
  - Parameter smoothing of manual parameter changes.
  - Audio rate parameter changes, automatically switching to sample by sample processing for only the destination UGen.

## Example

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

## Project structure

As a user of the framework, you only need to depend on `knaster`, which re-exports the other crates.

Knaster is split into several crates for modularity and to allow for more flexibility.
The crates are defined in the following chain, where each crate imports and re-exports the previous to reduce the risk of dependency version mismatches.

- `knaster_primitives`: Contains basic types that may be useful even to crates that otherwise have little to do with Knaster. Does not need `alloc`.
- `knaster_core`: The core building blocks of the structure of DSP in Knaster, and all of the fundamental DSP that doesn't depend on the internals of the graph. `alloc` is optional, unlocking some DSP and convenient data structures e.g. `VecBlock`.
- `knaster_graph`: The `Graph` and everything that directly relates to it. Requires `alloc`.
- `knaster`: Preludes, convenience functions and re-exports.

Additionally, there are crates which only implement new UGens, and are not required for the core functionality of Knaster. These typically only depend on `knaster_core`.

## Contributions

Any contributions will, unless otherwise explicitly stated, be submitted and licensed under the same license as the crate they pertain to.

## License

Knaster is licensed under MIT + Apache 2.0.
