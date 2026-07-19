#!/usr/bin/env bash

set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
    echo "Usage: $0 VERSION [OUTPUT]" >&2
    exit 2
fi

version=$1
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
output=${2:-"$project_root/luma-$version-vendor.tar.xz"}
work_dir=$(mktemp -d)

cleanup() {
    rm -rf -- "$work_dir"
}
trap cleanup EXIT

manifest_version=$(sed -n '/^\[package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' \
    "$project_root/Cargo.toml")
if [[ "$manifest_version" != "$version" ]]; then
    echo "Cargo.toml version is $manifest_version, not $version" >&2
    exit 1
fi

cargo vendor \
    --quiet \
    --manifest-path "$project_root/Cargo.toml" \
    --locked \
    --versioned-dirs \
    "$work_dir/vendor" >/dev/null

install -Dm0644 \
    "$project_root/packaging/fedora/cargo-config.toml" \
    "$work_dir/.cargo/config.toml"

if ! command -v cargo2rpm >/dev/null; then
    echo "cargo2rpm is required to create the Fedora vendor manifest" >&2
    exit 1
fi

ln -s "$project_root/Cargo.toml" "$work_dir/Cargo.toml"
ln -s "$project_root/Cargo.lock" "$work_dir/Cargo.lock"
ln -s "$project_root/src" "$work_dir/src"
(
    cd "$work_dir"
    cargo2rpm -p "$work_dir/cargo-vendor.txt" write-vendor-manifest
)

source_date_epoch=${SOURCE_DATE_EPOCH:-$(git -C "$project_root" log -1 --format=%ct)}
tar \
    --sort=name \
    --mtime="@$source_date_epoch" \
    --owner=0 \
    --group=0 \
    --numeric-owner \
    -C "$work_dir" \
    -cJf "$output" \
    .cargo/config.toml cargo-vendor.txt vendor

sha256sum "$output"
