# Development

## Commands

```sh
pip install -e '.[dev]'
cargo test --release && pytest
scripts/xcframework.sh && swift test
python bench/bench_tok.py; python bench/bench_hybrid.py
```

`pip install -e '.[dev]'` builds the extension with maturin. The Swift commands run on macOS only.

`python scripts/gen_tables.py` writes `src/tables.rs`. The script needs litesearch and apsw. Run it again when the tables of `sanskrit.py` change, or when apsw moves to a new Unicode version.

## Versioning

The version is in `Cargo.toml`, in `[package].version`. Python reads it through `dynamic = ["version"]`. `adhara.__version__` comes from `CARGO_PKG_VERSION`. `ship-bump` changes the version.

## Build profiles

The `release` profile is for tests and benchmarks. The `dist` profile is `release` with the symbols stripped. CI builds the wheels with `dist`. `scripts/xcframework.sh` builds the xcframework with `dist`.

## Release

The repository needs the secret `PYPI_TOKEN`. CI uploads the wheels to PyPI with it.

1. Run the tests.
2. Check the version in `Cargo.toml`.
3. Run `scripts/swift_release.sh`. It builds `target/AdharaCore.xcframework.zip`. It writes the URL and the checksum of the zip into `Package.swift`.
4. Commit `Package.swift`.
5. Run `ship-release` (fastship). It pushes the tag `v<version>`, then bumps the version and pushes that commit.
6. Run `gh release create v<version> --verify-tag --title v<version> target/AdharaCore.xcframework.zip`. If CI made the release first, run `gh release upload v<version> target/AdharaCore.xcframework.zip`.

The tag starts CI. CI runs the tests and the Swift tests. It builds abi3 wheels for Linux (x86_64, arm64) and macOS (arm64, x86_64), and an sdist. It adds them to the GitHub release and uploads them to PyPI.

The zip in the release must be the file from step 3. A new build has a different checksum, and SwiftPM refuses it.
