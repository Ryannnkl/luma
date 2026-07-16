# PAM authentication

Luma authenticates the user that owns its process through the `luma` PAM
service. The username is resolved from the real UID and is never accepted from
environment variables or configuration.

The source policy lives at `pam/luma`. On Fedora, install the linker dependency
before building Luma with PAM support:

```sh
sudo dnf install pam-devel
```

Install the policy only when testing the reviewed authentication integration:

```sh
sudo install -Dm644 pam/luma /etc/pam.d/luma
```

The policy imports only the `auth` rules from the system `login` stack. Luma
does not open a PAM session or run account-management rules because it is
unlocking an existing desktop session rather than creating a new login.

The Rust authentication boundary returns only three states to the lock client:
authenticated, denied, or infrastructure failure. Incorrect passwords, unknown
users, and similar credential failures are intentionally indistinguishable to
the interface. PAM prompts and messages are not logged or retained.

The authenticated path is started with:

```sh
luma --lock
```

Luma checks that `/etc/pam.d/luma` is installed and readable before requesting
the Wayland session lock. Enter transfers the zeroizing password attempt to PAM.
Only an authenticated result authorizes `unlock_and_destroy`; denial or PAM
infrastructure failure leaves the session locked.

The authentication state issues an `AttemptToken` for each submission, rejects
concurrent attempts, ignores stale results, and enforces a progressive bounded
cooldown after failures. PAM runs on the dedicated `luma-pam` thread; its generic
completion wakes the Wayland event loop through a registered `calloop` channel.
An authentication panic is contained as an infrastructure failure and does not
authorize unlocking.

While PAM runs, the prompt replaces password-length dots with bounded generic
status text. Credential denial and infrastructure failure both render the same
failure text, followed by generic cooldown text. These feedback frames do not
reveal the submitted password length or failure category. Prompt geometry, dot
behavior, colors, duration, text size, and status strings come from the validated
`[input]` configuration and are clipped to the prompt rectangle.

The bounded `--lock-smoke` command remains disconnected from PAM and exists only
in debug builds. Release builds do not contain its command, timer, or environment
variable gate.

Authenticated testing uses `scripts/test-nested-lock.sh`. Its timer is an external
systemd watchdog that destroys only the nested compositor after 60 seconds; it is
not an authentication result and never calls Luma's unlock path.
