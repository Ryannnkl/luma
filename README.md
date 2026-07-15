# Luma

Luma is an experimental, customizable Wayland session locker written in Rust.
Its primary target is niri. Support for other Wayland compositors implementing
`ext-session-lock-v1` is a secondary, protocol-based goal.

> [!WARNING]
> Luma's authenticated lock is still experimental. Test `--lock` only inside a
> nested compositor or virtual machine; keep swaylock configured as recovery and
> do not use Luma as the primary session keybinding yet.

## Current demo

The demo displays:

- a responsive two-line clock;
- a softly blurred abstract background;
- a dimmed overlay for legibility;
- a bottom input indicator that stores only a character count;
- explicit demo feedback instead of authentication.

Run it with:

```sh
cargo run -- --demo
```

Use `Backspace` to remove input, `Enter` to preview feedback, and `Escape` to
close the window. Do not type a real password into development builds.

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

The configuration controls:

- window size and maximized state;
- procedural background colors, dimming, and color spots;
- clock visibility, normalized position, size, two-line offsets, formats, and colors;
- optional date visibility, format, position, size, and color;
- input visibility, position, dimensions, dot behavior, feedback, and colors;
- demo-label visibility, content, position, dimensions, and colors.

Positions use normalized coordinates from `0.0` to `1.0`. Time and date formats
use Chrono/strftime directives: `%H` is a 24-hour value, `%I` is a 12-hour value,
`%M` is minutes, and `%p` is AM/PM. Colors accept `#RRGGBB` or `#RRGGBBAA`.

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

The lock accepts input through Wayland and unlocks only after PAM succeeds. It
does not yet provide the final blurred background, clock, configurable real-lock
theme, or production integration with niri and wlogout. The test runner starts a
new nested niri and an external 30-second watchdog; it never adds a timed unlock
to Luma.

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

Real session locking and PAM authentication exist as an experimental nested-test
path. They are not approved for the primary session until the release gate in
[ROADMAP.md](ROADMAP.md) is satisfied.

## License

Luma is licensed under the MIT License.
