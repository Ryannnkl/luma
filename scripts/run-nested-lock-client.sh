#!/usr/bin/env bash

set -eu

if [ "$#" -ne 3 ]; then
    echo "Usage: ${0} LOCKER WALLPAPER DELAY_SECONDS" >&2
    exit 2
fi

locker="$1"
wallpaper="$2"
delay_seconds="$3"
wallpaper_pid=""

stop_wallpaper() {
    if [ -n "${wallpaper_pid}" ]; then
        kill "${wallpaper_pid}" >/dev/null 2>&1 || true
        wait "${wallpaper_pid}" 2>/dev/null || true
    fi
}

trap stop_wallpaper EXIT HUP INT TERM

swaybg -i "${wallpaper}" -m fill &
wallpaper_pid="$!"

# This delay belongs only to the nested test harness. It gives the wallpaper
# client time to present a frame before Luma captures the nested output.
sleep "${delay_seconds}"

if ! kill -0 "${wallpaper_pid}" >/dev/null 2>&1; then
    wait "${wallpaper_pid}"
    echo "The nested wallpaper process exited before Luma started." >&2
    exit 1
fi

if ! zenity \
    --question \
    --title="Luma nested test" \
    --text="Start the lock activation test?" \
    --ok-label="Lock now" \
    --cancel-label="Cancel"
then
    exit 0
fi

# Give the nested compositor one frame to remove the dialog so it is not part
# of the background capture. The measured interval starts after this delay.
sleep 0.1
started_at="$(date +%s%N)"
coproc LOCKER_PROCESS { "${locker}" --lock --notify-ready; }
locker_pid="${LOCKER_PROCESS_PID}"
if IFS= read -r ready_message <&"${LOCKER_PROCESS[0]}" &&
    [ "${ready_message}" = "LUMA_LOCK_READY" ]
then
    ready_at="$(date +%s%N)"
    elapsed_ms=$(((ready_at - started_at) / 1000000))
    echo "Luma covered every nested output in ${elapsed_ms} ms."
else
    echo "Luma exited before reporting that every nested output was covered." >&2
fi
wait "${locker_pid}"
