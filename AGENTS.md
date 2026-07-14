# Luma Contributor Guide

## Project purpose

Luma is a secure, customizable Wayland session locker written in Rust. Its first
target is niri. Support for other compositors must be based on standard Wayland
protocols, beginning with `ext-session-lock-v1`.

## Safety invariants

- A normal fullscreen window is never considered a session lock.
- Production builds must never include a password bypass, secret unlock key, or
  crash-to-unlock behavior.
- Keep `--lock-smoke` and its timer behind `debug_assertions`; release builds must
  not contain that command or its environment-variable gate.
- Unlock only after successful authentication through PAM.
- Keep the PAM policy at `pam/luma`, authenticate only, and do not open a new PAM
  session or run login account-management rules while unlocking.
- Never log, persist, clone unnecessarily, or expose password contents.
- Clear sensitive input from memory immediately after each authentication attempt.
- Scope every authentication result to an attempt token. Ignore stale or cancelled
  results, and authorize unlocking only through the authenticated state transition.
- Validate critical resources before requesting a session lock. After the
  compositor's `locked` event, create an opaque fallback surface with a usable
  authentication prompt for every active output.
- Handle outputs added, removed, resized, scaled, or transformed while locked;
  never leave a newly active output uncovered.
- Keep demo mode separate from real locking. Demo mode must never authenticate a
  real user or acquire `ext-session-lock-v1`.
- Test real locking in a nested compositor or virtual machine before testing it in
  the primary desktop session.
- Follow `docs/TESTING.md` for nested niri setup, watchdog recovery, and the
  real-session test gate.
- Keep swaylock installed and configured as a recovery option until Luma has been
  exercised successfully in production-like tests.

## Architecture

Keep security-sensitive code small and independent from presentation code:

- `wayland`: session-lock lifecycle, outputs, surfaces, and input dispatch.
- `auth`: PAM integration and secret-memory handling.
- `renderer`: opaque fallback, background, blur, clock, prompt, and animations.
- `state`: explicit application state transitions.
- `config`: validated user-facing configuration.
- `diagnostics`: useful logs that never contain secrets or raw key events.

The intended lifecycle is:

1. Validate configuration, the PAM policy, and critical resources.
2. Capture the session background, when supported.
3. Request the Wayland session lock.
4. Wait for the compositor's `locked` event.
5. Create and render an opaque surface for every active output.
6. Accept input and authenticate through PAM.
7. Call `unlock_and_destroy` only after authentication succeeds.

Authentication state is modeled independently in `src/state.rs`. The Wayland
client must begin at most one attempt at a time, associate worker results with the
returned `AttemptToken`, and act on `UnlockAuthorized` only. Denial and
infrastructure failure keep the session locked and pass through feedback and a
progressively bounded cooldown before input is accepted again.

## Development workflow

- Use stable Rust and keep `cargo fmt`, `cargo clippy`, and tests passing.
- Prefer safe Rust. Any `unsafe` block requires a nearby safety explanation and a
  focused review.
- Add tests for state transitions, configuration validation, and failure paths.
- Do not test an unreviewed real-lock path in the primary session.
- Do not introduce a dependency without checking its maintenance status, license,
  and role in the trusted computing base.

## Git conventions

- Write code, documentation, branches, and commit messages in English.
- Use Conventional Commits, such as `feat:`, `fix:`, `docs:`, `test:`, `refactor:`,
  `build:`, `ci:`, and `chore:`.
- Each commit must implement one coherent change and remain reviewable on its own.
- Keep commit subjects concise. Use a short body only when the reason cannot be
  understood from the diff.
- Do not add co-author trailers.
- Do not mix formatting, refactoring, dependency changes, and features unless they
  are inseparable parts of the same change.
- Never rewrite or discard user changes without explicit permission.
