//! Built-in local-shell session backed by the operating system's native PTY.
//!
//! The public surface mirrors SSH/serial/Telnet so the existing terminal tab,
//! VT parser, keyboard input, resize and reconnect paths can be reused.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::config::Session;
use crate::i18n::t;
use crate::ssh::{SessionCommand, SessionEvent, SessionHandle};

pub fn spawn_local_session(
    runtime: &tokio::runtime::Handle,
    tab_id: String,
    session: Session,
    initial_cols: u32,
    initial_rows: u32,
) -> (SessionHandle, UnboundedReceiver<SessionEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<SessionCommand>();
    let (evt_tx, evt_rx) = mpsc::unbounded_channel::<SessionEvent>();

    let task_events = evt_tx.clone();
    let join = runtime.spawn(async move {
        if let Err(error) = run_local_shell(
            session,
            cmd_rx,
            task_events.clone(),
            initial_cols,
            initial_rows,
        )
        .await
        {
            tracing::warn!("local shell ended with error: {error:#}");
            let _ = task_events.send(SessionEvent::Closed(format!("{error:#}")));
        }
    });

    (
        SessionHandle {
            tab_id,
            commands: cmd_tx,
            join,
        },
        evt_rx,
    )
}

async fn run_local_shell(
    session: Session,
    mut commands: UnboundedReceiver<SessionCommand>,
    events: UnboundedSender<SessionEvent>,
    initial_cols: u32,
    initial_rows: u32,
) -> Result<()> {
    let shell = preferred_shell();
    let shell_name = Path::new(&shell)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("shell")
        .to_string();
    let _ = events.send(SessionEvent::Status(format!(
        "{} {shell_name} ...",
        t("正在启动本地终端", "Starting local terminal")
    )));

    let pty = native_pty_system();
    let pair = pty
        .openpty(pty_size(initial_cols, initial_rows))
        .context("open local pseudo-terminal")?;

    let mut command = CommandBuilder::new(&shell);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    command.env("CLOUDSHELL_LOCAL", "1");
    if let Some(directory) = local_start_directory(&session) {
        command.cwd(directory);
    }

    let mut child = pair
        .slave
        .spawn_command(command)
        .with_context(|| format!("start local shell {}", Path::new(&shell).display()))?;
    // The child owns the slave side now. Keeping our copy open would delay EOF
    // after the shell exits and leave the reader thread blocked.
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .context("clone local PTY reader")?;
    let writer = pair.master.take_writer().context("open local PTY writer")?;
    let writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(writer));
    let master: Arc<Mutex<Box<dyn MasterPty + Send>>> = Arc::new(Mutex::new(pair.master));

    let reader_done = Arc::new(AtomicBool::new(false));
    let reader_error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let read_done = reader_done.clone();
    let read_error = reader_error.clone();
    let read_events = events.clone();
    let reader_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(read) => {
                    if read_events
                        .send(SessionEvent::Output(buf[..read].to_vec()))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => break,
                Err(error) => {
                    *read_error.lock().unwrap() = Some(error.to_string());
                    break;
                }
            }
        }
        read_done.store(true, Ordering::Release);
    });

    let _ = events.send(SessionEvent::Connected);
    let _ = events.send(SessionEvent::Status(format!(
        "{} {shell_name}",
        t("本地终端", "Local terminal")
    )));

    if !session.startup_command.trim().is_empty() {
        let mut startup = session.startup_command.trim().as_bytes().to_vec();
        startup.push(b'\r');
        let mut output = writer.lock().unwrap();
        output
            .write_all(&startup)
            .and_then(|_| output.flush())
            .context("send local startup command")?;
    }

    let mut timer = tokio::time::interval(Duration::from_millis(100));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut kill_child = false;
    let close_reason = loop {
        tokio::select! {
            command = commands.recv() => {
                match command {
                    Some(SessionCommand::RawInput(bytes)) => {
                        // Never log input bytes: terminal input may contain secrets.
                        let result = {
                            let mut output = writer.lock().unwrap();
                            output.write_all(&bytes).and_then(|_| output.flush())
                        };
                        if let Err(error) = result {
                            kill_child = true;
                            break format!("{}: {error}", t("本地终端写入失败", "local terminal write failed"));
                        }
                    }
                    Some(SessionCommand::Resize(cols, rows)) => {
                        if let Err(error) = master.lock().unwrap().resize(pty_size(cols, rows)) {
                            tracing::debug!("local PTY resize failed: {error:#}");
                        }
                    }
                    Some(SessionCommand::SetProcessMonitor(_)) => {}
                    Some(SessionCommand::RequestSystemInfo(_)) => {}
                    Some(SessionCommand::Close) | None => {
                        kill_child = true;
                        break t("本地终端已关闭", "local terminal closed").to_string();
                    }
                }
            }
            _ = timer.tick() => {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        break format!(
                            "{} ({status})",
                            t("本地 Shell 已退出", "local shell exited")
                        );
                    }
                    Ok(None) => {
                        if reader_done.load(Ordering::Acquire) {
                            if let Some(error) = reader_error.lock().unwrap().take() {
                                kill_child = true;
                                break format!(
                                    "{}: {error}",
                                    t("本地终端读取失败", "local terminal read failed")
                                );
                            }
                        }
                    }
                    Err(error) => {
                        kill_child = true;
                        break format!(
                            "{}: {error}",
                            t("无法获取本地 Shell 状态", "failed to query local shell")
                        );
                    }
                }
            }
        }
    };

    if kill_child {
        let _ = child.kill();
        let _ = child.wait();
    }
    drop(writer);
    drop(master);
    let _ = tokio::task::spawn_blocking(move || reader_thread.join()).await;
    let _ = events.send(SessionEvent::Closed(close_reason));
    Ok(())
}

