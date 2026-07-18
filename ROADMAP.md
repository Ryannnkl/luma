# Luma implementation roadmap

Luma is growing from a harmless visual prototype into a real session locker.
Authenticated locking is available only for isolated nested-compositor testing
until the release gate is satisfied.

## Phase 1: repository foundation

- Establish contribution, safety, and Git rules.
- Create a minimal Rust binary with formatting, linting, and test commands.
- Add CI for formatting, Clippy, and tests.

## Phase 2: demo application

- Add a debug-only `--demo` mode that cannot acquire a session lock.
- Render a window with the two-line clock and bottom password indicator.
- Add theme configuration and responsive output scaling.
- Use synthetic input only; do not connect demo mode to PAM.

## Phase 3: Wayland lock foundation

- [x] Connect to Wayland and discover `ext-session-lock-v1` support.
- [x] Track outputs, configure lock surfaces, and handle output hotplug.
- [x] Render an opaque fallback on every lock surface.
- [x] Model and test the lock lifecycle as explicit state transitions.
- [x] Exercise the implementation only in a nested compositor or virtual machine.
- Scale, transform, suspend/resume, and renderer-failure scenarios still need
  dedicated lock tests before primary-session use.

## Phase 4: authentication

- [x] Build a zeroizing password-input state and receive Wayland keyboard text.
- [x] Redraw a password-length indicator without rendering password contents.
- [x] Add a minimal PAM service and a narrow Rust authentication boundary.
- [x] Store password input in zeroizing memory and exclude secrets from diagnostics.
- [x] Unlock only after PAM succeeds, without a timer bypass in release builds.
- [x] Model one active attempt, stale-result rejection, and progressive bounded
  cooldown as explicit state transitions.
- [x] Connect the authentication state and its cooldown to the lock event loop.
- [x] Keep rendering and input responsive while PAM performs authentication.
- [x] Render generic authentication failure feedback on every lock surface.

## Phase 5: visual design

- [x] Connect validated input geometry and colors to the opaque real-lock prompt.
- [x] Render clock, optional date, and bounded prompt feedback in software.
- [x] Capture every current output before acquiring the lock when configured.
- [x] Add bounded software blur and dimming without persisting screenshots.
- Add GPU blur, full typography, and subtle animation.
- [x] Preserve the opaque software fallback for new or unmatched outputs.
- [x] Support multiple captured monitors without exposing uncaptured contents.

## Phase 6: resilience and integration

- Test process failures, output hotplug, suspend/resume, scaling, and GPU loss.
- Add niri keybinding and wlogout integration documentation.
- [x] Add manual TTY recovery and staged real-session trial documentation.
- Package Luma for Fedora and prepare a COPR-compatible build.

## Release gate

Luma may replace swaylock only after all of the following are true:

- Authentication and lock lifecycle tests pass.
- Multi-output and hotplug scenarios have been exercised.
- Renderer failures preserve an opaque, usable prompt.
- Suspend and resume have been tested repeatedly.
- TTY recovery has been verified by the user.
- swaylock remains available during an initial trial period.
