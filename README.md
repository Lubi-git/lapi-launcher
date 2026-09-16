# Lapi Launcher

Lapi Launcher is a keyboard-first Linux application launcher written in Rust.
Its terminal user interface is inspired by Yazi: a dark layout, subtle borders,
fuzzy search, mouse and keyboard navigation, and optional in-terminal images.
It is a single native binary; it does not need a server, daemon, or background
process.

## Run

    cargo run --release

Lapi Launcher requires an interactive terminal and a minimum viewport of
40 × 20 cells. Desktop and Recent keep one launcher row each. Use the mouse
wheel, Page Up, or Page Down to scroll the complete home view when necessary.

    official logo
    System information
    ├ ▸ Operating System      › distribution
    ├ ▸ Desktop Environment   › desktop session
    └ ▸ PC                    › hardware

     LAUNCHER        INSTALLER          MANAGER

    ╭ Search ─────────────────────────────────────────╮
    │ Type an application name…                        │
    ╰─────────────────────────────────────────────────╯
    ▸ Desktop
       ╭──────────╮     ╭──────────╮
       │   icon   │     │   icon   │
       ╰──────────╯     ╰──────────╯
          App              App

      Recent
       applications launched by Lapi

Desktop lists only application entries, files, and folders placed directly in
the user desktop directory. That location comes from XDG_DESKTOP_DIR in
user-dirs.dirs. Recent contains items opened through Lapi, newest first.

While searching, both groups are replaced by a vertical list containing an icon
and one alias per result. Search includes installed applications plus desktop
files and folders, without crawling the rest of the filesystem. Esc clears the
query and restores the groups.

LAUNCHER identifies the active tool. Clicking INSTALLER or MANAGER restores the
terminal and replaces Lapi with lapi-installer or lapi-manager in that same
terminal. If the selected binary is not present in PATH, Lapi stays open and
reports the error.

Applications use their localized desktop-entry Name. Files use their basename
without an extension, while folders retain their name. The interface and
--list show aliases only.

## Controls

| Action | Control |
| --- | --- |
| Search | Type or paste text |
| Move selection | Arrow keys; the mouse wheel also moves search results |
| Change section | Tab / Shift+Tab or click a heading; Tab cycles results while searching |
| Select or open | Click / double-click; Enter opens the selection |
| Scroll the home view | Mouse wheel, Page Up / Page Down, or Ctrl+Up / Ctrl+Down |
| Jump to the beginning or end | Home / End |
| Expand operating system, desktop, or PC | Click its row or use F2 / F3 / F4 |
| Open Lapi Installer or Manager | Click INSTALLER / MANAGER |
| Refresh applications | F5 |
| Clear search | Ctrl+L / Ctrl+U / Esc |
| Quit | Esc with an empty query or Ctrl+C |
| Help | F1 |

## Configuration and Official Logo

Lapi combines configuration files in this order:

1. User: ${XDG_CONFIG_HOME:-~/.config}/lapi-launcher/config.toml
2. Global: /etc/lapi-launcher/config.toml

Every setting defined by the user takes precedence over the global setting. A
user can override one button color without losing global defaults. When neither
configuration exists, Lapi creates the user configuration from
[config.example.toml](config.example.toml). If an older configuration exists
next to the binary, it is migrated to the user location first.

The official 256 × 256 PNG is stored at
[resources/logo.png](resources/logo.png). The default configuration contains
path = "./logo.png". When Lapi creates or migrates a user configuration, it
writes the byte-identical official logo beside it as logo.png; an existing file
is never overwritten. The Pacman package installs the same pair in
/etc/lapi-launcher/, so the default path always resolves to the closest
configuration logo.

    [logo]
    source = "builtin"
    path = "./logo.png"
    width = 22
    height = 7

    [images]
    enabled = true
    protocol = "auto"
    # icon_theme = "breeze"

    [launcher]
    terminal = []
    recent_limit = 12
    close_on_launch = false

    [applications]
    extra_dirs = []
    # desktop_dir = "~/Desktop"

    [buttons]
    launcher_background = "#a6e3a1"
    installer_background = "#dc2626"
    manager_background = "#dc2626"
    launcher_foreground = "#000000"
    installer_manager_foreground = "#ffffff"

- logo.path has priority over source. Relative paths resolve from the directory
  containing their configuration file, and ~/ is supported.
- Without a custom path, source = "builtin" displays the Lapi text mark.
  source = "os" looks up the LOGO icon from /etc/os-release; desktop looks up
  the active desktop environment icon.
- width and height are terminal-cell dimensions. For a square image,
  width = height × 2 is a useful starting ratio for terminal cells.
