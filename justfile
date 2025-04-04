
test-all:
  cd knaster_primitives && cargo test --no-default-features
  cd knaster_primitives && cargo test --no-default-features --features=alloc
  cd knaster_primitives && cargo test --no-default-features --features=std

  cd knaster_core && cargo test --no-default-features
  cd knaster_core && cargo test --no-default-features --features=alloc
  cd knaster_core && cargo test --no-default-features --features=std
  cd knaster_core && cargo test --no-default-features --features=std,sound_files
