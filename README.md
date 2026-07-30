# Cloudshell

**简体中文** | [English](./README.en.md)

> 轻量、原生、专为日常远程工作设计的 SSH 与终端客户端。

Cloudshell 使用 Rust 与 [Slint](https://slint.dev) 构建，提供多会话终端、文件传输、端口转发和本地/远端资源监控。它希望保留专业 SSH 客户端的核心工作流，同时避免 JVM 应用常见的高内存占用。

## 为什么使用 Cloudshell

- **原生且轻量**：Rust 二进制、无 GC，适合长期保持多个连接。
- **一个工作区完成远程工作**：终端、SFTP、资源面板和会话管理无需在多个工具间切换。
- **按你的 SSH 习惯工作**：支持密码、OpenSSH/PEM/PPK 私钥、跳板连接和 `~/.ssh/config` 导入。
- **跨平台**：提供 Windows、Linux 与 macOS 构建产物。

## 主要功能

| 能力 | 说明 |
| --- | --- |
| SSH 与终端 | VT/ANSI 终端模拟；支持 `vim`、`htop`、`btop` 等全屏程序和多标签会话。 |
| 会话管理 | 创建、编辑、删除、分组、复制、导入和导出连接配置。 |
| SFTP 与传输 | 浏览远端文件、上传下载、拖拽传输，以及终端内 ZMODEM（`sz`）接收。 |
| 监控 | 查看本机与远端的 CPU、内存、交换、网络、磁盘、进程和系统信息。 |
| 连接方式 | SSH 密码/私钥认证（含加密 PPK v2/v3）、单跳板连接、串口和 Telnet 会话、SOCKS5/HTTP 出站代理。 |
| 隧道 | 本地转发（`-L`）、远程转发（`-R`）和动态 SOCKS5 转发（`-D`）。 |
| 效率工具 | 快捷命令、命令历史，以及向全部在线会话广播输入。 |
| 安全 | 可选询问/严格/自动接受新主机密钥策略；连接测试执行真实认证；会话密码以 ChaCha20-Poly1305 加密保存。 |

## 安装

在 GitHub 的 [Releases](https://github.com/jeff141/cloudshell/releases) 页面下载对应平台的构建包。每个 `v*` 标签均会触发 Windows、Linux 和 macOS 的自动构建。

### Windows

下载 `cloudshell-*-windows-x86_64.zip`，解压后运行 `cloudshell.exe`。

### Linux

```bash
tar -xzf cloudshell-*-linux-x86_64.tar.gz
cd cloudshell-*-linux-x86_64
./cloudshell

# 可选：安装应用菜单和 Dock 图标
chmod +x install-linux.sh && ./install-linux.sh
```

需要 glibc 2.35 或更新版本（例如 Ubuntu 22.04+、Debian 12+）。Wayland 下安装图标后可能需要重新登录一次。

### macOS

```bash
tar -xzf cloudshell-*-macos-*.tar.gz
xattr -dr com.apple.quarantine cloudshell
./cloudshell
```

`aarch64` 对应 Apple 芯片，`x86_64` 对应 Intel 芯片。未签名构建可能需要上述命令移除隔离属性。

## 快速开始

1. 启动 Cloudshell，点击右上角的 **新建会话**。
2. 输入主机地址、端口和用户名，选择密码或私钥认证。
3. 保存后点击会话即可连接；首次连接时，核对并确认主机密钥指纹。
4. 在底部打开 SFTP，在左侧查看资源，在工具栏使用隧道、快捷命令和同步输入。

会话数据保存在本机：

- Windows：`%APPDATA%/cloudshell/sessions.json`
- Linux：`~/.config/cloudshell/sessions.json`
- macOS：`~/Library/Application Support/cloudshell/sessions.json`

## 导入 OpenSSH 配置

如果你已使用 OpenSSH，可在设置中选择 **导入 `~/.ssh/config`**。Cloudshell 会把每个具体的 `Host` 条目创建为会话，读取以下字段：

```sshconfig
Host production
  HostName 10.0.0.5
  User deploy
  Port 2222
  IdentityFile ~/.ssh/id_ed25519
  ProxyJump bastion
```

- 支持 `Host`、`HostName`、`User`、`Port`、`IdentityFile`、单跳 `ProxyJump`。
- `Host *` 等通配规则和不支持的 OpenSSH 指令不会导入。
- 重名，或“相同主机 + 相同用户”的会话会跳过；未设置用户时默认使用 `root`。
- 导入只创建/补充 Cloudshell 会话，不会修改你的 `~/.ssh/config`。

## 从源码运行

前提：Rust 1.75 或更高版本，以及目标平台所需的 GUI 构建依赖。

```bash
cargo run --release
```

常用开发检查：

```bash
cargo check
cargo test
```

## 项目结构

```text
cloudshell/
├── src/                  # 应用状态、连接协议、系统采样与后端逻辑
├── ui/                   # Slint 界面、主题和可复用组件
├── assets/               # 图标、安装脚本与平台元数据
├── lang/                 # 中英文翻译
├── packaging/            # 发行包配置
└── scripts/              # 本地构建辅助脚本
```

## 技术栈

- UI：[Slint](https://slint.dev)
- 异步运行时：[Tokio](https://tokio.rs/)
- SSH：[russh](https://crates.io/crates/russh)
- 系统指标：[sysinfo](https://crates.io/crates/sysinfo)
- 数据序列化：`serde`、`serde_json`

## 路线图

- [ ] 使用系统钥匙串保存会话密码
- [ ] 终端分屏

## 许可证

本项目采用 MIT OR Apache-2.0 双许可证；具体声明见 `Cargo.toml`。
