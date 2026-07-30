# Cloudshell

[简体中文](./README.md) | **English**

> A lightweight, native SSH and terminal client for everyday remote work.

Cloudshell is built with Rust and [Slint](https://slint.dev). It brings tabbed terminals, file transfer, port forwarding, and local/remote monitoring into one workspace, without the memory footprint typical of JVM-based clients.

## Why Cloudshell

- **Native and lightweight** — a Rust binary with no garbage collector, designed for keeping several connections open.
- **One remote-work workspace** — terminal, SFTP, resource panels, and session management stay together.
- **Works with your SSH setup** — password, private-key, encrypted-key, and `~/.ssh/config` workflows are supported.
- **Cross-platform** — builds are available for Windows, Linux, and macOS.

## Highlights

| Area | What it provides |
| --- | --- |
| SSH & terminal | VT/ANSI terminal emulation, full-screen programs such as `vim`, `htop`, and `btop`, and multiple tabs. |
| Session management | Create, edit, delete, group, duplicate, import, and export connections. |
| SFTP & transfer | Browse remote files, upload/download, drag-and-drop transfer, and in-terminal ZMODEM (`sz`) receive. |
| Monitoring | Local and remote CPU, memory, swap, network, disk, process, and system information. |
| Connectivity | SSH password/private-key authentication, serial and Telnet sessions, SOCKS5/HTTP outbound proxies. |
| Tunnels | Local (`-L`), remote (`-R`), and dynamic SOCKS5 (`-D`) forwarding. |
| Productivity | Quick commands, command history, and synchronized input to all online sessions. |
| Security | First-use host-key confirmation, changed-key warnings, and ChaCha20-Poly1305 encrypted saved passwords. |

## Install

Download the package for your platform from [Releases](https://github.com/jeff141/cloudshell/releases). Every `v*` tag triggers automated Windows, Linux, and macOS builds.

### Windows

Download `cloudshell-*-windows-x86_64.zip`, extract it, then run `cloudshell.exe`.

### Linux

```bash
tar -xzf cloudshell-*-linux-x86_64.tar.gz
cd cloudshell-*-linux-x86_64
./cloudshell

# Optional: install the launcher and Dock icon
chmod +x install-linux.sh && ./install-linux.sh
```

glibc 2.35 or later is required (for example, Ubuntu 22.04+ or Debian 12+). On Wayland, logging out and back in may be necessary after icon installation.

### macOS

```bash
tar -xzf cloudshell-*-macos-*.tar.gz
xattr -dr com.apple.quarantine cloudshell
./cloudshell
```

Use the `aarch64` build for Apple silicon and `x86_64` for Intel Macs. The command above clears the quarantine attribute for unsigned builds.

## Quick start

1. Launch Cloudshell and choose **New Session** in the upper right.
2. Enter the host, port, and user, then select password or private-key authentication.
3. Save and connect. On the first connection, verify and accept the host-key fingerprint.
4. Use the bottom panel for SFTP, the sidebar for resources, and the toolbar for tunnels, quick commands, and synchronized input.

Sessions are stored locally:

- Windows: `%APPDATA%/cloudshell/sessions.json`
- Linux: `~/.config/cloudshell/sessions.json`
- macOS: `~/Library/Application Support/cloudshell/sessions.json`

## Import an OpenSSH configuration

If you already use OpenSSH, choose **Import `~/.ssh/config`** from Settings. Each concrete `Host` entry becomes a Cloudshell session, using the following fields:

```sshconfig
Host production
  HostName 10.0.0.5
  User deploy
  Port 2222
  IdentityFile ~/.ssh/id_ed25519
```

- Supported: `Host`, `HostName`, `User`, `Port`, and `IdentityFile`.
- Wildcard rules such as `Host *`, along with unsupported OpenSSH directives, are not imported.
- Existing aliases and entries with the same host + user are skipped. A missing user defaults to `root`.
- Importing only creates or supplements Cloudshell sessions; it never changes `~/.ssh/config`.

## Run from source

Requires Rust 1.75 or later and the GUI build dependencies for your platform.

```bash
cargo run --release
```

Useful development checks:

```bash
cargo check
cargo test
```

## Repository layout

```text
cloudshell/
├── src/                  # state, protocol, system sampling, and backend logic
├── ui/                   # Slint screens, theme, and reusable components
├── assets/               # icons, install scripts, and platform metadata
├── lang/                 # Chinese and English translations
├── packaging/            # distribution packaging
└── scripts/              # local build helpers
```

## Stack

- UI: [Slint](https://slint.dev)
- Async runtime: [Tokio](https://tokio.rs/)
- SSH: [russh](https://crates.io/crates/russh)
- System metrics: [sysinfo](https://crates.io/crates/sysinfo)
- Serialization: `serde`, `serde_json`

## Roadmap

- [ ] Store session passwords in the system keychain
- [ ] Split terminal panes

## License

Dual-licensed under MIT OR Apache-2.0; see `Cargo.toml` for the declaration.
