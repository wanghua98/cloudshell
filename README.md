# Cloudshell

**简体中文** | [English](./README.en.md)
[![OctoCounts](https://api.octocounts.com/badge/wanghua98/cloudshell/branch/master)](https://octocounts.com/github/wanghua98/cloudshell/tree/master)
> 轻量、原生的跨平台 SSH、SFTP 与终端客户端。

Cloudshell 使用 Rust 和 [Slint](https://slint.dev) 构建，将多标签终端、文件传输、隧道和远程主机监控整合到一个桌面应用中。它面向日常服务器运维、开发和网络设备管理，力求提供专业 SSH 客户端常用的工作流，同时保持原生应用的轻量体验。

## 功能一览

| 类别 | 功能 |
| --- | --- |
| 终端与会话 | 支持多标签 VT/ANSI 终端，可运行 `vim`、`htop`、`btop` 等全屏程序；会话可创建、分组、复制、导入、导出与连接测试。 |
| 连接方式 | SSH、Telnet 与串口会话；SSH 支持密码、OpenSSH/PEM 和加密 PuTTY PPK v2/v3 私钥。 |
| SSH 网络能力 | 单跳跳板机、SOCKS5/HTTP 出站代理、连接超时与保活；支持本地（`-L`）、远程（`-R`）和动态 SOCKS5（`-D`）转发。 |
| SFTP 与传输 | 浏览远端文件、上传、下载、拖拽上传；支持新建目录/文件、重命名、权限修改和多选操作，以及终端内 ZMODEM（`sz`）接收。 |
| 系统信息 | 查看本机与远端的 CPU、内存、交换、网络、磁盘、进程和系统概况。 |
| 效率与界面 | 快捷命令、命令历史、向在线会话同步输入与上传；中英文界面、深浅色主题、界面缩放和终端字体设置。 |
| 安全 | 主机密钥提供询问、严格和仅接受新密钥策略；保存的会话密码使用 ChaCha20-Poly1305 加密。 |

## 安装

请从 [GitHub Releases](https://github.com/wanghua98/cloudshell/releases) 下载对应平台的构建包。

### Windows

下载 Windows 压缩包，解压后运行 `cloudshell.exe`。

### Linux

下载并解压 Linux 发布包后运行二进制文件：

```bash
tar -xzf cloudshell-*-linux-*.tar.gz
cd cloudshell-*-linux-*
./cloudshell

# 可选：安装应用启动器与 Dock 图标
chmod +x install-linux.sh && ./install-linux.sh
```

二进制发布包需要 glibc 2.35 或更新版本，例如 Ubuntu 22.04+ 或 Debian 12+。在 Wayland 桌面环境中，安装图标后可能需要重新登录。

### macOS

下载与处理器匹配的发布包：`aarch64` 对应 Apple 芯片，`x86_64` 对应 Intel 芯片。若系统阻止打开未签名版本，可在解压目录运行：

```bash
xattr -dr com.apple.quarantine cloudshell
./cloudshell
```

## 快速开始

1. 启动 Cloudshell，点击右上角的“新建会话”。
2. 选择 SSH、Telnet 或串口；对于 SSH，填写主机、端口、用户并选择密码或私钥认证。
3. 保存后点击会话连接。首次 SSH 连接时，请核对主机密钥指纹。
4. 通过底部面板使用 SFTP，通过资源侧栏查看主机信息；在会话高级设置中配置代理、跳板机和端口转发。

## 导入 OpenSSH 配置

在设置中选择导入 `~/.ssh/config`，Cloudshell 会为每个具体的 `Host` 条目创建会话。例如：

```sshconfig
Host production
  HostName 10.0.0.5
  User deploy
  Port 2222
  IdentityFile ~/.ssh/id_ed25519
  ProxyJump bastion
```

支持 `Host`、`HostName`、`User`、`Port`、`IdentityFile` 和单跳 `ProxyJump`。`Host *` 等通配规则及其他不支持的 OpenSSH 指令不会导入；导入不会修改原有的 `~/.ssh/config`。

## 本地数据

会话与界面设置仅保存在本机：

| 平台 | 配置文件 |
| --- | --- |
| Windows | `%APPDATA%/cloudshell/sessions.json` |
| Linux | `~/.config/cloudshell/sessions.json` |
| macOS | `~/Library/Application Support/cloudshell/sessions.json` |

同一目录中的 `secret.key` 用于加密已保存的会话密码。请妥善备份配置，并不要将该目录提交到版本控制系统。

## 从源码构建

前提：Rust 1.75 或更高版本，以及目标操作系统所需的图形界面构建依赖。

```bash
git clone https://github.com/wanghua98/cloudshell.git
cd cloudshell
cargo run --release
```

常用检查：

```bash
cargo check
cargo test
```

查看已构建版本：

```bash
cargo run -- --version
```

## 项目结构

```text
cloudshell/
├── src/          # 应用状态、SSH/SFTP/串口/Telnet 协议与系统采样
├── ui/           # Slint 界面、主题和可复用组件
├── assets/       # 图标、Linux 安装脚本与平台元数据
├── lang/         # 中英文翻译
├── packaging/    # 发行包配置（含 AUR）
└── scripts/      # 本地构建与发布辅助脚本
```

## 技术栈

- UI：[Slint](https://slint.dev)
- 异步运行时：[Tokio](https://tokio.rs/)
- SSH：[russh](https://crates.io/crates/russh)
- SFTP：[russh-sftp](https://crates.io/crates/russh-sftp)
- 系统指标：[sysinfo](https://crates.io/crates/sysinfo)

## 许可证

本项目采用 MIT 或 Apache-2.0 双许可证，具体声明见 [Cargo.toml](./Cargo.toml)。
