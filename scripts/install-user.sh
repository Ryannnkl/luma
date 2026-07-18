#!/usr/bin/env bash

set -eu

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
release_binary="${project_dir}/target/release/luma"
install_dir="${LUMA_INSTALL_DIR:-${HOME}/.local/bin}"
destination="${install_dir}/luma"
temporary=""

cleanup() {
    if [ -n "${temporary}" ]; then
        rm -f "${temporary}"
    fi
}

trap cleanup EXIT HUP INT TERM

for command in cargo install mktemp mv; do
    if ! command -v "${command}" >/dev/null 2>&1; then
        echo "Missing required command: ${command}" >&2
        exit 1
    fi
done

cd "${project_dir}"
cargo build --locked --release

if [ ! -x "${release_binary}" ]; then
    echo "Release binary was not created at ${release_binary}." >&2
    exit 1
fi

install -d -m 0755 "${install_dir}"
temporary="$(mktemp --tmpdir="${install_dir}" .luma.XXXXXX)"
install -m 0755 "${release_binary}" "${temporary}"
mv -f "${temporary}" "${destination}"
temporary=""

echo "Installed Luma at ${destination}."
