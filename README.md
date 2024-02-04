libyaml-safer
==============

[<img alt="github" src="https://img.shields.io/badge/github-simonask/libyaml--safer-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/simonask/libyaml-safer)
[<img alt="crates.io" src="https://img.shields.io/crates/v/libyaml-safer.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/libyaml-safer)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-libyaml--safer-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/libyaml-safer)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/simonask/libyaml-safer/ci.yml?branch=master&style=for-the-badge" height="20">](https://github.com/simonask/libyaml-safer/actions?query=branch%3Amaster)

This library is a fork of [unsafe-libyaml] translated to safe and idiomatic Rust.

[unsafe-libyaml] is [libyaml] translated from C to unsafe Rust with the
assistance of [c2rust].

[unsafe-libyaml]: https://github.com/dtolnay/unsafe-libyaml
[libyaml]: https://github.com/yaml/libyaml/tree/2c891fc7a770e8ba2fec34fc6b545c672beb37e6
[c2rust]: https://github.com/immunant/c2rust

```toml
[dependencies]
libyaml-safer = "0.1"
```

*Compiler support: requires rustc 1.70*

## Notes

This library uses the same test suite as unsafe-libyaml, which is also the
"official" test suite for libyaml. The library was ported line by line, function
by function, from unsafe-libyaml, with the aim of precisely matching its
behavior, including performance and allocation patterns. Any observable
difference in behavior, outside of API differences due to Rust conventions, is
considered a bug.

One notable exception to the above is that this library uses the Rust standard
library in place of custom routines where possible. For example, most UTF-8 and
UTF-16 encoding and decoding is handled by the standard library, and
input/output callbacks are replaced with the applicable `std::io::*` traits. Due
to the use of `std::io`, this library cannot currently be `no_std`.

Memory allocation patterns are generally preserved, except that standard library
containers may overallocate buffers using different heuristics.

In places where libyaml routines are replaced by the standard library, certain
errors may be reported with reduced fidelity compared with libyaml (e.g., error
messages may look slightly different), but the same inputs should generate the
same general errors.

This library introduces no new dependencies except for
[`thiserror`](https://docs.rs/thiserror) for convenience. This dependency may go
away in the future.

### Compatibility and interoperability

While this library matches the behavior of libyaml, it is not intended as a
drop-in replacement. The shape of the API is idiomatic Rust, and while it is
possible to emulate the C API using this library, supporting this use case is
not a priority. Use `unsafe-libyaml` if that is what you need.

### Performance

Performance is largely on par with `unsafe-libyaml`. No significant effort has
been put into optimizing this library, beyond just choosing the most
straightforward ways to reasonably port concepts from the C-like code.

See
[`benches/bench.rs`](https://github.com/simonask/libyaml-safer/benches/bench.rs)
for a very simple benchmark dealing with a very large (~700 KiB) YAML document.
On my machine (Ryzen 9 3950X) the parser from this library is slightly slower
and the emitter is slightly faster, but both within about ~1ms of their unsafe
counterparts. Run `cargo bench` to test on your machine.

If there is demand, there are clear paths forward to optimize the parser. For
example, due to it being ported directly from unsafe C-like code doing pointer
arithmetic, it performs a completely unreasonable number of bounds checks for
each input byte.

## License

<a href="LICENSE-MIT">MIT license</a>, same as unsafe-libyaml and libyaml.
