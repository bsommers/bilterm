//! Single-pass VT control sequence scanner and CSI / extended SGR handlers (`ARCH-R04`, `P2.2`).

use super::graphics::strip_apc_and_dcs_envelopes;
use super::snapshot::{RgbTriplet, UnderlineKind};
use super::VtParser;

impl VtParser {
    pub(super) fn record_escape_sequence(&mut self, seq: impl Into<String>) {
        if self.recent_escape_sequences.len() >= 16 {
            self.recent_escape_sequences.remove(0);
        }
        self.recent_escape_sequences.push(seq.into());
    }

    /// Scans raw bytes in a single `ESC` (`\x1b`) pass for `CSI` (`\x1b[`), `OSC` (`\x1b]`),
    /// `DCS` (`\x1bP`), and `APC` (`\x1b_`) control sequences (`P2.2`).
    pub(super) fn scan_control_sequences(&mut self, bytes: &[u8]) {
        if !bytes.contains(&0x1b) {
            return;
        }
        let text = String::from_utf8_lossy(bytes);
        for (idx, _) in text.match_indices('\x1b') {
            let after_esc = &text[idx + 1..];
            let Some(&kind) = after_esc.as_bytes().first() else {
                break;
            };
            let payload_rest = &after_esc[1..];
            match kind {
                b'[' => self.scan_csi_sequence(payload_rest),
                b']' => self.scan_osc_sequence(&text[idx..], payload_rest),
                b'P' => self.scan_dcs_sequence(payload_rest),
                b'_' => self.scan_apc_sequence(payload_rest),
                _ => {}
            }
        }
    }

