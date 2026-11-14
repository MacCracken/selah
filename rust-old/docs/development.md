# Development

## Prerequisites

- Rust 2024 edition (1.85+)
- A running daimon instance (port 8090) for capture features

## Build

```bash
cargo build --workspace
```

## Test

```bash
cargo test --workspace
```

## Lint

```bash
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

## Release

1. Update the version:
   ```bash
   ./bump-version.sh 2026.3.17
   ```
2. Update `CHANGELOG.md` with the new version's changes.
3. Tag and push:
   ```bash
   git tag 2026.3.17
   git push origin main --tags
   ```
4. The release workflow builds amd64 and arm64 binaries and creates a GitHub release.
