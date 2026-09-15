# Pacman Binary Package

This directory packages an already compiled `lapi-launcher` binary for Arch Linux
and Pacman. The package definition does not invoke Cargo or download source code.

It installs the following files:

- `/usr/bin/lapi-launcher`
- `/etc/lapi-launcher/config.toml` as a Pacman backup configuration file
- `/etc/lapi-launcher/logo.png` as the official default logo
- `/usr/share/applications/lapi-launcher.desktop`
- `/usr/share/icons/hicolor/256x256/apps/lapi-launcher.png`

The configuration uses `path = "./logo.png"`, so it resolves to the official logo
next to the global configuration. Pacman preserves an administrator-modified
configuration during upgrades while updating the installed logo asset.

## Release Input

Build the release binary for the target architecture first and publish that file
with its checksum. The PKGBUILD expects an executable Linux binary through
`LAPI_BINARY`, not a Rust source build. The current package targets `x86_64`.

Before publishing to an Arch repository, replace the maintainer and license
metadata with the project’s final values, update the package version and release,
and verify the binary ABI against the oldest supported Arch environment.

## Build

    cd packaging/arch
    LAPI_BINARY=/absolute/path/to/lapi-launcher makepkg -f

The resulting `.pkg.tar.zst` is the native Lapi Launcher distribution artifact.
