# Knaster

Knaster is a real time sound synthesis framework focused on a balance between performance, flexibility and ergonomics.

## Features 

- dynamic graph that can be changed while it's running, similar to the SuperCollider structure
- `no_std` compatible (knaster_graph requires `alloc`)

## Project structure

Note: As a user of the framework, you only need to depend on `knaster`

Knaster is split into several crates so as to make parts of it reusable and facilitate experimentation. 
The crates are defined in the following chain, where each crate imports and re-exports the previous to reduce the risk of dependency version mismatches. 

- `knaster_primitives`: Contains basic types that may be useful even to crates that otherwise have little to do with Knaster. Does not need `alloc`.
- `knaster_core`: The core building blocks of the structure of DSP in Knaster, and all of the fundamental DSP that doesn't depend on the internals of the graph. `alloc` is optional, unlocking some DSP and convenient data structures e.g. `VecBlock`.
- `knaster_graph`: The `Graph` and everything that directly relates to it. Requires `alloc`.
- `knaster`: Preludes, convenience functions and re-exports.

Additionally, there are crates which only implement 

## Contributions

Any contributions will, unless otherwise explicitly stated, be submitted and licensed under the same license as the crate they pertain to.

## License

Mostly MIT + Apache 2.0, but some crates are GPL. See each crate for more info.
