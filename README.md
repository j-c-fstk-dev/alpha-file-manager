# Alpha File Manager

[Português (Brasil)](README.pt-BR.md)

A lightweight Linux file manager written in Rust with native GTK 3 widgets.

![Mountains and aurora in the default Alpha File Manager banner](assets/banner.jpg)

## Features

- Tabs, split view, an address bar, and per-pane navigation history.
- A collapsible sidebar with standard folders and pins shared with the home page.
- A customizable panoramic banner and four color profiles, with optional window transparency.
- List and icon views, image thumbnails, file type icons, and sorting by name, type, size, or date.
- A header search field: type a path to open it, or text to filter the current folder.
- Drag and drop within a pane, between panes, and to or from other apps. Items move on the same
  disk and copy across disks; hold Ctrl to copy or Shift to move.
- Fast loading of large folders, read in the background without blocking the window.
- Header and context menus for file operations, navigation, and view controls.
- A per-user application launcher and default file manager integration for Xfce.
- Automatic English and Portuguese localization based on the system language.

Creation dates are shown when the filesystem makes them available. Desktop transparency
requires a compositor. When enabled, the opacity slider ranges from 60% to 100%.

## Installation

Download the Linux x86-64 archive from [GitHub Releases](https://github.com/j-c-fstk-dev/alpha-file-manager/releases).
Extract it and run `./install-launcher.sh` from the extracted directory. The binary is built on
Kali Rolling and requires the GTK 3 runtime and glibc 2.34 or later. Build from source for other
architectures or if the binary is incompatible with your distribution.

## Build from source

Install Rust, a system compiler, pkg-config, and the GTK 3 development headers. On Kali and Debian:

```sh
sudo apt install build-essential pkg-config libgtk-3-dev
cargo build --release --locked
./target/release/alpha-file-manager
```

Run `./install-launcher.sh` to register the app for your account. Pass a directory path to open it directly.

## Usage

The header menus show available shortcuts. Press **Ctrl+K** to focus the search field. Use the context menu to access commands for the
selected files or folder. To pin a folder, use **Add Folder** on the home page or **Pin Folder
to Home and Sidebar** in its context menu. Right-click a pin to remove it.

Use the image button to change the home banner, or open **Settings** to change the color
profile, adjust transparency, or choose Alpha as the default file manager.

## License

GNU Affero General Public License version 3. See [LICENSE](LICENSE).
