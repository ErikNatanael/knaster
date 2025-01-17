# knaster_core

This crate includes the fundamental building blocks of Knaster, such as `Gen`, `Block`, `ParameterValue` etc. It re-exports the `knaster_primitives` crate.

knaster_core is `no_std` by default. Some features require `alloc`, such as `VecBlock` and `Wavetable`. The `unstable` feature unlocks potential optimisations the require the `nightly` compiler at the time of writing.

See the `knaster` crate for more details.

# License

Licensed under either of

- Apache License, Version 2.0 (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.


