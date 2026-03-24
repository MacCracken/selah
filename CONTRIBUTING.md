# Contributing to Selah

Thank you for your interest in contributing to Selah!

## Development

1. Fork and clone the repository
2. Ensure you have Rust 1.89+ installed
3. Run `make check` to verify your environment
4. Make your changes
5. Run `make check` before submitting

## Pull Requests

- Keep PRs focused on a single change
- Include tests for new functionality
- Include benchmarks for performance-sensitive changes
- Update CHANGELOG.md
- Ensure `make check` passes (fmt + clippy + test + audit)

## Testing

- Unit tests: inline in the same file (`#[cfg(test)] mod tests { }`)
- Integration tests: `tests/integration.rs`
- Benchmarks: `benches/benchmarks.rs`
- Doc tests: examples in `///` comments
- Target: 80%+ code coverage

## Benchmarks

Run benchmarks with history tracking:

```bash
./scripts/bench-history.sh
```

This appends to `bench-history.csv` and regenerates `benchmarks.md`.
