# Luma

Luma is an experimental, customizable Wayland session locker written in Rust.
The project is being developed for Sway and compositors that support
`ext-session-lock-v1`.

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

## Development checks

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

See [ROADMAP.md](ROADMAP.md) for the staged implementation plan and
[AGENTS.md](AGENTS.md) for security invariants and contribution rules.

## Security status

Real session locking and PAM authentication are intentionally unavailable. They
will be introduced only after the visual layer, lock lifecycle, opaque fallback,
and isolated test environment are ready.

## License

Luma is licensed under the MIT License.
