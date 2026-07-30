//! Pure adapters from domain data to Slint models.
//!
//! Keeping model construction separate from callback wiring makes UI updates
//! easier to test and keeps allocation work visible at the boundary.

use std::rc::Rc;

use slint::{Model, ModelRc, SharedString, VecModel};

use super::{DiskInfo, ProcRow, QuickCmd, SftpEntry, TermMatch, TerminalState};
use crate::config::{ConfigStore, PortForward};
use crate::ssh::{format_size, ProcInfo};

pub(super) fn normalized_history(buf: &[f32]) -> ModelRc<f32> {
    let max = buf.iter().copied().fold(1.0_f32, f32::max);
    let scaled: Vec<f32> = buf
        .iter()
        .map(|value| (value / max).clamp(0.0, 1.0))
        .collect();
    ModelRc::from(Rc::new(VecModel::from(scaled)))
}

pub(super) fn disk_model(disks: &[(String, u64, u64)]) -> ModelRc<DiskInfo> {
    let rows: Vec<DiskInfo> = disks
        .iter()
        .map(|(mount, avail, total)| {
            let used = total.saturating_sub(*avail);
            let percent = if *total > 0 {
                used as f32 / *total as f32
            } else {
                0.0
            };
            DiskInfo {
                path: mount.clone().into(),
                detail: format!("{}/{}", format_size(*avail), format_size(*total)).into(),
                percent,
            }
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

pub(super) fn proc_model(procs: &[ProcInfo]) -> ModelRc<ProcRow> {
    let rows: Vec<ProcRow> = procs
        .iter()
        .map(|process| ProcRow {
            pid: process.pid.to_string().into(),
            user: process.user.clone().into(),
            cpu: format!("{:.1}", process.cpu).into(),
            mem: format!("{:.1}", process.mem).into(),
            command: process.command.clone().into(),
            cpu_frac: (process.cpu / 100.0).clamp(0.0, 1.0),
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

pub(super) fn all_quick_group_names(store: &ConfigStore) -> std::collections::HashSet<String> {
    let commands = store.quick_commands();
    let mut groups: std::collections::HashSet<String> =
        store.quick_groups().iter().cloned().collect();
    if commands
        .iter()
        .any(|command| command.group.trim().is_empty())
    {
        groups.insert("default".to_owned());
    }
    groups.extend(
        commands
            .iter()
            .map(|command| command.group.trim())
            .filter(|group| !group.is_empty())
            .map(str::to_owned),
    );
    groups
}

pub(super) fn quick_cmd_model(
    store: &ConfigStore,
    collapsed_groups: &std::collections::HashSet<String>,
) -> ModelRc<QuickCmd> {
    let commands = store.quick_commands();
    let has_default = commands
        .iter()
        .any(|command| command.group.trim().is_empty());
    let mut named: Vec<String> = store
        .quick_groups()
        .iter()
        .cloned()
        .chain(
            commands
                .iter()
                .map(|command| command.group.trim().to_owned())
                .filter(|group| !group.is_empty()),
        )
        .collect();
    named.sort_by_key(|group| group.to_lowercase());
    named.dedup();

    let mut groups = Vec::with_capacity(named.len() + usize::from(has_default));
    if has_default {
        groups.push("default".to_owned());
    }
    groups.extend(named);

    let mut rows = Vec::new();
    for group in &groups {
        let collapsed = collapsed_groups.contains(group);
        let mut members = commands
            .iter()
            .enumerate()
            .filter(|(_, command)| match group.as_str() {
                "default" => command.group.trim().is_empty(),
                _ => command.group.trim() == group,
            })
            .peekable();
        if members.peek().is_none() {
            rows.push(QuickCmd {
                name: "".into(),
                command: "".into(),
                group: group.clone().into(),
                group_header: group.clone().into(),
                collapsed,
                orig_index: -1,
            });
            continue;
        }
        rows.extend(
            members
                .enumerate()
                .map(|(index, (original_index, command))| QuickCmd {
                    name: command.name.clone().into(),
                    command: command.command.clone().into(),
                    group: group.clone().into(),
                    group_header: if index == 0 {
                        group.clone().into()
                    } else {
                        "".into()
                    },
                    collapsed,
                    orig_index: original_index as i32,
                }),
        );
    }
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

pub(super) fn forward_model(forwards: &[PortForward]) -> ModelRc<super::PortFwd> {
    let rows: Vec<super::PortFwd> = forwards
        .iter()
        .map(|forward| {
            let bind = if forward.bind_addr.trim().is_empty() {
                "127.0.0.1"
            } else {
                forward.bind_addr.trim()
            };
            let summary = match forward.kind.as_str() {
                "local" => format!(
                    "-L {bind}:{} → {}:{}",
                    forward.bind_port, forward.host, forward.host_port
                ),
                "remote" => format!(
                    "-R {bind}:{} → {}:{}",
                    forward.bind_port, forward.host, forward.host_port
                ),
                "dynamic" => format!("-D {bind}:{} (SOCKS5)", forward.bind_port),
                _ => String::new(),
            };
            super::PortFwd {
                kind: forward.kind.clone().into(),
                name: forward.name.clone().into(),
                summary: summary.into(),
            }
        })
        .collect();
    ModelRc::from(Rc::new(VecModel::from(rows)))
}

pub(super) fn selected_sftp_paths(
    terminals: &VecModel<TerminalState>,
    tab_id: &str,
) -> Vec<String> {
    for index in 0..terminals.row_count() {
        let Some(row) = terminals.row_data(index) else {
            continue;
        };
        if row.id.as_str() != tab_id {
            continue;
        }
        return row
            .sftp_entries
            .as_any()
            .downcast_ref::<VecModel<SftpEntry>>()
            .map(|entries| {
                (0..entries.row_count())
                    .filter_map(|entry_index| entries.row_data(entry_index))
                    .filter(|entry| entry.selected)
                    .map(|entry| entry.full_path.to_string())
                    .collect()
            })
            .unwrap_or_default();
    }
    Vec::new()
}

pub(super) fn clear_sftp_selection(terminals: &VecModel<TerminalState>, tab_id: &str) {
    for index in 0..terminals.row_count() {
        let Some(mut row) = terminals.row_data(index) else {
            continue;
        };
        if row.id.as_str() != tab_id {
            continue;
        }
        if let Some(entries) = row
            .sftp_entries
            .as_any()
            .downcast_ref::<VecModel<SftpEntry>>()
        {
            for entry_index in 0..entries.row_count() {
                if let Some(mut entry) = entries.row_data(entry_index) {
                    if entry.selected {
                        entry.selected = false;
                        entries.set_row_data(entry_index, entry);
                    }
                }
            }
        }
        row.sftp_selected_count = 0;
        terminals.set_row_data(index, row);
        return;
    }
}

pub(super) fn history_model(store: &ConfigStore) -> ModelRc<SharedString> {
    ModelRc::from(Rc::new(VecModel::from(
        store
            .command_history()
            .iter()
            .cloned()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    )))
}

pub(super) fn history_view_model(store: &ConfigStore, query: &str) -> ModelRc<SharedString> {
    let query = query.trim().to_lowercase();
    ModelRc::from(Rc::new(VecModel::from(
        store
            .command_history()
            .iter()
            .filter(|command| query.is_empty() || command.to_lowercase().contains(&query))
            .cloned()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    )))
}

pub(super) fn find_matches(rows: &[String], query: &str) -> Vec<TermMatch> {
    let query: Vec<char> = query
        .chars()
        .map(|character| character.to_ascii_lowercase())
        .collect();
    if query.is_empty() {
        return Vec::new();
    }
    let mut matches = Vec::new();
    for (row, line) in rows.iter().enumerate() {
        let lower: Vec<char> = line
            .chars()
            .map(|character| character.to_ascii_lowercase())
            .collect();
        let mut column = 0;
        while column + query.len() <= lower.len() {
            if lower[column..column + query.len()] == query[..] {
                matches.push(TermMatch {
                    row: row as i32,
                    col: column as i32,
                    len: query.len() as i32,
                });
                column += query.len();
            } else {
                column += 1;
            }
        }
    }
    matches
}
