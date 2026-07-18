#!/usr/bin/env bash

set -euo pipefail

readonly repository="Ryannnkl/luma"
readonly asset="luma-x86_64-unknown-linux-gnu"
readonly release_base="https://github.com/${repository}/releases/latest/download"

download_dir=""
install_temp=""

cleanup() {
    if [[ -n "${install_temp}" && -e "${install_temp}" ]]; then
        rm -f -- "${install_temp}"
    fi
    if [[ -n "${download_dir}" && -d "${download_dir}" ]]; then
        rm -rf -- "${download_dir}"
    fi
}

trap cleanup EXIT HUP INT TERM

if [[ "$(uname -s)" != "Linux" ]]; then
    echo "Luma release binaries currently support Linux only." >&2
    exit 1
fi

case "$(uname -m)" in
    x86_64|amd64)
        ;;
    *)
        echo "Luma release binaries currently support x86_64 only." >&2
        exit 1
        ;;
esac

if [[ -z "${HOME:-}" ]]; then
    echo "HOME is required to select the user installation directory." >&2
    exit 1
fi

for required_command in cmp curl install mktemp mv sha256sum uname; do
    if ! command -v "${required_command}" >/dev/null 2>&1; then
        echo "Missing required command: ${required_command}" >&2
        exit 1
    fi
done

readonly install_dir="${LUMA_INSTALL_DIR:-${HOME}/.local/bin}"
readonly destination="${install_dir}/luma"

download_dir="$(mktemp -d)"
curl --fail --location --show-error --silent \
    "${release_base}/${asset}" \
    --output "${download_dir}/${asset}"
curl --fail --location --show-error --silent \
    "${release_base}/luma.pam" \
    --output "${download_dir}/luma.pam"
curl --fail --location --show-error --silent \
    "${release_base}/SHA256SUMS" \
    --output "${download_dir}/SHA256SUMS"

(
    cd "${download_dir}"
    sha256sum --check SHA256SUMS
)

if [[ ! -r /etc/pam.d/luma ]] || ! cmp --silent "${download_dir}/luma.pam" /etc/pam.d/luma; then
    if ! command -v sudo >/dev/null 2>&1; then
        echo "sudo is required to install the PAM policy at /etc/pam.d/luma." >&2
        exit 1
    fi
    echo "Installing the reviewed PAM policy requires administrator access."
    sudo install -Dm644 "${download_dir}/luma.pam" /etc/pam.d/luma
fi

install -d -m 0755 "${install_dir}"
install_temp="$(mktemp "${install_dir}/.luma.XXXXXX")"
install -m 0755 "${download_dir}/${asset}" "${install_temp}"
mv -f -- "${install_temp}" "${destination}"
install_temp=""

echo "Installed Luma at ${destination}."
if [[ ":${PATH}:" != *":${install_dir}:"* ]]; then
    echo "Add ${install_dir} to PATH or invoke Luma with its absolute path."
fi
echo "Run ${destination} --check before configuring a lock keybinding."