fn pty_size(cols: u32, rows: u32) -> PtySize {
    PtySize {
        rows: rows.clamp(1, u16::MAX as u32) as u16,
        cols: cols.clamp(1, u16::MAX as u32) as u16,
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn local_start_directory(session: &Session) -> Option<OsString> {
    if !session.initial_directory.trim().is_empty() {
        return Some(session.initial_directory.trim().into());
    }
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
}

#[cfg(windows)]
fn preferred_shell() -> OsString {
    std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into())
}

#[cfg(not(windows))]
fn preferred_shell() -> OsString {
    std::env::var_os("SHELL")
        .filter(|shell| !shell.is_empty())
        .unwrap_or_else(|| {
            if cfg!(target_os = "macos") {
                "/bin/zsh".into()
            } else {
                "/bin/sh".into()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pty_size_is_never_zero_and_caps_at_u16() {
        let size = pty_size(0, u32::MAX);
        assert_eq!(size.cols, 1);
        assert_eq!(size.rows, u16::MAX);
    }

    #[test]
    fn explicit_start_directory_wins() {
        let mut session = Session::local();
        session.initial_directory = "/tmp/cloudshell-local".into();
        assert_eq!(
            local_start_directory(&session),
            Some(OsString::from("/tmp/cloudshell-local"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn local_session_runs_a_command_through_the_pty() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mut session = Session::local();
        session.startup_command = "printf '\\n__cloudshell_local_pty_ok__\\n'; exit".to_string();
        let (_handle, mut events) =
            spawn_local_session(runtime.handle(), "local-test".into(), session, 80, 24);

        let output = runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(8), async {
                let mut output = Vec::new();
                while let Some(event) = events.recv().await {
                    match event {
                        SessionEvent::Output(bytes) => output.extend(bytes),
                        SessionEvent::Closed(_) => break,
                        _ => {}
                    }
                }
                output
            })
            .await
            .expect("local PTY did not exit in time")
        });

        assert!(
            String::from_utf8_lossy(&output).contains("__cloudshell_local_pty_ok__"),
            "local PTY did not return command output: {}",
            String::from_utf8_lossy(&output)
        );
    }
}
