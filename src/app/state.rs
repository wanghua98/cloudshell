//! Shared runtime state for the application coordinator.
//!
//! Keeping these data-only types outside the callback wiring makes the UI
//! coordinator smaller and gives subsequent feature work a stable boundary.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::ssh::ProcInfo;
use crate::system::SystemSnapshot;

/// A rendered terminal row and its highlighted spans.
pub(crate) type Line = (String, Vec<super::HistSpan>);

/// Per-terminal VT parser state, selection state, and scrollback.
pub(crate) struct TermBuffer {
    pub(crate) parser: vt100::Parser,
    pub(crate) find_query: String,
    pub(crate) is_dark: bool,
    pub(crate) sel_anchor: Option<(usize, u16)>,
    pub(crate) sel_focus: Option<(usize, u16)>,
    pub(crate) history: Vec<Line>,
    pub(crate) prev: Vec<Line>,
    pub(crate) view_offset: usize,
    pub(crate) displayed_text: Vec<String>,
    pub(crate) csi_state: CsiState,
}

/// Minimal CSI-final-byte rewriter state, persisted across read chunks.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum CsiState {
    Normal,
    Esc,
    Csi,
}

pub(crate) type TermBuffers = Arc<Mutex<HashMap<String, TermBuffer>>>;

pub(crate) type SftpHandles = Arc<Mutex<HashMap<String, crate::sftp::SftpHandle>>>;
pub(crate) type SftpLastCwd = Arc<Mutex<HashMap<String, String>>>;

/// Per-tab connection status and the latest remote resource sample.
#[derive(Clone, Default)]
pub(crate) struct TabStatus {
    pub(crate) host: String,
    pub(crate) session_id: String,
    pub(crate) state: u8,
    pub(crate) cpu: f32,
    pub(crate) mem_used_kib: u64,
    pub(crate) mem_total_kib: u64,
    pub(crate) swap_used_kib: u64,
    pub(crate) swap_total_kib: u64,
    pub(crate) net: Vec<(String, u64, u64)>,
    pub(crate) selected_iface: String,
    pub(crate) net_hist: Vec<f32>,
    pub(crate) disks: Vec<(String, u64, u64)>,
    pub(crate) procs: Vec<ProcInfo>,
}

pub(crate) type TabStatuses = Arc<Mutex<HashMap<String, TabStatus>>>;
pub(crate) type LocalSnap = Arc<Mutex<SystemSnapshot>>;
