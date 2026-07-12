# Luma implementation roadmap

Luma will grow from a harmless visual prototype into a real session locker. Real
locking remains disabled until its lifecycle and recovery behavior are tested in
an isolated environment.

## Phase 1: repository foundation

- Establish contribution, safety, and Git rules.
- Create a minimal Rust binary with formatting, linting, and test commands.
- Add CI for formatting, Clippy, and tests.

## Phase 2: demo application

- Add a `--demo` mode that cannot acquire a session lock.
- Render a window with the two-line clock and bottom password indicator.
- Add theme configuration and responsive output scaling.
- Use synthetic input only; do not connect demo mode to PAM.

## Phase 3: Wayland lock foundation

- Connect to Wayland and discover `ext-session-lock-v1` support.
- Track all outputs and their configure, scale, and transform events.
- Render an opaque fallback on every lock surface.
- Model and test the lock lifecycle as explicit state transitions.
- Exercise the implementation only in a nested compositor or virtual machine.

## Phase 4: authentication

- Add a minimal PAM service and a narrow Rust authentication boundary.
- Store password input in zeroizing memory and exclude secrets from diagnostics.
- Add retry throttling, cancellation, and generic failure messages.
- Keep rendering and input responsive while PAM performs authentication.

## Phase 5: visual design

- Capture the current session before acquiring the lock.
- Add GPU blur, dimming, clock typography, prompt feedback, and subtle animation.
- Preserve the opaque software fallback when capture or GPU rendering fails.
- Support multiple monitors without exposing uncaptured session contents.

## Phase 6: resilience and integration

- Test process failures, output hotplug, suspend/resume, scaling, and GPU loss.
- Add niri keybinding and wlogout integration documentation.
- Add manual TTY recovery documentation.
- Package Luma for Fedora and prepare a COPR-compatible build.

## Release gate

Luma may replace swaylock only after all of the following are true:

- Authentication and lock lifecycle tests pass.
- Multi-output and hotplug scenarios have been exercised.
- Renderer failures preserve an opaque, usable prompt.
- Suspend and resume have been tested repeatedly.
- TTY recovery has been verified by the user.
- swaylock remains available during an initial trial period.
