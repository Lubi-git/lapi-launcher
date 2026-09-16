# Arch Linux Binary Package

This directory contains the `PKGBUILD` for the precompiled `lapi-launcher`
release asset. The package definition does not invoke Cargo.

It installs the following files:

- `/usr/bin/lapi-launcher`
- `/etc/lapi-launcher/config.toml` as a Pacman backup configuration file
- `/etc/lapi-launcher/logo.png` as the official default logo
- `/usr/share/applications/lapi-launcher.desktop`
- `/usr/share/icons/hicolor/256x256/apps/lapi-launcher.png`

The configuration uses `path = "./logo.png"`, so it resolves to the official logo
next to the global configuration. Pacman preserves an administrator-modified
configuration during upgrades while updating the installed logo asset.

## GitHub Release

Create and push the `v<version>` Git tag, then publish its GitHub Release with
the executable asset named `lapi-launcher` and a copy of `PKGBUILD`. GitHub
automatically publishes a SHA-256 digest for every release asset. The
PKGBUILD downloads that exact asset and verifies it with the checksum embedded
in the package recipe. It also verifies the configuration, logo, desktop entry,
and license from the matching Git tag. The current package targets `x86_64`.

A release may additionally include the prebuilt `lapi-launcher.pkg.tar.zst`
package. Arch Linux users can install that file directly with:

    sudo pacman -U lapi-launcher.pkg.tar.zst

Prebuilt package artifacts are not stored in Git.

Users with GitHub CLI can verify the downloaded binary with:

    gh release verify-asset v<version> lapi-launcher

Before publishing, update the package version, embedded binary checksum, and
release number as needed, then verify the binary ABI against the oldest
supported Arch environment.

## AUR

The AUR repository contains `PKGBUILD` and `.SRCINFO` at its root. It does not
contain the binary or a prebuilt Pacman package. AUR helpers download the
release asset and build the package on the user's machine.

## Local Build

On Arch Linux, place the published `PKGBUILD` in an empty directory and run:

    makepkg -si

`makepkg` downloads the release binary, verifies every declared checksum, and
creates the package locally.
