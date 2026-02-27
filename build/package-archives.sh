#!/usr/bin/env bash
#
# Strip debug symbols from .a files and package them into a tar.zst archive.
#
# Usage: ./package-archives.sh <lib-dir> <target-triple> <bun-version>
#
# Creates: dist/bun-libs-v<version>-<target>.tar.zst
# Prints:  SHA-256 checksum of the archive
#
set -euo pipefail

if [ $# -ne 3 ]; then
    echo "Usage: $0 <lib-dir> <target-triple> <bun-version>" >&2
    exit 1
fi

LIB_DIR="$1"
TARGET_TRIPLE="$2"
BUN_VERSION="$3"

ARCHIVE_NAME="bun-libs-v${BUN_VERSION}-${TARGET_TRIPLE}.tar.zst"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DIST_DIR="$REPO_ROOT/dist"

mkdir -p "$DIST_DIR"

# Create a temp directory for stripped copies
STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT

echo "Copying .a files from $LIB_DIR to staging..." >&2
cp "$LIB_DIR"/*.a "$STAGING/"

echo "Stripping debug symbols..." >&2
case "$(uname -s)" in
    Darwin)
        strip -S "$STAGING"/*.a
        ;;
    Linux)
        strip --strip-debug "$STAGING"/*.a
        ;;
    *)
        echo "Warning: unsupported OS for stripping, skipping" >&2
        ;;
esac

echo "Creating $ARCHIVE_NAME (zstd level 19)..." >&2
tar -C "$STAGING" -cf - . | zstd -19 -o "$DIST_DIR/$ARCHIVE_NAME"

CHECKSUM="$(shasum -a 256 "$DIST_DIR/$ARCHIVE_NAME" | awk '{print $1}')"

echo "Archive: $DIST_DIR/$ARCHIVE_NAME" >&2
echo "Size:    $(du -h "$DIST_DIR/$ARCHIVE_NAME" | awk '{print $1}')" >&2
echo "$CHECKSUM"
