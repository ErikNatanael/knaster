# Knaster Benchmarks

Contains benchmarks for testing the performance of Knaster, as well as benchmarks for evaluating different solutions for implementation into Knaster.

## Running benchmarks

Before running benchmarks, disable turbo boost

```bash
echo "1" | sudo tee /sys/devices/system/cpu/intel_pstate/no_turbo
```

run benchmarks
```bash
cargo bench
```

restore turbo boost
```bash
echo "0" | sudo tee /sys/devices/system/cpu/intel_pstate/no_turbo
```

cat /sys/devices/system/cpu/intel_pstate/no_turbo