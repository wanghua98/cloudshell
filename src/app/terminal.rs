//! Terminal input encoding and path helpers.
//!
//! These functions are deliberately UI-framework independent so they can be
//! tested without a Slint event loop.

/// Normalize clipboard line endings to the carriage return expected by PTYs.
pub(super) fn normalize_pasted_newlines(text: &str) -> String {
    text.replace("\r\n", "\r").replace('\n', "\r")
}

/// Convert a Slint key event into the byte sequence expected by a terminal.
pub(super) fn key_to_pty_bytes(key: &str, ctrl: bool, alt: bool, app_cursor: bool) -> Vec<u8> {
    let special: Option<&[u8]> = match key {
        "\u{F700}" => Some(if app_cursor { b"\x1bOA" } else { b"\x1b[A" }),
        "\u{F701}" => Some(if app_cursor { b"\x1bOB" } else { b"\x1b[B" }),
        "\u{F702}" => Some(if app_cursor { b"\x1bOD" } else { b"\x1b[D" }),
        "\u{F703}" => Some(if app_cursor { b"\x1bOC" } else { b"\x1b[C" }),
        "\u{F729}" => Some(b"\x1b[H"),
        "\u{F72B}" => Some(b"\x1b[F"),
        "\u{F72C}" => Some(b"\x1b[5~"),
        "\u{F72D}" => Some(b"\x1b[6~"),
        "\u{F728}" => Some(b"\x1b[3~"),
        "\u{F704}" => Some(b"\x1bOP"),
        "\u{F705}" => Some(b"\x1bOQ"),
        "\u{F706}" => Some(b"\x1bOR"),
        "\u{F707}" => Some(b"\x1bOS"),
        "\u{F708}" => Some(b"\x1b[15~"),
        "\u{F709}" => Some(b"\x1b[17~"),
        "\u{F70A}" => Some(b"\x1b[18~"),
        "\u{F70B}" => Some(b"\x1b[19~"),
        "\u{F70C}" => Some(b"\x1b[20~"),
        "\u{F70D}" => Some(b"\x1b[21~"),
        "\u{F70E}" => Some(b"\x1b[23~"),
        "\u{F70F}" => Some(b"\x1b[24~"),
        _ => None,
    };
    if let Some(sequence) = special {
        return sequence.to_vec();
    }
    if key == "\u{0008}" {
        return vec![0x7f];
    }
    if key == "\n" && !ctrl && !alt {
        return vec![0x0d];
    }
    let mut characters = key.chars();
    let first = characters.next();
    let single = first.filter(|_| characters.next().is_none());
    let Some(character) = first else {
        return Vec::new();
    };
    let codepoint = character as u32;
    if !ctrl && single.is_some() && (0x10..=0x18).contains(&codepoint) {
        return Vec::new();
    }
    if ctrl && single.is_some() {
        if (0x01..=0x1f).contains(&codepoint) {
            return vec![codepoint as u8];
        }
        if let Some(byte) = match character.to_ascii_uppercase() as u8 {
            b'A'..=b'Z' => Some(character.to_ascii_uppercase() as u8 - b'A' + 1),
            b'[' => Some(0x1b),
            b'\\' => Some(0x1c),
            b']' => Some(0x1d),
            b'^' => Some(0x1e),
            b'_' => Some(0x1f),
            b'@' => Some(0),
            _ => None,
        } {
            return vec![byte];
        }
    }
    if key
        .chars()
        .any(|value| (0xE000..=0xF8FF).contains(&(value as u32)))
    {
        return Vec::new();
    }
    if alt && !ctrl {
        let mut bytes = Vec::with_capacity(key.len() + 1);
        bytes.push(0x1b);
        bytes.extend_from_slice(key.as_bytes());
        return bytes;
    }
    key.as_bytes().to_vec()
}

/// Return the parent directory of a POSIX path.
pub(super) fn parent_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(0) | None => "/".to_owned(),
        Some(index) => trimmed[..index].to_owned(),
    }
}
