#!/usr/bin/env bash

set -euo pipefail

usage() {
    echo "Usage: $0 VERSION SOURCE_SHA256 [PACKAGE_DIRECTORY]" >&2
}

if [[ $# -lt 2 || $# -gt 3 ]]; then
    usage
    exit 2
fi

version=${1#v}
source_sha256=$2
package_dir=${3:-}

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Version must use MAJOR.MINOR.PATCH without a leading v." >&2
    exit 2
fi

if [[ ! "$source_sha256" =~ ^[[:xdigit:]]{64}$ ]]; then
    echo "Source checksum must be a SHA-256 digest." >&2
    exit 2
fi

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
project_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
package_dir=${package_dir:-"$project_root/packaging/aur"}
pkgbuild="$package_dir/PKGBUILD"
srcinfo="$package_dir/.SRCINFO"

if [[ ! -f "$pkgbuild" ]]; then
    echo "PKGBUILD was not found in $package_dir." >&2
    exit 1
fi

if ! command -v makepkg >/dev/null; then
    echo "makepkg is required to regenerate .SRCINFO." >&2
    exit 1
fi

pkgbuild_tmp=$(mktemp "$package_dir/.PKGBUILD.XXXXXX")
srcinfo_tmp=$(mktemp "$package_dir/.SRCINFO.XXXXXX")

cleanup() {
    rm -f -- "$pkgbuild_tmp" "$srcinfo_tmp"
}
trap cleanup EXIT

awk -v version="$version" -v checksum="$source_sha256" '
    BEGIN {
        version_updated = 0
        release_updated = 0
        checksum_updated = 0
    }

    !version_updated && /^pkgver=/ {
        print "pkgver=" version
        version_updated = 1
        next
    }

    !release_updated && /^pkgrel=/ {
        print "pkgrel=1"
        release_updated = 1
        next
    }

    !checksum_updated && /^sha256sums=/ {
        print "sha256sums=(\047" checksum "\047)"
        checksum_updated = 1
        next
    }

    {
        print
    }

    END {
        if (!version_updated || !release_updated || !checksum_updated) {
            exit 42
        }
    }
' "$pkgbuild" >"$pkgbuild_tmp"

chmod --reference="$pkgbuild" "$pkgbuild_tmp"
mv -- "$pkgbuild_tmp" "$pkgbuild"

(
    cd "$package_dir"
    makepkg --printsrcinfo
) >"$srcinfo_tmp"

chmod 0644 "$srcinfo_tmp"
mv -- "$srcinfo_tmp" "$srcinfo"

echo "Updated AUR recipe for lumalock $version."
