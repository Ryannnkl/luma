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
script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
project_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)

cd "$project_root"

for command in git gh; do
    if ! command -v "$command" >/dev/null; then
        echo "$command is required to start a release." >&2
        exit 1
    fi
done

if [[ "$(git branch --show-current)" != "main" ]]; then
    echo "Releases must start from the main branch." >&2
    exit 1
fi

if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "Starting a release requires a clean working tree." >&2
    exit 1
fi

git fetch --quiet origin main

local_head=$(git rev-parse HEAD)
remote_head=$(git rev-parse origin/main)
if [[ "$local_head" != "$remote_head" ]]; then
    echo "Local main must match origin/main before starting a release." >&2
    exit 1
fi

gh auth status --hostname github.com >/dev/null
"$script_dir/prepare-release.sh" "$version" "$summary"

git add Cargo.toml Cargo.lock packaging/fedora/lumalock.spec

unexpected_changes=$(git status --short | awk '
    {
        path = substr($0, 4)
        if (path != "Cargo.toml" &&
            path != "Cargo.lock" &&
            path != "packaging/fedora/lumalock.spec") {
            print path
        }
    }
')

if [[ -n "$unexpected_changes" ]]; then
    echo "Release preparation changed unexpected files:" >&2
    echo "$unexpected_changes" >&2
    exit 1
fi

echo
git diff --cached --stat
echo

if [[ "${LUMA_RELEASE_CONFIRM:-}" != "$version" ]]; then
    if [[ ! -t 0 ]]; then
        echo "Set LUMA_RELEASE_CONFIRM=$version for non-interactive use." >&2
        exit 1
    fi

    read -r -p "Commit, push, and start release $version? [y/N] " answer
    if [[ "$answer" != "y" && "$answer" != "Y" ]]; then
        echo "Release cancelled. Prepared changes remain in the working tree."
        exit 1
    fi
fi

git commit -m "chore: prepare $version release"
git push origin main

gh workflow run release.yml \
    --ref main \
    --field version="$version"

echo
echo "Release $version was started."
echo "Follow it with: gh run watch --exit-status"
