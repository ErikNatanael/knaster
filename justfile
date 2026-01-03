
test-all:
  cd knaster_primitives && cargo test --no-default-features
  cd knaster_primitives && cargo test --no-default-features --features=alloc
  cd knaster_primitives && cargo test --no-default-features --features=std
  
  cd knaster_core && cargo test --no-default-features
  cd knaster_core && cargo test --no-default-features --features=alloc
  cd knaster_core && cargo test --no-default-features --features=std

  cd knaster_core_dsp && cargo test --no-default-features
  cd knaster_core_dsp && cargo test --no-default-features --features=no_denormals
  cd knaster_core_dsp && cargo test --no-default-features --features=alloc
  cd knaster_core_dsp && cargo test --no-default-features --features=std
  cd knaster_core_dsp && cargo test --no-default-features --features=std,no_denormals
  cd knaster_core_dsp && cargo test --no-default-features --features=std,sound_files

  cd knaster_graph && cargo test --no-default-features --features=alloc
  cd knaster_graph && cargo test --no-default-features --features=std
  cd knaster_graph && cargo test --no-default-features --features=std,sound_files
  cd knaster_graph && cargo test --no-default-features --features=std,sound_files,assert_no_alloc
  cd knaster_graph && cargo test --no-default-features --features=std,cpal,jack
  cd knaster_graph && cargo test --no-default-features --features=std,cpal,jack,sound_files,assert_no_alloc,dot
