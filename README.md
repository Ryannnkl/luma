# Luma

Luma is an experimental, customizable Wayland session locker written in Rust.
Its primary target is niri. Support for other Wayland compositors implementing
`ext-session-lock-v1` is a secondary, protocol-based goal.

> [!WARNING]
> Luma does not lock sessions yet. The current application is a harmless visual
> demo and must not be used as a security boundary.

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

## Development checks

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

See [ROADMAP.md](ROADMAP.md) for the staged implementation plan and
[AGENTS.md](AGENTS.md) for security invariants and contribution rules. Real lock
work must follow the isolated procedure in [docs/TESTING.md](docs/TESTING.md).

## Security status

Real session locking and PAM authentication are intentionally unavailable. They
will be introduced only after the visual layer, lock lifecycle, opaque fallback,
and isolated test environment are ready.

## License

Luma is licensed under the MIT License.