- [buttons] uses five #RRGGBB colors. The Launcher foreground is independent;
  Installer and Manager share their foreground color.
- images.protocol supports auto, kitty, kitty-legacy, iterm2, sixel, and
  halfblocks. Auto uses ratatui-image detection and uses the classic Kitty
  protocol in Konsole when it is outside a multiplexer.
- PNG, JPEG, GIF (first frame), WebP, and ICO are decoded. SVG/SVGZ and XPM are
  not rasterized in this release. Use PNG for a custom logo.
- Images are loaded, scaled with Lanczos3, and encoded on a worker. Native
  graphics preserve source resolution up to 1024 pixels; halfblocks is limited
  by terminal cells.
- terminal is an argument list without shell interpretation. It is used only by
  desktop entries that set Terminal=true.
- recent_limit accepts 0–100. With close_on_launch = true, Lapi exits after
  starting a process; the default keeps it open.
- extra_dirs adds desktop-entry directories ahead of standard search paths.
  desktop_dir overrides the Desktop group directory.

## Pacman Binary Package

The PKGBUILD downloads the already compiled `lapi-launcher` asset from the
matching GitHub Release. It does not compile Rust code during installation.

[packaging/arch](packaging/arch) provides the Pacman PKGBUILD and .SRCINFO. The
global configuration is a Pacman backup file, so upgrades never overwrite an
administrator-edited configuration.

The package installs the binary, desktop entry, hicolor icon, global
configuration, and official logo. GitHub publishes a SHA-256 digest for each
release asset, while the PKGBUILD contains the checksums required by `makepkg`.
See [packaging/README.md](packaging/README.md) for release input, ABI, and
packaging instructions.

Releases may also include a prebuilt Pacman package named
`lapi-launcher.pkg.tar.zst`. Install it directly on Arch Linux with
`sudo pacman -U lapi-launcher.pkg.tar.zst`.

## System Information

System information is collected once at startup from /etc/os-release, /proc,
/sys, and session variables. Expanded rows provide kernel, architecture, uptime,
session, protocol, terminal, shell, model, CPU, threads, GPU, total RAM, and
persistent storage details.

Storage usage refers to the filesystem containing the home directory, not RAM.
If the root filesystem is on a different persistent volume, it is listed
separately. Lapi avoids presenting tmpfs and virtual overlay filesystems as
physical disks, deduplicates Btrfs subvolumes, and follows partitions,
device-mapper, encrypted, and LVM layers when Linux exposes enough information.

Each persistent volume can report its filesystem, backing device model, device
kind such as SSD/NVMe or HDD, and physical capacity. Physical device capacity
does not necessarily equal filesystem capacity. Available space is the amount
usable by the user; used space is obtained separately without counting reserved
blocks as used data. The complete view scrolls when needed.

These are passive startup details, not real-time monitoring data. Lapi does not
run Fastfetch and does not assume any specific distribution.

## Application Discovery and History

Lapi reads XDG_DATA_HOME/applications, XDG_DATA_DIRS, and common Flatpak and
Snap exports. It honors user-entry precedence, Hidden, NoDisplay, OnlyShowIn,
NotShowIn, and TryExec; invalid entries are skipped.

Application launching preserves quoted arguments, Exec field codes, the Path
working directory, and Terminal. Commands never pass through a shell. D-Bus
activated applications use gio launch when available, then fall back to Exec.
Files and folders open through xdg-open or gio open; they are not executed as
programs.

History uses atomic replacement at XDG_STATE_HOME/lapi-launcher/recent.toml,
normally ~/.local/state/lapi-launcher/recent.toml. It stores application
identifiers and paths opened through Lapi, never search queries or activity
from other programs.

## CLI

    cargo run -- --list
    cargo run -- --no-images
    cargo run -- --print-default-config
    cargo run -- --help

--list prints catalog aliases without opening the interface, which is useful for
checking discovery in the current desktop session.

The packaged desktop entry is
[resources/lapi-launcher.desktop](resources/lapi-launcher.desktop). It runs Lapi
in the desktop-configured terminal.

## Development

    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    cargo build --release --locked

desktop discovers and launches applications; app handles events and search; ui
renders and records click targets; icons resolves and prepares images; graphics
handles classic Kitty graphics; system, storage, config, and history manage local
state.

Implementation references: [Ratatui](https://docs.rs/ratatui/0.30.2/),
[ratatui-image](https://docs.rs/ratatui-image/11.0.8/ratatui_image/), and the
[Desktop Entry Specification](https://specifications.freedesktop.org/desktop-entry/latest-single/).
Image-protocol behavior follows [Yazi](https://yazi-rs.github.io/docs/image-preview/)
and the [Kitty graphics protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/).
