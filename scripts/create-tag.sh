#!/bin/bash
# Script to create git tag based on Cargo.toml version
# Usage: ./scripts/create-tag.sh [message]

set -euo pipefail

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Change to project root
cd "$PROJECT_ROOT"

# Extract version from Cargo.toml
VERSION=$(grep '^version = ' Cargo.toml | head -n1 | cut -d'"' -f2)

if [ -z "$VERSION" ]; then
    echo "Error: Could not extract version from Cargo.toml"
    exit 1
fi

TAG="v$VERSION"

# Check if tag already exists
if git rev-parse "$TAG" >/dev/null 2>&1; then
    echo "Error: Tag $TAG already exists"
    echo "Current tags with this version:"
    git tag -l "$TAG"
    exit 1
fi

# Check if working tree is clean
if ! git diff-index --quiet HEAD --; then
    echo "Error: Working tree has uncommitted changes"
    echo "Please commit or stash changes before creating a tag"
    git status --short
    exit 1
fi

# Get the message from argument or use default
if [ $# -ge 1 ]; then
    MESSAGE="$1"
else
    MESSAGE="Release version $VERSION"
fi

# Create the annotated tag
echo "Creating tag $TAG (version $VERSION)..."
git tag -a "$TAG" -m "$MESSAGE"

echo ""
echo "✓ Tag $TAG created successfully!"
echo ""
echo "To push this tag to remote:"
echo "  git push origin $TAG"
echo ""
echo "To push all tags:"
echo "  git push origin --tags"
