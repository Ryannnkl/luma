#!/usr/bin/env bash

set -euo pipefail

usage() {
    echo "Usage: $0 VERSION SOURCE_TARBALL VENDOR_ARCHIVE OUTPUT_DIRECTORY" >&2
}

if [[ $# -ne 4 ]]; then
    usage
    exit 2
fi

version=${1#v}
source_tarball=$2
vendor_archive=$3
output_dir=$4

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Version must use MAJOR.MINOR.PATCH without a leading v." >&2
    exit 2
fi

for input in "$source_tarball" "$vendor_archive"; do
    if [[ ! -f "$input" ]]; then
        echo "Required source file does not exist: $input" >&2
        exit 1
    fi
done

for command in rpmbuild rpmlint rpm; do
    if ! command -v "$command" >/dev/null; then
        echo "$command is required to build the Fedora package." >&2
        exit 1
    fi
done

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
project_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
spec="$project_root/packaging/fedora/lumalock.spec"
spec_version=$(sed -n 's/^Version:[[:space:]]*//p' "$spec")

if [[ "$spec_version" != "$version" ]]; then
    echo "Fedora spec version is $spec_version, not $version." >&2
    exit 1
fi

build_root=$(mktemp -d)

cleanup() {
    rm -rf -- "$build_root"
}
trap cleanup EXIT

mkdir -p \
    "$build_root/BUILD" \
    "$build_root/BUILDROOT" \
    "$build_root/RPMS" \
    "$build_root/SOURCES" \
    "$build_root/SPECS" \
    "$build_root/SRPMS" \
    "$output_dir"

cp -- "$spec" "$build_root/SPECS/lumalock.spec"
cp -- "$source_tarball" "$build_root/SOURCES/v$version.tar.gz"
cp -- "$vendor_archive" "$build_root/SOURCES/luma-$version-vendor.tar.xz"

rpmlint "$build_root/SPECS/lumalock.spec"
rpmbuild \
    -ba \
    --define "_topdir $build_root" \
    "$build_root/SPECS/lumalock.spec"

source_rpm="$build_root/SRPMS/lumalock-$version-1.fc44.src.rpm"
binary_rpm="$build_root/RPMS/x86_64/lumalock-$version-1.fc44.x86_64.rpm"

if [[ ! -f "$source_rpm" || ! -f "$binary_rpm" ]]; then
    echo "Expected Fedora packages were not created." >&2
    exit 1
fi

rpmlint "$source_rpm" "$binary_rpm"
rpm -qpl "$binary_rpm" >/dev/null

cp -- "$source_rpm" "$binary_rpm" "$output_dir/"
sha256sum "$output_dir"/*.rpm
