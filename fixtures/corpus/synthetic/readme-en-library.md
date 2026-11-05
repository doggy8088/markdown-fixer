<p align="center">
  <img src="docs/logo.svg" width="120" alt="tinyqueue logo">
</p>

<h1 align="center">tinyqueue</h1>

[![CI](https://github.com/example/tinyqueue/actions/workflows/ci.yml/badge.svg)](https://github.com/example/tinyqueue/actions)
[![crates.io](https://img.shields.io/crates/v/tinyqueue.svg)](https://crates.io/crates/tinyqueue)
[![docs.rs](https://docs.rs/tinyqueue/badge.svg)](https://docs.rs/tinyqueue)
![](https://img.shields.io/badge/license-MIT-blue.svg)

A tiny, lock-free, bounded MPMC queue for Rust.
## Features

- **Lock-free** push and pop using atomics
- **Bounded** capacity, decided at construction time
- `no_std` support behind the `std` feature flag
- Zero dependencies

## Installation

```toml
[dependencies]
tinyqueue = "0.4"
```

## Usage

```rust
use tinyqueue::Queue;

let q = Queue::with_capacity(8);
q.push(1).unwrap();
assert_eq!(q.pop(), Some(1));
```

### Benchmarks

| Queue | push (ns) | pop (ns) |
|-------|----------:|---------:|
| tinyqueue | 12.1 | 11.8 |
| crossbeam ArrayQueue | 13.4 | 12.9 |
| std Mutex<VecDeque> | 41.0 | 39.7 |

Run them yourself with `cargo bench`. Results above are from an M3 Pro, see [the benchmark notes] (docs/bench.md).

## FAQ

### Is it really lock-free?
Yes. Every operation is a bounded number of CAS loops. See the [design doc][design].

### Why not just use crossbeam?
You probably should! tinyqueue exists for `no_std` targets where crossbeam is too heavy.

## Contributing

1. Fork it
2. Create your feature branch (`git checkout -b feature/foo`)
3. Commit your changes (`git commit -am 'Add some foo'`)
4. Push to the branch (`git push origin feature/foo`)
5. Create a new Pull Request

Please read CONTRIBUTING.md first.

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

[design]: docs/design.md
