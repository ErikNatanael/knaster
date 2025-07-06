# knaster_graph

This crate contains everything directly related to the Knaster `Graph` structure and reexports `knaster_core` which reexports `knaster_primitives`.

knaster_graph is `no_std` by default, but requires `alloc` and therefore a global allocator to be available. If the `std` feature is not enabled, the `alloc` feature is required. Failure to enable either of these features will result in obscure compile errors.

See the `knaster` crate for more details.

# License

Licensed under either of

- Apache License, Version 2.0 (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

