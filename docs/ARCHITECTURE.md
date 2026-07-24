# Current architecture

Luma is a Rust Wayland client. The first compositor target is niri through
`ext-session-lock-v1`; a normal fullscreen window is never used as a lock.

## Runtime paths

The command paths are intentionally separate:

- `--demo` is compiled only in debug builds and opens a normal `eframe` window.
  It never connects to PAM or requests a session lock. Release builds exclude the
  demo module and reject the command.
- `--lock` is the authenticated path. It loads validated configuration, validates
  `/etc/pam.d/luma`, requests the session lock, and unlocks only after PAM returns
  success. `--config PATH` selects an explicit TOML file.
- `--lock --daemonize` starts the same authenticated path as a child and waits
  for its readiness notification. The parent exits only after every current
  output has an opaque committed frame and the Wayland connection is flushed.
  This mode exists for waiting callers such as `swayidle -w`; the child remains
  the locker and has no alternate unlock path.
- `--lock-smoke` is a bounded protocol test compiled only in debug builds. It
  requires `LUMA_ALLOW_LOCK_SMOKE=1`, unlocks on a timer, and must never be used
  as a production keybinding.

## Authenticated lock lifecycle

The current `--lock` path in `src/wayland/smoke.rs` (the shared lock client
implementation) follows this sequence:

1. Verify the Luma PAM service is a readable regular file.
2. Connect to Wayland, bind the compositor, shared memory, outputs, seats, and
   session-lock manager, and reject an environment with no outputs.
3. Request `ext-session-lock-v1` and wait for `locked`.
4. Create one lock surface per active output. Each surface is configured by the
   compositor and receives an opaque ARGB8888 shared-memory frame.
5. Add surfaces for outputs appearing during the lock and remove surfaces for
   destroyed outputs.
6. When readiness notification was requested internally, flush Wayland and
   notify the waiting parent only after the compositor has confirmed the lock
   and every current output has an attached opaque frame.
7. Receive keyboard text into `InputState`. Backspace removes one Unicode scalar
   value; Enter starts an attempt and transfers its token and zeroizing
   `PasswordAttempt` to the PAM worker.
8. Continue dispatching Wayland while PAM runs. The worker sends only the token
   and a generic result through a `calloop` channel that wakes the event loop.
9. Apply the result to `AuthenticationState`. Denial and infrastructure failure
   render the same generic warning, keep the lock active, and enforce the
   progressive bounded cooldown.
10. Call `unlock_and_destroy` only when the active attempt returns
   `UnlockAuthorized`.
11. Flush the unlock request and exit the client event loop immediately.

`finished` rejects or cancels a lock; it is not sent to acknowledge a
client-initiated `unlock_and_destroy`. Receiving it without a successful PAM
result is treated as an unsuccessful lock run. The client never treats a client
crash, Enter alone, a blank password, or an authentication error as an unlock
authorization.

## Security boundaries

- `src/input.rs` owns password bytes in `zeroize::Zeroizing<Vec<u8>>`. The
  password handoff has no public byte accessor; only the crate-local auth module
  can borrow it for the PAM conversation.
- `src/auth.rs` resolves the username from the process UID using `uzers`, never
  from `$USER` or configuration. It uses `pam-client2` with a custom conversation
  and does not log PAM prompts or messages.
- `src/auth/worker.rs` owns the background PAM thread. It catches authentication
  panics as infrastructure failures, drops rejected requests immediately, and
  returns no password or PAM diagnostic through its completion channel.
- `pam/luma` imports only the `auth` rules from the system `login` policy. Luma
  does not create a PAM session or run account-management rules while unlocking
  an already-running desktop session.
- The production path contains no timer, environment-variable unlock gate, or
  secret bypass. The smoke timer is removed from release builds with
  `debug_assertions`.
- `src/wayland/opaque.rs` maps authentication phases to four opaque prompt states.
  Ready renders password dots; authenticating renders a three-dot loader; failure
  renders a short shake with a border and cross icon; cooldown renders a moving
  six-dot indicator. Feedback frames do not encode the previous password length
  or distinguish credential denial from infrastructure failure.
- `scripts/test-nested-lock.sh` is outside the runtime trust boundary. Its
  60-second systemd watchdog stops the named nested niri service rather than
  sending an unlock request. The production binary contains no corresponding
  timer or environment-variable gate.
- The real fallback consumes validated `[input]` geometry, shape, border, limits,
  colors, and feedback duration. Semi-transparent configured colors are
  composited over the opaque fallback; their alpha is never copied to the
  lock-surface frame.
- Clock and optional date text use either the validated embedded font or separate
  configured hour, minute, and date font files. Custom files must be regular,
  valid, and at most 16 MiB. Every configured font is loaded before the session
  lock is requested, and the authentication prompt is rendered last so text
  cannot cover it.
- `src/wayland/capture.rs` obtains one cursor-free wlr-screencopy frame per
  current output before requesting the lock. It accepts only ARGB8888/XRGB8888
  shared-memory buffers, bounds per-image and aggregate allocations, and maps
  captures to lock surfaces by stable output name.
- `src/renderer/background.rs` normalizes captures to packed opaque ARGB8888 and
  applies a bounded linear-time software blur. Pixels remain in process memory
  and are dropped when the locker exits.

## Authentication state contract

`src/state.rs` keeps authentication control independent from PAM, Wayland, and
rendering. Its public phases are idle, authenticating, denied, error, cooldown,
and authenticated.

- `begin_attempt` accepts only the idle state and returns a unique
  `AttemptToken`. A second submission cannot start concurrently.
- `complete_attempt` accepts only the active token. A stale or cancelled worker
  result is ignored, including a late successful result.
- Only an authenticated completion returns `UnlockAuthorized`. Denial and
  infrastructure failure return `KeepLocked` and use distinct internal phases
  without exposing credential details.
- Failed attempts pass through a generic feedback interval and a progressive,
  capped cooldown before input is accepted again.
- Time is supplied by the event loop using `Instant`, keeping transitions
  deterministic and unit-testable without sleeping.

The authenticated lock drives this state machine from the Wayland event loop.
Its dispatch timeout follows the next feedback or cooldown deadline, while the
worker completion channel wakes the loop immediately when PAM finishes.

## Current limitations

These are known follow-up tasks, not reasons to bypass the safety rules:

- Authentication prompt geometry and colors are configurable. Language-neutral
  feedback animations are connected to the real lock; broader motion theming is
  not configurable yet.
- A PAM transaction has no cancellation timeout yet. A PAM backend that never
  returns leaves the attempt authenticating, although Wayland rendering and
  output handling continue to run.
- The real lock supports optional screenshot capture and software blur. GPU blur,
  procedural fallback theming, and broader animation controls are not connected
  to the lock surfaces yet.
- Shared-memory allocation and attach failures need a reviewed recovery path
  that preserves an opaque usable prompt before primary-session use.
- Output hotplug is handled, but repeated scale, transform, suspend/resume, and
  GPU-loss scenarios still require dedicated tests.
- Automatic suspend locking still needs repeated real suspend/resume tests.
  Swaylock remains installed as a manual recovery locker during this trial.

## Verification status

The authenticated path, asynchronous PAM feedback, optional captured background,
and readiness-aware daemon parent have been exercised in a nested niri under the
external watchdog. Repeated manual primary-session locks also completed without
leaving a Luma process behind. The release binary builds without the smoke or
demo commands. The current suite passes `85` tests with cargo fmt, Clippy, and
Cargo tests. Renderer-failure, repeated output-change, and suspend/resume gates
remain open.
