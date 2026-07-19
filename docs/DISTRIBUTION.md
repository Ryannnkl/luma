# Distribution

Luma is distributed under the package name `lumalock`. Packages must continue
to install the executable as `/usr/bin/luma` and the PAM policy as
`/etc/pam.d/luma` so existing commands, configuration, and authentication keep
working.

## Release inputs

Package only signed or otherwise intentional upstream tags. Before publishing a
new package version:

1. Update the version in `Cargo.toml` and both distribution recipes.
2. Run `cargo fmt --check`, `cargo clippy`, and the complete test suite.
3. Create the upstream tag and GitHub release.
4. Update source checksums in `packaging/aur/PKGBUILD` and regenerate
   `packaging/aur/.SRCINFO`.
5. Reset the Fedora RPM `Release` to `1` and add its changelog entry.

Never package an uncommitted worktree or silently substitute a different source
archive for an existing version.

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
committed, copied into build artifacts, or included in logs.

## Arch User Repository

The reviewed recipe lives in `packaging/aur`. Copy `PKGBUILD` and `.SRCINFO` to
the separate AUR Git repository, validate with a clean Arch build, and publish an
atomic Conventional Commit. The AUR recipe must compile the tagged source and
verify its checksum rather than installing a prebuilt GitHub binary.

## Package safety

- Mark `/etc/pam.d/luma` as a preserved package configuration file.
- Do not package debug-only demo or smoke escape behavior in release binaries.
- Do not add installation scripts that replace a user's active locker or desktop
  hooks automatically.
- Test the actual repository package, not only a locally produced binary.
- Preserve user configuration during package removal.
