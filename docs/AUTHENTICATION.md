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

The bounded `--lock-smoke` command remains disconnected from PAM. Do not enter a
real password into it; authentication will be connected only to a real lock path
without the smoke timer bypass.
