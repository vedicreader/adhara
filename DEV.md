# Development

## Commands

```sh
pip install -e '.[dev]'        # builds the extension with maturin
cargo test --release && pytest -q
scripts/xcframework.sh && swift test     # macOS only
python bench/bench_tok.py; python bench/bench_hybrid.py
```

`src/tables.rs` comes from `python scripts/gen_tables.py` (needs litesearch and apsw). Run it again when sanskrit.py's tables or apsw's Unicode version change.

## Versioning

The version lives in `Cargo.toml` (`[package].version`). Python reads it through `dynamic = ["version"]`, and `adhara.__version__` comes from `CARGO_PKG_VERSION`. `ship-bump` moves it.

## Build profiles

`release` is for tests and benchmarks. `dist` is `release` stripped; CI builds wheels and the xcframework with it.

## Release

1. Run the tests.
2. Check the version in `Cargo.toml`.
3. Run `ship-release` (fastship). It pushes a `v<version>` tag and bumps the version.

The tag runs CI: tests, the Swift job, abi3 wheels for Linux (x86_64, arm64) and macOS (arm64, x86_64), an sdist, a GitHub release, and PyPI through trusted publishing. Set up the trusted publisher for `adhara` on PyPI first.
