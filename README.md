# Collect unmarked pytest

A fast Rust tool to collect Python tests that don't have specific pytest markers.

## Usage

```bash
# Scan default 'tests' directory
cargo run

# Scan specific directory
cargo run -- src

# Exclude specific markers
cargo run -- --exclude-markers unit,integration,component,slow tests
```

## Build

Prerequisites:

- Rust toolchain (stable). Install via <https://rustup.rs>

Debug build:

```bash
cargo build
./target/debug/collect-unmarked-tests --help
```

Release build:

```bash
cargo build --release
./target/release/collect-unmarked-tests --help
```

Install locally (optional):

```bash
cargo install --path .
collect-unmarked-tests --help
```

The last command is equivalent to:

```bash
pytest -m 'not unit and not integration and not component and not slow' tests
```

## Default excluded markers

- `unit`
- `integration`
- `component`
- `skip`
- `slow`

The tool scans Python files for `test_*` functions and excludes those with
pytest markers like `@pytest.mark.unit` or `@skip`.

## Exit codes

- 0: No unmarked tests found
- 1: Unmarked tests found (for CI/CD integration)
