#!/usr/bin/env bash

set -eu

LOCK_UNIT="luma-auth-lock.service"
WATCHDOG_UNIT="luma-auth-watchdog"
WATCHDOG_TIMER="${WATCHDOG_UNIT}.timer"
WATCHDOG_SERVICE="${WATCHDOG_UNIT}.service"
WATCHDOG_SECONDS=60

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
release_binary="${project_dir}/target/release/luma"

stop_test() {
    systemctl --user stop "${LOCK_UNIT}" >/dev/null 2>&1 || true
    systemctl --user stop "${WATCHDOG_TIMER}" "${WATCHDOG_SERVICE}" >/dev/null 2>&1 || true
}

case "${1:-start}" in
    start)
        ;;
    --stop|stop)
        stop_test
        echo "Stopped the nested Luma lock test and its watchdog."
        exit 0
        ;;
    *)
        echo "Usage: ${0} [start|--stop]" >&2
        exit 2
        ;;
esac

if [ "${LUMA_ALLOW_NESTED_TEST:-}" != "1" ]; then
    echo "Refusing to start without LUMA_ALLOW_NESTED_TEST=1." >&2
    echo "This command is only for an isolated nested niri test." >&2
    exit 1
fi

if [ -z "${WAYLAND_DISPLAY:-}" ] || [ -z "${XDG_RUNTIME_DIR:-}" ]; then
    echo "Refusing to start outside an active Wayland user session." >&2
    exit 1
fi

for command in cargo niri systemctl systemd-run; do
    if ! command -v "${command}" >/dev/null 2>&1; then
        echo "Missing required command: ${command}" >&2
        exit 1
    fi
done

if [ ! -f /etc/pam.d/luma ] || [ ! -r /etc/pam.d/luma ]; then
    echo "The readable PAM policy /etc/pam.d/luma is required." >&2
    echo "Install it only after reviewing pam/luma." >&2
    exit 1
fi

if systemctl --user is-active --quiet "${LOCK_UNIT}"; then
    echo "A nested Luma lock test is already active." >&2
    echo "Run ${0} --stop before starting another one." >&2
    exit 1
fi

cd "${project_dir}"
cargo build --release

if [ ! -x "${release_binary}" ]; then
    echo "Release binary was not created at ${release_binary}." >&2
    exit 1
fi

stop_test

systemd-run --user --unit="${WATCHDOG_UNIT}" --collect --on-active="${WATCHDOG_SECONDS}s" \
    systemctl --user stop "${LOCK_UNIT}"

if ! systemd-run --user --unit="${LOCK_UNIT%.service}" --collect \
    niri -- "${release_binary}" --lock
then
    stop_test
    echo "Could not start the nested niri lock test." >&2
    exit 1
fi

echo "Nested Luma lock test started."
echo "The external watchdog will close it after ${WATCHDOG_SECONDS} seconds."
echo "Close it sooner with: ${0} --stop"
