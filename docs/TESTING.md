# Safe lock testing

This guide separates harmless UI testing, isolated protocol testing, and the
eventual real-session test. Do not skip directly to the last stage.

## Debug-only demo

The `--demo` mode exists only in debug builds and opens a normal window. It never
requests a session lock or contacts PAM, and `Escape` closes it:

```sh
cargo run -- --demo
```

The release binary must reject `--demo`; escape-to-close behavior is not compiled
into the production locker.

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

Before testing Luma's authenticated lock client, run the installed swaylock-effects
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

Before promoting Luma to the primary session, all of these must be true:

- the lock client is launched inside nested niri;
- every output receives an opaque fallback surface;
- the compositor's `locked` event is handled explicitly;
- lock-client crashes and renderer failures have been exercised;
- the outer-session service exit and watchdog have both been verified;
- no development bypass is present in the production binary.

The normal release binary has no password bypass. The smoke command is compiled
only with `debug_assertions`, is explicitly guarded, and is not a production
lock mode.

The current smoke client is guarded by an environment variable and unlocks after
five seconds. Run it only from the outer session with the nested compositor as
its parent:

```sh
systemd-run --user --unit=luma-smoke-watchdog --on-active=30s \
  systemctl --user stop luma-lock-smoke.service
systemd-run --user --unit=luma-lock-smoke --collect \
  niri -- env LUMA_ALLOW_LOCK_SMOKE=1 \
  /absolute/path/to/target/debug/luma --lock-smoke
```

The nested niri window will be covered by Luma's opaque surface, then unlock
automatically after five seconds. Stop the named service after the test:

```sh
systemctl --user stop luma-lock-smoke.service
systemctl --user stop luma-smoke-watchdog.timer
```

This smoke path does not authenticate a password and must never be used as the
production keybinding.

The smoke client renders a bottom password-length indicator and clears it on
focus or seat loss. It never renders password contents and intentionally does
not authorize unlocks. The separate `--lock` path uses the same input renderer
but authorizes unlock only through PAM and has no timer bypass.

## Authenticated nested lock test

Review and install the PAM policy before starting the test:

```sh
sudo install -Dm644 pam/luma /etc/pam.d/luma
```

Start the guarded test from the project root:

```sh
LUMA_ALLOW_NESTED_TEST=1 ./scripts/test-nested-lock.sh
```

The runner loads `~/.config/luma/config.toml` when present. Copy and edit
`config.example.toml` there before starting if the test should exercise custom
prompt geometry or colors. Invalid configuration is rejected before niri is
locked.

The runner validates its dependencies and PAM policy, builds the release binary,
arms an external systemd watchdog, and then launches that binary inside a new
nested niri. After 60 seconds the watchdog stops `luma-auth-lock.service`, closing
the entire nested compositor even if Luma's own event loop is stuck. It does not
unlock Luma and cannot recover a lock started in the primary compositor.

Type the normal user password inside the nested lock and press Enter. An
incorrect password must leave it locked; a correct password must unlock only the
nested niri window. Verify this prompt sequence for an incorrect password:

1. The password dots become an animated three-dot loader while PAM runs.
2. The prompt shakes and shows a border with a cross icon without exposing the
   previous password length.
3. A moving six-dot cooldown indicator remains while input is intentionally ignored.
4. The neutral password dots return when another attempt is allowed.

Confirm that every nested output shows the same state and that output handling
remains responsive while PAM is running. The watchdog closes the nested window
automatically after 60 seconds. To close it sooner, run this from an outer-session
terminal:

```sh
./scripts/test-nested-lock.sh --stop
```

For the optional capture path, set `background.capture_enabled = true` and a
`background.blur_radius` from 0 through 64 in the test configuration. Confirm
that each output shows its own cursor-free screenshot beneath the clock and
prompt. A radius of 0 must remain sharp, a positive radius must visibly blur the
frame, and the prompt must remain opaque. Invalid radii and capture failures must
abort before nested niri becomes locked.

Confirm that release builds reject the smoke command:

```sh
target/release/luma --lock-smoke
```

Do not change the normal keybinding at this milestone. The asynchronous worker,
enforced cooldown, visual feedback, and captured background have passed the
guarded nested test. The next gate is one deliberate primary-session trial with
verified TTY recovery. A timeout for a stuck PAM backend is not implemented yet.

## Eventual real-session test

Keep swaylock configured as the normal keybinding while Luma is experimental.
Start the first real Luma lock manually from a terminal instead of changing niri
or wlogout configuration.

Before the first trial:

1. Save all work that must survive terminating the graphical session.
2. Record the graphical session ID and virtual terminal:

   ```sh
   loginctl show-session "$XDG_SESSION_ID" -p Id -p VTNr -p State
   ```

3. While nothing is locked, switch to a spare TTY, log in, confirm that the
   recorded graphical session appears in `loginctl list-sessions`, and return to
   the graphical VT. Do not continue until this recovery route works.
4. Build and atomically install the reviewed release binary:

   ```sh
   ./scripts/install-user.sh
   ~/.local/bin/luma --check
   ```

5. Confirm `/etc/pam.d/luma` still matches the reviewed `pam/luma` policy and
   leave the niri keybinding, swayidle command, and wlogout action on swaylock.

Start exactly one manual trial from a terminal in the primary session:

```sh
~/.local/bin/luma --lock
```

First confirm a normal unlock. On a later trial, confirm that an incorrect
password keeps the session locked before entering the correct password. Do not
test suspend/resume or output reconfiguration during this milestone.

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

Only after repeated manual trials and the remaining release-gate tests should
the normal integrations be changed, one at a time: the explicit niri keybinding,
then wlogout, and finally swayidle and `before-sleep`. Keep swaylock installed
through the initial Luma trial period.