    /// Dispatches a single `CSI` (`\x1b[...`) sequence starting at `csi_rest` (`&text[idx + 2..]`).
    fn scan_csi_sequence(&mut self, csi_rest: &str) {
        if let Some(rest) = csi_rest.strip_prefix('?') {
            if rest.starts_with("1049h") || rest.starts_with("1047h") || rest.starts_with("47h") {
                self.alt_screen_47_or_1047 = true;
                self.record_escape_sequence("CSI ?1049h (AltScreen ON)");
            } else if rest.starts_with("1049l")
                || rest.starts_with("1047l")
                || rest.starts_with("47l")
            {
                self.alt_screen_47_or_1047 = false;
                self.record_escape_sequence("CSI ?1049l (AltScreen OFF)");
            } else if rest.starts_with("2004h") {
                self.bracketed_paste_2004 = true;
                self.record_escape_sequence("CSI ?2004h (BracketedPaste ON)");
            } else if rest.starts_with("2004l") {
                self.bracketed_paste_2004 = false;
                self.record_escape_sequence("CSI ?2004l (BracketedPaste OFF)");
            } else if rest.starts_with("1h") {
                self.application_cursor_decckm = true;
                self.record_escape_sequence("CSI ?1h (DECCKM ON)");
            } else if rest.starts_with("1l") {
                self.application_cursor_decckm = false;
                self.record_escape_sequence("CSI ?1l (DECCKM OFF)");
            } else if rest.starts_with("67h") {
                self.backarrow_key_67 = true;
            } else if rest.starts_with("67l") {
                self.backarrow_key_67 = false;
            } else if rest.starts_with("1007h") {
                self.mouse_alternate_scroll_1007 = true;
            } else if rest.starts_with("1007l") {
                self.mouse_alternate_scroll_1007 = false;
            } else if rest.starts_with("1000h")
                || rest.starts_with("1002h")
                || rest.starts_with("1003h")
                || rest.starts_with("1006h")
            {
                self.mouse_tracking_mode = true;
                self.record_escape_sequence("CSI ?1006h (MouseSGR ON)");
            } else if rest.starts_with("1000l")
                || rest.starts_with("1002l")
                || rest.starts_with("1003l")
                || rest.starts_with("1006l")
            {
                self.mouse_tracking_mode = false;
                self.record_escape_sequence("CSI ?1006l (MouseSGR OFF)");
            } else if rest.starts_with("1004h") {
                self.focus_reporting_1004 = true;
            } else if rest.starts_with("1004l") {
                self.focus_reporting_1004 = false;
            } else if rest.starts_with("2026h") {
                self.synchronized_output_2026 = true;
                self.record_escape_sequence("CSI ?2026h (SyncOutput ON)");
            } else if rest.starts_with("2026l") {
                self.synchronized_output_2026 = false;
                self.record_escape_sequence("CSI ?2026l (SyncOutput OFF)");
            } else if rest.starts_with("2027h") {
                self.grapheme_cluster_2027 = true;
                self.record_escape_sequence("CSI ?2027h (GraphemeCluster ON)");
            } else if rest.starts_with("2027l") {
                self.grapheme_cluster_2027 = false;
                self.record_escape_sequence("CSI ?2027l (GraphemeCluster OFF)");
            } else if rest.starts_with("2031h") {
                self.report_color_scheme_2031 = true;
                self.record_escape_sequence("CSI ?2031h (ColorSchemeNotify ON)");
            } else if rest.starts_with("2031l") {
                self.report_color_scheme_2031 = false;
                self.record_escape_sequence("CSI ?2031l (ColorSchemeNotify OFF)");
            } else if rest.starts_with("2048h") {
                self.in_band_size_2048 = true;
                self.record_escape_sequence("CSI ?2048h (InBandResize ON)");
            } else if rest.starts_with("2048l") {
                self.in_band_size_2048 = false;
                self.record_escape_sequence("CSI ?2048l (InBandResize OFF)");
            } else if rest.starts_with("996n") {
                let code = self.color_scheme_code();
                self.pending_query_responses
                    .push(format!("\x1b[?997;{code}n").into_bytes());
                self.record_escape_sequence(format!("CSI ?996n -> ?997;{code}n"));
            } else if rest.starts_with('u') {
                let flags = self.kitty_keyboard_flags();
                self.pending_query_responses
                    .push(format!("\x1b[?{flags}u").into_bytes());
                self.record_escape_sequence(format!("CSI ?u -> ?{flags}u"));
            } else {
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if !digits.is_empty()
                    && rest[digits.len()..].starts_with("$p")
                    && let Ok(mode) = digits.parse::<u16>()
                {
                    let ps = self.decrpm_private_mode_state(mode);
                    self.pending_query_responses
                        .push(format!("\x1b[?{mode};{ps}$y").into_bytes());
                }
            }
            return;
        }

        let rest = csi_rest;
        if rest.starts_with("5n") {
            self.pending_query_responses.push(b"\x1b[0n".to_vec());
        } else if rest.starts_with("6n") {
            let (r, c) = self.parser.screen().cursor_position();
            self.pending_query_responses
                .push(format!("\x1b[{};{}R", r + 1, c + 1).into_bytes());
        } else if rest.starts_with('c') || rest.starts_with("0c") {
            self.pending_query_responses.push(b"\x1b[?62;22c".to_vec());
        } else if let Some(after_gt) = rest.strip_prefix('>') {
            let digits: String = after_gt
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if after_gt[digits.len()..].starts_with('u') {
                let flags = digits.parse::<u8>().unwrap_or(0) & 0x1F;
                let stack = self.kitty_keyboard_stack_mut();
                if stack.len() >= 16 {
                    stack.remove(0);
                }
                stack.push(flags);
            }
        } else if let Some(after_lt) = rest.strip_prefix('<') {
            let digits: String = after_lt
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if after_lt[digits.len()..].starts_with('u') {
                let count = digits.parse::<usize>().unwrap_or(1).max(1);
                let stack = self.kitty_keyboard_stack_mut();
                for _ in 0..count {
                    if stack.len() > 1 {
                        stack.pop();
                    } else {
                        stack[0] = 0;
                    }
                }
            }
        } else if let Some(after_eq) = rest.strip_prefix('=') {
            let param_str: String = after_eq
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == ';')
                .collect();
            if !param_str.is_empty() && after_eq[param_str.len()..].starts_with('u') {
                let mut parts = param_str.split(';');
                let flags = parts
                    .next()
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(0)
                    & 0x1F;
                let mode = parts
                    .next()
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(1);
                let stack = self.kitty_keyboard_stack_mut();
                if let Some(top) = stack.last_mut() {
                    match mode {
                        2 => *top |= flags,
                        3 => *top &= !flags,
                        _ => *top = flags,
                    }
                }
            }
        } else {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty()
                && rest[digits.len()..].starts_with("$p")
                && let Ok(mode) = digits.parse::<u16>()
            {
                let ps = match mode {
                    4 | 20 => 2u8,
                    _ => 0u8,
                };
                self.pending_query_responses
                    .push(format!("\x1b[{mode};{ps}$y").into_bytes());
            } else {
                let sgr_params: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || *c == ';' || *c == ':')
                    .collect();
                if rest[sgr_params.len()..].starts_with('m') {
                    self.apply_extended_sgr(&sgr_params);
                }
            }
        }
    }

    fn decrpm_private_mode_state(&self, mode: u16) -> u8 {
        let is_set = match mode {
            1 => self.is_application_cursor(),
            7 => true,
            25 => !self.parser.screen().hide_cursor(),
            47 | 1047 | 1049 => self.is_alternate_screen(),
            67 => self.backarrow_key_67,
            1000 | 1002 | 1003 | 1006 => self.is_mouse_sgr_or_tracking(),
            1004 => self.focus_reporting_1004,
            1007 => self.mouse_alternate_scroll_1007,
            2004 => self.is_bracketed_paste(),
            2026 => self.synchronized_output_2026,
            2027 => self.grapheme_cluster_2027,
            2031 => self.report_color_scheme_2031,
            2048 => self.in_band_size_2048,
            _ => return 0,
        };
        if is_set { 1 } else { 2 }
    }

    fn apply_extended_sgr(&mut self, params: &str) {
        if params.is_empty() {
            self.active_underline_kind = UnderlineKind::None;
            self.active_underline_color = None;
            self.active_overline = false;
            return;
        }
        let tokens: Vec<&str> = params.split(';').collect();
        let mut idx = 0usize;
        while idx < tokens.len() {
            let tok = tokens[idx];
            if let Some(sub) = tok.strip_prefix("4:") {
                self.active_underline_kind = match sub {
                    "0" => UnderlineKind::None,
                    "1" => UnderlineKind::Single,
                    "2" => UnderlineKind::Double,
                    "3" => UnderlineKind::Curly,
                    "4" => UnderlineKind::Dotted,
                    "5" => UnderlineKind::Dashed,
                    _ => UnderlineKind::Single,
                };
                if self.active_underline_kind.is_underlined() {
                    self.last_styled_underline_kind = self.active_underline_kind;
                }
            } else if let Some(sub) = tok.strip_prefix("58:2:") {
                let nums: Vec<u8> = sub
                    .split(':')
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse::<u8>().ok())
                    .collect();
                if nums.len() >= 3 {
                    let n = nums.len();
                    let rgb = RgbTriplet::new(nums[n - 3], nums[n - 2], nums[n - 1]);
                    self.active_underline_color = Some(rgb);
                    self.last_styled_underline_color = Some(rgb);
                }
            } else if tok == "58" && idx + 4 < tokens.len() && tokens[idx + 1] == "2" {
                if let (Ok(r), Ok(g), Ok(b)) = (
                    tokens[idx + 2].parse::<u8>(),
                    tokens[idx + 3].parse::<u8>(),
                    tokens[idx + 4].parse::<u8>(),
                ) {
                    let rgb = RgbTriplet::new(r, g, b);
                    self.active_underline_color = Some(rgb);
                    self.last_styled_underline_color = Some(rgb);
                }
                idx += 4;
            } else {
                match tok {
                    "0" | "" => {
                        self.active_underline_kind = UnderlineKind::None;
                        self.active_underline_color = None;
                        self.active_overline = false;
                    }
                    "4" => {
                        self.active_underline_kind = UnderlineKind::Single;
                        self.last_styled_underline_kind = UnderlineKind::Single;
                    }
                    "21" => {
                        self.active_underline_kind = UnderlineKind::Double;
                        self.last_styled_underline_kind = UnderlineKind::Double;
                    }
                    "24" => self.active_underline_kind = UnderlineKind::None,
                    "53" => self.active_overline = true,
                    "55" => self.active_overline = false,
                    "59" => self.active_underline_color = None,
                    _ => {}
                }
            }
            idx += 1;
        }
    }
}

