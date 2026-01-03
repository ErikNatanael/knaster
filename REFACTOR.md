- [x] Should float params have their own attribute? Not yet, maybe in the future.
- [x] Should float params be required to take Self::Sample? Yes
- [x] Should float params be required take a &AudioCtx? Yes
- [ ] macro: Generate correct float param functions for all float params.

## Smoothing and precise timing

Currently works by intercepting param_apply

### Can we generate inlinable functions that wrap the parameter setters the same way?

--We could if the index was a generic constant.-- No, because the function in the end needs to be non-generic.

```rust
    fn float_param_set_fn<const INDEX: usize>(
        &mut self,
        ctx: &mut AudioCtx,
    ) -> fn(ugen: &mut Self, value: Self::Sample, ctx: &mut AudioCtx) {
        todo!()
    }
```

We could allow closures

```rust
    fn float_param_set_fn<const INDEX: usize>(
        &mut self,
        ctx: &mut AudioCtx,
    ) -> impl Fn(ugen: &mut Self, value: Self::Sample, ctx: &mut AudioCtx) {
        | ugen, value, ctx | {
            ugen.float_param_set_fn(ctx, INDEX).expect("param index out of bounds").call(ugen.ugen, value, ctx);
        }
    }
```
