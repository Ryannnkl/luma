# Safe lock testing

This guide separates harmless UI testing, isolated protocol testing, and the
eventual real-session test. Do not skip directly to the last stage.

## Current demo

The current `--demo` mode is a normal window. It never requests a session lock
or contacts PAM, and `Escape` closes it:

```sh
cargo run -- --demo
```

## Verify TTY recovery first

Before implementing or starting a real lock, identify the graphical session's
virtual terminal:

```sh
loginctl show-session "$XDG_SESSION_ID" -p VTNr
```

Then verify recovery while nothing is locked:

1. Save open work.
2. Switch to a spare TTY, usually with `Ctrl+Alt+F3`.
3. Log in as the normal user.
4. Run `loginctl list-sessions` and confirm the graphical session is visible.
5. Return to the graphical VT, currently `Ctrl+Alt+F2` on the development machine.

Do not begin real-session testing until this has worked.

## Start an isolated nested niri

Run niri as a nested window from a terminal in the real session. Do not pass
`--session`; that flag is reserved for the main compositor instance.

The user service manager already has the outer Wayland environment. Start the
nested compositor as a named transient service:

```sh
systemd-run --user --unit=luma-nested-test --collect niri -- alacritty
```

Create an automatic two-minute escape hatch:

```sh
systemd-run --user --unit=luma-nested-watchdog --on-active=2m \
  systemctl --user stop luma-nested-test.service
```

The nested niri appears as one window inside the real niri session. A session
lock acquired inside it covers only the nested compositor.

## Validate the environment with the existing locker

Before testing Luma's future lock client, run the installed swaylock-effects
binary from the terminal inside nested niri:

```sh
swaylock
```

Confirm that it locks only the nested window and that the normal password
unlocks it. This validates the nested compositor and authentication path without
involving unfinished Luma lock code.

## Easy exits from a failed nested test

Use these in order:

1. Unlock normally if input and authentication still work.
2. From any terminal in the outer session, terminate only the nested compositor:

   ```sh
   systemctl --user stop luma-nested-test.service
   ```

3. Wait for the two-minute watchdog to terminate it automatically.
4. If the outer session itself becomes unresponsive, switch to the verified TTY.

After a successful test, stop the nested compositor and cancel the watchdog
before it fires:

```sh
systemctl --user stop luma-nested-test.service
systemctl --user stop luma-nested-watchdog.timer
```

Terminating nested niri destroys only the isolated test compositor and its child
clients. It does not unlock or terminate the real desktop session.

## Luma protocol test gate

Luma must not request `ext-session-lock-v1` until all of these are true:

- the lock client is launched inside nested niri;
- every output receives an opaque fallback surface;
- the compositor's `locked` event is handled explicitly;
- lock-client crashes and renderer failures have been exercised;
- the outer-session service exit and watchdog have both been verified;
- no development bypass is present in the production binary.

The normal `luma` binary will not gain a password bypass. The current smoke
command is explicitly guarded and is not a production lock mode.

The current smoke client is guarded by an environment variable and unlocks after
five seconds. Run it only from the outer session with the nested compositor as
its parent:

```sh
systemd-run --user --unit=luma-smoke-watchdog --on-active=30s \
  systemctl --user stop luma-lock-smoke.service
systemd-run --user --unit=luma-lock-smoke --collect \
  niri -- env LUMA_ALLOW_LOCK_SMOKE=1 \
  /absolute/path/to/luma --lock-smoke
```

The nested niri window will be covered by Luma's opaque surface, then unlock
automatically after five seconds. Stop the named service after the test:

```sh
systemctl --user stop luma-lock-smoke.service
systemctl --user stop luma-smoke-watchdog.timer
```

This smoke path does not authenticate a password and must never be used as the
production keybinding.

The smoke client now receives keyboard text and clears it on focus or seat loss,
but it intentionally does not render password dots or authorize unlocks. Those
behaviors arrive with the lock renderer and PAM integration.

## Eventual real-session test

Keep swaylock configured as the normal keybinding while Luma is experimental.
Start the first real Luma lock manually from a terminal instead of changing niri
or wlogout configuration.

If a real lock client crashes after niri confirms the session lock, killing the
client may leave the session securely locked. From the verified TTY, inspect it:

```sh
loginctl list-sessions
loginctl session-status SESSION_ID
```

As a destructive last resort, terminate the graphical session and return to the
display manager:

```sh
loginctl terminate-session SESSION_ID
```

This closes applications in that session and can lose unsaved work. It is not a
normal unlock mechanism.
