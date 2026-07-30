# Cloudshell

[简体中文](./README.md) | **English**

> A lightweight, native, cross-platform SSH, SFTP, and terminal client.

Cloudshell is built with Rust and [Slint](https://slint.dev). It brings tabbed terminals, file transfer, tunnels, and remote-host monitoring into one desktop application. It is designed for everyday server administration, development, and network-device management, with the familiar workflow of a capable SSH client in a lightweight native app.

## Features

| Area | Included |
| --- | --- |
| Terminal & sessions | Tabbed VT/ANSI terminals for full-screen tools such as `vim`, `htop`, and `btop`; create, group, duplicate, import, export, and test sessions. |
| Connection types | SSH, Telnet, and serial sessions. SSH supports passwords plus OpenSSH/PEM and encrypted PuTTY PPK v2/v3 private keys. |
| SSH networking | Single-hop jump hosts, SOCKS5/HTTP outbound proxies, connection timeouts and keepalives; local (`-L`), remote (`-R`), and dynamic SOCKS5 (`-D`) forwarding. |
| SFTP & transfer | Remote file browsing, upload, download, drag-and-drop upload, directory/file creation, rename, permission changes, multi-select actions, and in-terminal ZMODEM (`sz`) receive. |
| System information | Local and remote CPU, memory, swap, network, disk, process, and system overviews. |
| Productivity & UI | Quick commands, command history, synchronized input and uploads across online sessions, Chinese/English UI, light/dark themes, UI scaling, and terminal font settings. |
| Security | Ask, strict, and accept-new host-key policies; saved session passwords are encrypted with ChaCha20-Poly1305. |

## Install

Download the package for your platform from [GitHub Releases](https://github.com/wanghua98/cloudshell/releases).

### Windows

Download the Windows archive, extract it, and run `cloudshell.exe`.

### Linux

Download and extract the Linux release archive, then run the binary:

```bash
tar -xzf cloudshell-*-linux-*.tar.gz
cd cloudshell-*-linux-*
./cloudshell

# Optional: install the application launcher and Dock icon
chmod +x install-linux.sh && ./install-linux.sh
```

The binary release requires glibc 2.35 or newer, such as Ubuntu 22.04+ or Debian 12+. Under Wayland, you may need to log out and back in after installing the icon.

### macOS

Choose the build for your processor: `aarch64` for Apple silicon or `x86_64` for Intel. If macOS blocks an unsigned build, run this from the extracted directory:

```bash
xattr -dr com.apple.quarantine cloudshell
./cloudshell
```

## Quick start

1. Launch Cloudshell and select **New Session** in the upper-right corner.
2. Choose SSH, Telnet, or Serial. For SSH, enter the host, port, and user, then select password or private-key authentication.
3. Save and click the session to connect. On the first SSH connection, verify the host-key fingerprint.
4. Use the bottom panel for SFTP and the resource sidebar for host information. Configure proxies, jump hosts, and port forwarding in the session's advanced settings.

## Import an OpenSSH configuration

Select **Import `~/.ssh/config`** in Settings. Cloudshell creates a session for each concrete `Host` entry, for example:

```sshconfig
Host production
  HostName 10.0.0.5
  User deploy
  Port 2222
  IdentityFile ~/.ssh/id_ed25519
  ProxyJump bastion
```

Supported fields are `Host`, `HostName`, `User`, `Port`, `IdentityFile`, and single-hop `ProxyJump`. Wildcard rules such as `Host *` and other unsupported OpenSSH directives are not imported. Your existing `~/.ssh/config` is never modified.

## Local data

Sessions and UI settings remain on the local machine:

| Platform | Configuration file |
| --- | --- |
| Windows | `%APPDATA%/cloudshell/sessions.json` |
| Linux | `~/.config/cloudshell/sessions.json` |
| macOS | `~/Library/Application Support/cloudshell/sessions.json` |

`secret.key` in the same directory encrypts saved session passwords. Back up the configuration carefully and do not commit that directory to version control.

## Build from source

Requirements: Rust 1.75 or later, plus the GUI build dependencies required by your target operating system.

```bash
git clone https://github.com/wanghua98/cloudshell.git
cd cloudshell
cargo run --release
```

Useful checks:

```bash
cargo check
cargo test
```

To display the built version:

```bash
cargo run -- --version
```

## Repository layout

```text
cloudshell/
├── src/          # application state, SSH/SFTP/serial/Telnet, and system sampling
├── ui/           # Slint screens, themes, and reusable components
├── assets/       # icons, Linux installer, and platform metadata
├── lang/         # Chinese and English translations
├── packaging/    # distribution packaging, including AUR
└── scripts/      # local build and release helpers
```

## Stack

- UI: [Slint](https://slint.dev)
- Async runtime: [Tokio](https://tokio.rs/)
- SSH: [russh](https://crates.io/crates/russh)
- SFTP: [russh-sftp](https://crates.io/crates/russh-sftp)
- System metrics: [sysinfo](https://crates.io/crates/sysinfo)

## License

Dual-licensed under MIT or Apache-2.0. See [Cargo.toml](./Cargo.toml) for the declaration.
