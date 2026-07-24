# Distribution

Luma is distributed under the package name `lumalock`. Packages must continue
to install the executable as `/usr/bin/luma` and the PAM policy as
`/etc/pam.d/luma` so existing commands, configuration, and authentication keep
working.

## Automated release

The normal maintainer entry point is:

```sh
./scripts/start-release.sh 0.4.0 "Add configurable animations"
```

Use a version without a leading `v` and a short English changelog summary. The
script requires a clean `main` branch equal to `origin/main` and an authenticated
GitHub CLI. It then:

1. Updates `Cargo.toml`, `Cargo.lock`, and the Fedora spec.
2. Resets the RPM release to `1` and inserts the changelog entry.
3. Shows the exact version diff and asks for confirmation.
4. Creates and pushes `chore: prepare VERSION release`.
5. Dispatches the protected `Publish release` workflow.

The workflow has two security boundaries. The validation job has no distribution
credentials. It checks version consistency, formatting, Clippy, tests, the
release binary, shell scripts, debug-only marker exclusion, and Fedora's vendored
dependency archive. The publishing job starts only after approval through the
GitHub `release` environment.

After approval, the workflow:

1. Creates the immutable `vVERSION` tag and GitHub release assets.
2. Calculates the tagged source checksum, builds and installs the Arch package
   in a clean container, publishes it to the AUR, and synchronizes the reviewed
   recipe in `packaging/aur`.
3. Builds the SRPM and RPM without network access, runs package checks, and
   verifies local installation and removal.
4. Submits the SRPM to COPR, waits for the build, and verifies installation from
   the public repository.

Official actions are pinned to commits and packaging images are pinned to image
digests. Updating either pin is a deliberate maintenance change. Never package
an uncommitted worktree or silently substitute a different source archive for an
existing version.

### Release environment

Create a GitHub environment named `release` and require a maintainer review
before deployment. Add these environment secrets:

- `AUR_SSH_PRIVATE_KEY`: a dedicated, unencrypted deployment key registered on
  the maintainer's AUR account. Never reuse a personal SSH key.
- `AUR_KNOWN_HOSTS`: a previously verified `aur.archlinux.org` host-key entry.
- `COPR_CONFIG`: the complete COPR API configuration normally stored at
  `~/.config/copr`.

Generate the dedicated AUR key locally:

```sh
ssh-keygen -t ed25519 \
  -C "lumalock GitHub Actions AUR" \
  -f ~/.ssh/lumalock-aur-actions
```

Add `~/.ssh/lumalock-aur-actions.pub` to the SSH public keys in the AUR account.
After manually verifying and accepting the AUR host key, upload the three
environment secrets from the repository:

```sh
gh secret set AUR_SSH_PRIVATE_KEY --env release \
  < ~/.ssh/lumalock-aur-actions
ssh-keygen -F aur.archlinux.org -f ~/.ssh/known_hosts |
  sed '/^#/d' |
  gh secret set AUR_KNOWN_HOSTS --env release
gh secret set COPR_CONFIG --env release < ~/.config/copr
gh secret list --env release
```

Secrets must remain limited to the `release` environment. The workflow checkout
does not persist a GitHub credential inside the repository mounted into package
containers.

### Recovery and reruns

Release preparation failures leave the version changes uncommitted for review.
Do not start another release until those changes are either completed or
explicitly discarded.

If the preparation commit was pushed but workflow dispatch failed, start the
same version directly:

```sh
gh workflow run release.yml --ref main --field version=0.4.0
```

Follow the newest run with:

```sh
gh run watch --exit-status
```

A failed publishing job may be rerun for the same version only while its tag
still points to the original release commit. Existing release assets are
replaced, and already synchronized AUR state becomes a no-op. The workflow
refuses to reuse a version whose tag points elsewhere; never move or silently
replace a published release tag.

## Manual release fallback

Use this only when repairing the automation or when GitHub Actions is
unavailable:

1. Update the version in `Cargo.toml`, `Cargo.lock`, and the Fedora spec.
2. Reset the Fedora RPM `Release` to `1` and add its changelog entry.
3. Run `cargo fmt --check`, `cargo clippy`, and the complete test suite.
4. Create the intentional upstream tag and GitHub release.
5. Calculate the tagged source checksum, update `packaging/aur/PKGBUILD`, and
   regenerate `packaging/aur/.SRCINFO`.
6. Build and test the AUR and Fedora packages before publishing them.

## Fedora and COPR

The Fedora recipe is `packaging/fedora/lumalock.spec`. Rust dependencies are
locked by `Cargo.lock`, stored in a separate release asset, and consumed with
Cargo's offline mode. Generate that asset from the release commit with:

```sh
SOURCE_DATE_EPOCH="$(git show -s --format=%ct "v$VERSION")" \
  scripts/create-vendor-archive.sh "$VERSION"
```

This requires `cargo2rpm`. The resulting
`luma-$VERSION-vendor.tar.xz` contains the Cargo source directory, offline Cargo
configuration, and `cargo-vendor.txt`. Upload it to the matching GitHub release;
do not commit it to the repository.

Build and inspect both the source and binary RPM in a clean Fedora environment.
At minimum, run `rpmlint` on the spec, SRPM, and binary RPM, run the RPM test
suite, and verify installation, `luma --help`, file ownership, and removal in a
fresh container or virtual machine.

The public project is
[`ryannnkl/lumalock`](https://copr.fedorainfracloud.org/coprs/ryannnkl/lumalock/).
Submit the validated SRPM with:

```sh
copr-cli build lumalock /path/to/lumalock-$VERSION-$RELEASE.src.rpm
```

COPR network access must remain disabled. Wait for a successful build and test
installation from the public repository before updating user-facing installation
instructions. COPR credentials belong only in `~/.config/copr` and must never be
committed, copied into build artifacts, or included in logs. In GitHub Actions,
the equivalent credential is the protected `COPR_CONFIG` environment secret.

## Arch User Repository

The reviewed recipe lives in `packaging/aur`. The release workflow updates its
tagged source checksum only after the immutable GitHub tag exists, validates it
with a clean Arch build, and publishes an atomic Conventional Commit to the
separate AUR Git repository. The AUR recipe must compile the tagged source and
verify its checksum rather than installing a prebuilt GitHub binary.

## Package safety

- Mark `/etc/pam.d/luma` as a preserved package configuration file.
- Do not package debug-only demo or smoke escape behavior in release binaries.
- Do not add installation scripts that replace a user's active locker or desktop
  hooks automatically.
- Test the actual repository package, not only a locally produced binary.
- Preserve user configuration during package removal.
