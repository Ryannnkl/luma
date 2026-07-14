# Current architecture

Luma is a Rust Wayland client. The first compositor target is niri through
`ext-session-lock-v1`; a normal fullscreen window is never used as a lock.

## Runtime paths

The command paths are intentionally separate:

- `--demo` opens a normal `eframe` window. It never connects to PAM or requests
  a session lock.
- `--lock` is the authenticated path. It validates `/etc/pam.d/luma`, requests
  the session lock, and unlocks only after PAM returns success.
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
6. Receive keyboard text into `InputState`. Backspace removes one Unicode scalar
   value; Enter transfers a `PasswordAttempt` to the PAM boundary.
7. Flush the cleared prompt frame, authenticate the process owner with the
   `luma` PAM service, and discard the attempt when PAM returns.
8. Call `unlock_and_destroy` only for `Authenticated`, then flush that request.

`finished` without a successful PAM result is treated as an unsuccessful lock
run. The client never treats a client crash, Enter alone, a blank password, or
an authentication error as an unlock authorization.

## Security boundaries

- `src/input.rs` owns password bytes in `zeroize::Zeroizing<Vec<u8>>`. The
  password handoff has no public byte accessor; only the crate-local auth module
  can borrow it for the PAM conversation.
- `src/auth.rs` resolves the username from the process UID using `uzers`, never
  from `$USER` or configuration. It uses `pam-client2` with a custom conversation
  and does not log PAM prompts or messages.
- `pam/luma` imports only the `auth` rules from the system `login` policy. Luma
  does not create a PAM session or run account-management rules while unlocking
  an already-running desktop session.
- The production path contains no timer, environment-variable unlock gate, or
  secret bypass. The smoke timer is removed from release builds with
  `debug_assertions`.

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

This state machine is not connected to the current synchronous PAM call yet.
The next authentication milestone must move PAM work off the Wayland event loop
and drive this contract with attempt-scoped worker results.

## Current limitations

These are known follow-up tasks, not reasons to bypass the safety rules:

- PAM runs synchronously after the prompt frame is flushed. The authentication
  state model exists, but connecting its throttling and stale-result protection,
  rendering generic visible feedback, and moving PAM work off the event loop are
  still pending.
- The real lock currently renders the opaque software fallback only. Background
  capture, blur, clock typography, theming of the real lock, and animation are
  not connected to the lock surfaces yet.
- Shared-memory allocation and attach failures need a reviewed recovery path
  that preserves an opaque usable prompt before primary-session use.
- Output hotplug is handled, but repeated scale, transform, suspend/resume, and
  GPU-loss scenarios still require dedicated tests.
- The niri keybinding and wlogout integration remain unchanged; swaylock must
  stay installed as the recovery locker.

## Verification status

The authenticated path has been exercised in a nested niri with a watchdog. A
correct password unlocked only the nested compositor. The release binary builds
without the smoke command, and the current suite passes `51` tests with
`cargo fmt`, Clippy, and Cargo tests.
