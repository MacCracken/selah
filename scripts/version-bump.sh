#!/usr/bin/env bash
set -euo pipefail

# Usage: ./scripts/version-bump.sh <new-version>
# Example: ./scripts/version-bump.sh 0.25.0

NEW_VERSION="${1:?Usage: version-bump.sh <new-version>}"

cd "$(git rev-parse --show-toplevel)"

# Update Cargo.toml
sed -i "s/^version = \".*\"/version = \"${NEW_VERSION}\"/" Cargo.toml

# Update VERSION file
printf '%s' "${NEW_VERSION}" > VERSION

# Update Cargo.lock
cargo check --quiet 2>/dev/null || true

echo "Bumped to ${NEW_VERSION}"
echo "Don't forget to update CHANGELOG.md"