/// Rewrites extended SGR sub-parameters (`4:1..4:5` -> `4`, `4:0` -> `24`, and strips `58;2;r;g;b` / `58:2:...`)
/// and strips APC (`\x1b_...`) / DCS (`\x1bP...`) envelopes so `vt100_ctt::Parser` applies standard attributes
/// without dumping raw Kitty/Sixel graphics payloads onto the character grid.
pub(super) fn normalize_extended_sgr_for_vt100(bytes: &[u8]) -> Vec<u8> {
    let Ok(raw_text) = std::str::from_utf8(bytes) else {
        return bytes.to_vec();
    };
    let stripped_cow = if raw_text.contains("\x1b_") || raw_text.contains("\x1bP") {
        std::borrow::Cow::Owned(strip_apc_and_dcs_envelopes(raw_text))
    } else {
        std::borrow::Cow::Borrowed(raw_text)
    };
    let text = stripped_cow.as_ref();
    if !text.contains("\x1b[") || (!text.contains("4:") && !text.contains("58")) {
        return text.as_bytes().to_vec();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find("\x1b[") {
        out.push_str(&rest[..pos]);
        out.push_str("\x1b[");
        let after_csi = &rest[pos + 2..];
        let param_len = after_csi
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == ';' || *c == ':')
            .map(char::len_utf8)
            .sum::<usize>();
        if after_csi[param_len..].starts_with('m') {
            let params = &after_csi[..param_len];
            let tokens: Vec<&str> = params.split(';').collect();
            let mut norm_tokens = Vec::with_capacity(tokens.len());
            let mut idx = 0usize;
            while idx < tokens.len() {
                let tok = tokens[idx];
                if let Some(sub) = tok.strip_prefix("4:") {
                    if sub == "0" {
                        norm_tokens.push("24");
                    } else {
                        norm_tokens.push("4");
                    }
                } else if tok.starts_with("58:") {
                    // Strip colon-style underline color for vt100-ctt
                } else if tok == "58" && idx + 4 < tokens.len() && tokens[idx + 1] == "2" {
                    idx += 4;
                } else {
                    norm_tokens.push(tok);
                }
                idx += 1;
            }
            out.push_str(&norm_tokens.join(";"));
            out.push('m');
            rest = &after_csi[param_len + 1..];
        } else {
            rest = after_csi;
        }
    }
    out.push_str(rest);
    out.into_bytes()
}
