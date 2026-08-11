#!/bin/bash
set -e

NAME="khushu"
VERSION="1.3.2"

TAR_NAME="v$VERSION.tar.gz"
SOURCES_DIR="$HOME/rpmbuild/SOURCES"
PROJECT_ROOT=$(readlink -f "$(dirname "$0")/../../")

echo "Preparing source tarball for RPM build..."
echo "Project Root detected as: $PROJECT_ROOT"

mkdir -p "$SOURCES_DIR"

TEMP_DIR=$(mktemp -d)
PACKAGE_DIR="$TEMP_DIR/$NAME-$VERSION"

echo "Creating clean export of source code..."
mkdir -p "$PACKAGE_DIR"
rsync -a --exclude='target' --exclude='.git' --exclude='build' --exclude='.flatpak-builder' "$PROJECT_ROOT/" "$PACKAGE_DIR/"

echo "Compressing into $TAR_NAME..."
tar -czf "$TEMP_DIR/$TAR_NAME" -C "$TEMP_DIR" "$NAME-$VERSION"

echo "Installing to $SOURCES_DIR/$TAR_NAME..."
mv "$TEMP_DIR/$TAR_NAME" "$SOURCES_DIR/"

rm -rf "$TEMP_DIR"

echo "Done! You can now run:"
echo "rpmbuild -ba packaging/rpm/khushu.spec"
