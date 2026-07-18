# Luma

Luma is an experimental, customizable Wayland session locker written in Rust.
Its primary target is niri. Support for other Wayland compositors implementing
`ext-session-lock-v1` is a secondary, protocol-based goal.

> [!WARNING]
> Luma's authenticated lock is still experimental. Test `--lock` only inside a
> nested compositor or virtual machine; keep swaylock configured as recovery and
> do not use Luma as the primary session keybinding yet.

## Development-only demo

The demo displays:

- a responsive two-line clock;
- a softly blurred abstract background;
- a dimmed overlay for legibility;
- a bottom input indicator that stores only a character count;
- explicit demo feedback instead of authentication.

Run it from a debug build with:

```sh
cargo run -- --demo
```

Use `Backspace` to remove input, `Enter` to preview feedback, and `Escape` to
close the window. Do not type a real password into development builds.
The release binary does not contain the demo module and rejects `--demo`.

## Configuration

Luma reads `~/.config/luma/config.toml` when it exists. Missing sections and
fields use built-in defaults, while unknown fields and invalid values produce a
clear startup error.

Start from the complete example:

```sh
mkdir -p ~/.config/luma
cp config.example.toml ~/.config/luma/config.toml
cargo run -- --demo
```

To test another file without changing the user configuration:

```sh
cargo run -- --demo --config ./my-theme.toml
```

The authenticated path also accepts an explicit validated file:

```sh
target/release/luma --lock --config ./my-theme.toml
```

Run that command only inside the guarded nested-compositor procedure. The helper
script uses the default `~/.config/luma/config.toml` path.

The configuration controls:

- optional in-memory output capture, software blur, background dimming, and colors;
- clock visibility, normalized position, size, two-line offsets, formats, and colors;
- optional date visibility, format, position, size, and color;
- input visibility, position, dimensions, dot behavior, feedback, and colors.

Positions use normalized coordinates from `0.0` to `1.0`. Time and date formats
use Chrono/strftime directives: `%H` is a 24-hour value, `%I` is a 12-hour value,
`%M` is minutes, and `%p` is AM/PM. Colors accept `#RRGGBB` or `#RRGGBBAA`.

The debug-only demo previews these visual sections with a fixed, non-configurable
development warning. The real opaque fallback uses the configured clock,
optional date, and `[input]` geometry, limits, colors, duration, and bounded
authentication animations. It always keeps the authentication prompt visible
even when `input.enabled` is false.

Background capture is disabled by default. Enable it explicitly and choose a
software blur radius from 0 through 64 pixels:

```toml
[background]
capture_enabled = true
blur_radius = 24
```

The capture happens once per output before the lock request, excludes the cursor,
stays in memory, and is never written to disk. A zero radius keeps the screenshot
sharp. If enabled capture fails, Luma refuses to lock. Outputs connected after
capture use the opaque fallback.

## Wayland capability check

Inspect the active compositor without acquiring a session lock:

```sh
cargo run -- --check
```

The command reports `ext_session_lock_manager_v1`, compositor and shared-memory
versions, plus the number of outputs and seats. It exits unsuccessfully when the
minimum opaque lock-surface foundation is unavailable.

List current output metadata without locking:

```sh
cargo run -- --outputs
```

This reports each output's name, logical size, scale, transform, and current
mode. The same tracker will be used when Luma creates one lock surface per output.

## Experimental authenticated lock

Install the PAM policy described in [docs/AUTHENTICATION.md](docs/AUTHENTICATION.md),
then run the authenticated path only by following the watchdog procedure in
[docs/TESTING.md](docs/TESTING.md):

```sh
LUMA_ALLOW_NESTED_TEST=1 ./scripts/test-nested-lock.sh
```

The lock accepts input through Wayland, renders its configured background, clock,
and optional date, and unlocks only after PAM succeeds. It does not yet provide
GPU blur, the complete real-lock theme, or production integration with niri and
wlogout. The test runner starts a new nested niri and an external 60-second
watchdog; it never adds a timed unlock to Luma.

## User-local installation

Install a reviewed release build at a stable user-local path with:

```sh
./scripts/install-user.sh
```

The installer uses the locked dependency graph, builds the release target, and
atomically replaces `~/.local/bin/luma`. Set `LUMA_INSTALL_DIR` to select another
user-writable directory. Installing the binary does not modify niri, swayidle,
wlogout, PAM, or the Luma configuration.

Keep every normal lock action pointed at swaylock during the first real-session
trial. Follow the TTY recovery gate and manual launch procedure in
[docs/TESTING.md](docs/TESTING.md); do not point a keybinding at a Cargo `target`
artifact.

## Development checks

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

See [ROADMAP.md](ROADMAP.md) for the staged implementation plan and
[AGENTS.md](AGENTS.md) for security invariants and contribution rules. Real lock
work must follow the isolated procedure in [docs/TESTING.md](docs/TESTING.md).
The current runtime design and known limitations are recorded in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Security status

Real session locking and PAM authentication remain experimental. A deliberate
manual primary-session trial is permitted only through the recovery procedure in
[docs/TESTING.md](docs/TESTING.md); Luma must not replace the normal integrations
until the release gate in [ROADMAP.md](ROADMAP.md) is satisfied.

## License

Luma is licensed under the MIT License.
