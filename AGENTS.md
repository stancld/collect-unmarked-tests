# Agent Instructions for collect-unmarked-tests

## Commands
- **Build**: `cargo build` (debug), `cargo build --release` (release)
- **Run**: `cargo run` (default), `cargo run -- <dir>` (custom directory)
- **Test**: `cargo test` (all tests), `cargo test <test_name>` (single test)
- **Lint**: `cargo clippy` (if clippy is available)
- **Format**: `cargo fmt`
- **Pre-commit**: `pre-commit run --all-files` (run all pre-commit hooks)

## Architecture
- Single-file Rust CLI tool that scans Python test files
- Uses regex to parse Python AST and detect pytest markers
- Walks directory trees to find `.py` files containing `test_*` functions
- Filters out tests with excluded markers (default: unit, integration, component, skip, slow)
- Exit codes: 0 (no unmarked tests), 1 (unmarked tests found)

## Code Style
- Uses Rust 2024 edition with clap, regex, walkdir dependencies
- Standard Rust naming: snake_case for functions/variables, PascalCase for structs
- Error handling via `Result` types and early returns
- Comments only for complex logic (regex patterns, algorithm explanations)
- Prefer `collect()` over manual loops, use `filter_map()` for Option handling
- Use `#[cfg(test)]` for test modules, comprehensive unit tests for core parsing logic
