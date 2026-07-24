#!/usr/bin/env bash

set -euo pipefail

usage() {
    echo "Usage: $0 VERSION SUMMARY" >&2
    echo "Example: $0 0.4.0 \"Add configurable animations\"" >&2
}

if [[ $# -ne 2 ]]; then
    usage
    exit 2
fi

version=${1#v}
summary=$2

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Version must use MAJOR.MINOR.PATCH without a leading v." >&2
    exit 2
fi

if [[ -z "$summary" || "$summary" == *$'\n'* || ${#summary} -gt 100 ]]; then
    echo "Summary must be one non-empty line of at most 100 characters." >&2
    exit 2
fi

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
project_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
manifest="$project_root/Cargo.toml"
lockfile="$project_root/Cargo.lock"
spec="$project_root/packaging/fedora/lumalock.spec"

cd "$project_root"

if [[ "$(git branch --show-current)" != "main" ]]; then
    echo "Release preparation must run from the main branch." >&2
    exit 1
fi

if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "Release preparation requires a clean working tree." >&2
    exit 1
fi

current_version=$(sed -n \
    '/^\[package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' \
    "$manifest")

if [[ -z "$current_version" ]]; then
    echo "Could not read the package version from Cargo.toml." >&2
    exit 1
fi

if [[ "$current_version" == "$version" ]]; then
    echo "Cargo.toml is already at version $version." >&2
    exit 1
fi

highest_version=$(printf '%s\n%s\n' "$current_version" "$version" | sort -V | tail -n 1)
if [[ "$highest_version" != "$version" ]]; then
    echo "Version $version must be newer than $current_version." >&2
    exit 1
fi

if git rev-parse --verify --quiet "refs/tags/v$version" >/dev/null; then
    echo "Tag v$version already exists locally." >&2
    exit 1
fi

replace_manifest_version() {
    local input=$1
    local output=$2

    awk -v version="$version" '
        BEGIN {
            in_package = 0
            updated = 0
        }

        $0 == "[package]" {
            in_package = 1
        }

        in_package && !updated && /^version = "[^"]+"$/ {
            print "version = \"" version "\""
            updated = 1
            next
        }

        in_package && /^\[/ && $0 != "[package]" {
            in_package = 0
        }

        {
            print
        }

        END {
            if (!updated) {
                exit 42
            }
        }
    ' "$input" >"$output"
}

update_spec() {
    local input=$1
    local output=$2
    local release_date
    release_date=$(LC_ALL=C date '+%a %b %d %Y')

    awk \
        -v version="$version" \
        -v date="$release_date" \
        -v summary="$summary" '
        BEGIN {
            version_updated = 0
            release_updated = 0
            changelog_updated = 0
        }

        !version_updated && /^Version:[[:space:]]+/ {
            printf "Version:        %s\n", version
            version_updated = 1
            next
        }

        !release_updated && /^Release:[[:space:]]+/ {
            print "Release:        1%{?dist}"
            release_updated = 1
            next
        }

        !changelog_updated && $0 == "%changelog" {
            print
            print "* " date " Ryannnkl <ryannnkl@gmail.com> - " version "-1"
            print "- " summary
            print ""
            changelog_updated = 1
            next
        }

        {
            print
        }

        END {
            if (!version_updated || !release_updated || !changelog_updated) {
                exit 42
            }
        }
    ' "$input" >"$output"
}

manifest_tmp=$(mktemp "$project_root/.Cargo.toml.XXXXXX")
spec_tmp=$(mktemp "$project_root/packaging/fedora/.lumalock.spec.XXXXXX")

cleanup() {
    rm -f -- "$manifest_tmp" "$spec_tmp"
}
trap cleanup EXIT

replace_manifest_version "$manifest" "$manifest_tmp"
update_spec "$spec" "$spec_tmp"
chmod --reference="$manifest" "$manifest_tmp"
chmod --reference="$spec" "$spec_tmp"
mv -- "$manifest_tmp" "$manifest"
mv -- "$spec_tmp" "$spec"

# Cargo updates the root package entry while retaining the locked dependency set.
cargo check

lock_version=$(awk '
    $0 == "name = \"luma\"" {
        found = 1
        next
    }

    found && /^version = "/ {
        gsub(/^version = "|"$|"/, "")
        print
        exit
    }
' "$lockfile")

if [[ "$lock_version" != "$version" ]]; then
    echo "Cargo.lock contains luma $lock_version instead of $version." >&2
    exit 1
fi

bash -n install.sh scripts/*.sh
cargo fmt --check
git diff --check

echo
echo "Release $version is prepared locally."
echo "Review the changes before committing:"
git diff --stat
