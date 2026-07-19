# Luma

[![CI](https://github.com/Ryannnkl/luma/actions/workflows/ci.yml/badge.svg)](https://github.com/Ryannnkl/luma/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Ryannnkl/luma)](https://github.com/Ryannnkl/luma/releases/latest)
[![License](https://img.shields.io/github/license/Ryannnkl/luma)](LICENSE)

Luma is a secure, customizable Wayland session locker written in Rust. It uses
`ext-session-lock-v1` for real session locking, authenticates through PAM, and is
designed first for the niri compositor.

It can capture each output before locking, blur the screenshots in memory, and
render a configurable clock, date, typography, and language-neutral password
feedback over an always-opaque lock surface.

## Table of contents

- [Preview](#preview)
- [Features](#features)
- [Installation](#installation)
- [Uninstallation](#uninstallation)
- [First run](#first-run)
- [Configuration](#configuration)
- [niri integration](#niri-integration)
- [Testing and recovery](#testing-and-recovery)
- [Development](#development)
- [Security model](#security-model)
- [Project documentation](#project-documentation)
- [License](#license)

> [!WARNING]
> Luma is experimental. Test it in a nested compositor before using it in your
> primary session, keep swaylock installed as a recovery option, and read the
> [safe testing guide](docs/TESTING.md) before changing automatic or suspend
> hooks.

## Preview

![Luma lock screen with a blurred wallpaper, clock, date, and password indicator](docs/assets/luma-lock-screen.png)

## Features

- Real Wayland session locking through `ext-session-lock-v1`.
- PAM authentication with zeroizing password memory.
- One opaque lock surface for every active output.
- Optional cursor-free screenshot capture and bounded software blur.
- Configurable clock, date, colors, geometry, formats, and TTF/OTF fonts.
- Independent fonts and colors for hours, minutes, and the date.
- Visual loading, failure, and cooldown feedback without localized text.
- Output hotplug handling with an opaque fallback for uncaptured outputs.
- `Backspace` removes one character; `Ctrl+Backspace` clears the complete input.
- Debug-only demo and smoke paths excluded from release builds.

## Installation

Distribution packages use the name `lumalock`, while the installed executable
remains `luma`. This keeps the package name distinctive without breaking Luma
configuration, commands, or the PAM service name.

### Arch Linux (AUR)

Install [`lumalock`](https://aur.archlinux.org/packages/lumalock) with an AUR
helper:

```sh
yay -S lumalock
```

Or build the reviewed recipe directly with the standard Arch tools:

```sh
git clone https://aur.archlinux.org/lumalock.git
cd lumalock
makepkg -si
```

The package builds the tagged source with its locked Rust dependencies and
installs the executable at `/usr/bin/luma` and the PAM policy at
`/etc/pam.d/luma`.

### Prebuilt release

Prebuilt releases currently support Linux x86_64. Install the latest release
from GitHub with:

```sh
curl -fsSL https://raw.githubusercontent.com/Ryannnkl/luma/main/install.sh | bash
```

The installer:

1. Downloads the latest binary, PAM policy, and checksums from
   [GitHub Releases](https://github.com/Ryannnkl/luma/releases/latest).
2. Verifies the downloaded release assets.
3. Installs the reviewed PAM policy at `/etc/pam.d/luma`, using `sudo` when the
   policy is missing or different.
4. Atomically installs the binary at `~/.local/bin/luma`.

Set `LUMA_INSTALL_DIR` to choose another user-writable binary directory:

```sh
curl -fsSL https://raw.githubusercontent.com/Ryannnkl/luma/main/install.sh |
  LUMA_INSTALL_DIR="$HOME/bin" bash
```

Luma dynamically uses PAM and libxkbcommon. On Fedora these runtime libraries
can be installed with:

```sh
sudo dnf install pam-libs libxkbcommon
```

## Uninstallation

Before uninstalling, remove Luma from automatic lock hooks such as niri,
Waybar, wlogout, and `swayidle`. Restore another locker for `before-sleep` so the
session is not left without automatic locking.

If Luma was installed from the AUR, remove the package with:

```sh
sudo pacman -Rns lumalock
```

If Luma was installed with the `curl` command above, remove the installed binary
and PAM policy manually with:

```sh
rm -f "${LUMA_INSTALL_DIR:-$HOME/.local/bin}/luma"
sudo rm -f /etc/pam.d/luma
```

The user configuration is intentionally preserved. Remove it too only when its
settings are no longer needed:

```sh
rm -rf "$HOME/.config/luma"
```

## First run

Check the active compositor without locking it:

```sh
luma --check
luma --outputs
```

Create a user configuration from the complete example:

```sh
mkdir -p ~/.config/luma
curl -fsSL https://raw.githubusercontent.com/Ryannnkl/luma/main/config.example.toml \
  -o ~/.config/luma/config.toml
```

Do not make Luma your automatic locker yet. Follow the
[nested-compositor procedure](docs/TESTING.md#authenticated-nested-lock-test),
verify normal and failed authentication, and only then run one deliberate manual
trial in the primary session:

```sh
luma --lock
```

## Configuration

Luma reads `~/.config/luma/config.toml`. Missing sections use safe defaults;
unknown fields and invalid values abort before the session-lock request.

A compact configuration looks like this:

```toml
[background]
capture_enabled = true
blur_radius = 24
dim_color = "#00000052"

[clock]
enabled = true
hour_format = "%H"
minute_format = "%M"
hour_color = "#93e6be"
minute_color = "#f6f8f7"
# hour_font_path = "/usr/share/fonts/example/Example-Bold.ttf"
# minute_font_path = "/usr/share/fonts/example/Example-Bold.ttf"

[date]
enabled = true
format = "%d/%m/%Y"
# font_path = "/usr/share/fonts/example/Example-Regular.ttf"
color = "#f6f8f7dc"

[input]
enabled = true
max_characters = 12
```

Important details:

- `capture_enabled` is disabled by default. When enabled, capture failure aborts
  before locking rather than silently showing unexpected content.
- `blur_radius` accepts values from `0` through `64`; `0` keeps the capture sharp.
- `hour_color`, `minute_color`, and `date.color` are independent.
- Font paths are optional, absolute TTF/OTF paths. Each configured font must be a
  regular valid file no larger than 16 MiB.
- Time and date formats use Chrono/strftime directives such as `%H`, `%M`, `%p`,
  `%d`, `%m`, and `%Y`.
- Colors accept `#RRGGBB` or `#RRGGBBAA`.
- Positions use normalized coordinates from `0.0` to `1.0`.
- The real authentication prompt remains visible even if `[input].enabled` is
  configured as `false`.

See [config.example.toml](config.example.toml) for every available field.

## niri integration

After successful nested and manual tests, the normal niri keybinding can launch
Luma directly:

```kdl
binds {
    Super+Alt+L hotkey-overlay-title="Lock with Luma" {
        spawn "luma" "--lock"
    }
}
```

Interactive launchers such as wlogout and Waybar should also run `luma --lock`.
For `swayidle`, use the normal command for an idle timeout and the
readiness-aware mode for `before-sleep`:

```kdl
spawn-sh-at-startup "swayidle -w timeout 600 'luma --lock' timeout 900 'niri msg action power-off-monitors' before-sleep 'luma --lock --daemonize'"
```

`--daemonize` does not weaken or bypass the lock. Its parent waits until niri has
confirmed the session lock, every current output has an opaque frame, and the
Wayland connection has been flushed. It then exits so `swayidle -w` can allow
suspend to continue while the child remains responsible for authentication.
Keep swaylock installed as a manual recovery option during the initial Luma
trial period, and test suspend/resume only after the other integrations work.

## Testing and recovery

Real lock testing must start in a nested niri protected by the external
60-second watchdog:

```sh
git clone https://github.com/Ryannnkl/luma.git
cd luma
LUMA_ALLOW_NESTED_TEST=1 ./scripts/test-nested-lock.sh
```

Stop the isolated test early with:

```sh
./scripts/test-nested-lock.sh --stop
```

Before a primary-session trial, save open work and verify that you can access a
TTY and identify the graphical session. Killing a session-lock client is not an
unlock mechanism; a broken primary-session test may require terminating the
graphical session and losing unsaved work.

The complete gates and recovery commands are documented in
[docs/TESTING.md](docs/TESTING.md).

## Development

Source builds use stable Rust. Fedora development dependencies include:

```sh
sudo dnf install cargo rust pam-devel libxkbcommon-devel
```

Run the project checks with:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
cargo build --locked --release
```

The debug-only visual demo is available with:

```sh
cargo run -- --demo
```

Do not enter a real password in demo mode. It does not acquire a session lock or
authenticate through PAM, and release builds do not contain it.

Version tags matching `vMAJOR.MINOR.PATCH` trigger the release workflow. GitHub
Actions builds the x86_64 binary and publishes it with the PAM policy and
`SHA256SUMS`.

## Security model

Luma treats security-sensitive code separately from presentation:

- Only a successful PAM result associated with the active authentication token
  can authorize `unlock_and_destroy`.
- Password contents are never logged or rendered and are cleared after every
  attempt.
- PAM runs outside the Wayland rendering loop.
- Authentication failure categories share the same visual feedback.
- Release builds contain no timer, escape key, secret bypass, or crash-to-unlock
  path.
- Configurations, fonts, PAM policy, and critical rendering resources are
  validated before requesting the session lock.
- Screenshots remain in memory, exclude the cursor, and are dropped when Luma
  exits.

Read [AGENTS.md](AGENTS.md) for the complete safety invariants and
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the runtime boundaries.

## Project documentation

- [Roadmap](ROADMAP.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Authentication](docs/AUTHENTICATION.md)
- [Safe testing](docs/TESTING.md)
- [Contributor guide](AGENTS.md)

## License

Luma is distributed under the [MIT License](LICENSE).
