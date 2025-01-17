# knaster_primitives

Fundamental types used in Knaster, potentially useful for other crates that don't otherwise need to depend on `knaster_core` or `knaster_graph`.

`knaster_primitives` is `no_std` by default. Enabling the `std` feature and disabling the `libm` feature by using `default-features = false` may give better performance when using `Float` on some platforms. 

# License

Licensed under either of

- Apache License, Version 2.0 (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
