#!/usr/bin/env bash

set -eu

LOCK_UNIT="luma-auth-lock.service"
WATCHDOG_UNIT="luma-auth-watchdog"
WATCHDOG_TIMER="${WATCHDOG_UNIT}.timer"
WATCHDOG_SERVICE="${WATCHDOG_UNIT}.service"
WATCHDOG_SECONDS=60

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
release_binary="${project_dir}/target/release/luma"
nested_config="${project_dir}/scripts/niri-nested-test.kdl"
nested_client=("${release_binary}" --lock)

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

for command in cargo niri systemctl systemd-cat systemd-run; do
    if ! command -v "${command}" >/dev/null 2>&1; then
        echo "Missing required command: ${command}" >&2
        exit 1
    fi
done

if [ -n "${LUMA_NESTED_WALLPAPER:-}" ]; then
    for command in swaybg zenity; do
        if ! command -v "${command}" >/dev/null 2>&1; then
            echo "Missing required command for LUMA_NESTED_WALLPAPER: ${command}" >&2
            exit 1
        fi
    done
    if [ ! -f "${LUMA_NESTED_WALLPAPER}" ] || [ ! -r "${LUMA_NESTED_WALLPAPER}" ]; then
        echo "LUMA_NESTED_WALLPAPER must name a readable regular file." >&2
        exit 1
    fi
    wallpaper_delay="${LUMA_NESTED_WALLPAPER_DELAY_SECONDS:-1}"
    case "${wallpaper_delay}" in
        ''|*[!0-9]*)
            echo "LUMA_NESTED_WALLPAPER_DELAY_SECONDS must be a whole number." >&2
            exit 1
            ;;
    esac
    if [ "${wallpaper_delay}" -gt 10 ]; then
        echo "LUMA_NESTED_WALLPAPER_DELAY_SECONDS must not exceed 10." >&2
        exit 1
    fi
    nested_client=(
        "${project_dir}/scripts/run-nested-lock-client.sh"
        "${release_binary}"
        "${LUMA_NESTED_WALLPAPER}"
        "${wallpaper_delay}"
    )
fi

if [ ! -f /etc/pam.d/luma ] || [ ! -r /etc/pam.d/luma ]; then
    echo "The readable PAM policy /etc/pam.d/luma is required." >&2
    echo "Install it only after reviewing pam/luma." >&2
    exit 1
fi

if ! niri validate --config "${nested_config}"; then
    echo "The isolated nested niri configuration is invalid." >&2
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

systemd-run --user --unit="${WATCHDOG_UNIT}" --collect \
    --on-active="${WATCHDOG_SECONDS}s" --timer-property=AccuracySec=1s \
    systemctl --user stop "${LOCK_UNIT}"

if ! systemd-run --user --unit="${LOCK_UNIT%.service}" --collect \
    niri --config "${nested_config}" -- \
    systemd-cat --identifier=luma-nested-client "${nested_client[@]}"
then
    stop_test
    echo "Could not start the nested niri lock test." >&2
    exit 1
fi

echo "Nested Luma lock test started."
echo "The external watchdog will close it after ${WATCHDOG_SECONDS} seconds."
if [ -n "${LUMA_NESTED_WALLPAPER:-}" ]; then
    echo "The activation timer starts when the nested Lock now button is clicked."
fi
echo "Close it sooner with: ${0} --stop"
