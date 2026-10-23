use std::path::PathBuf;
use std::time::Instant;

use super::codec::{base64_decode, hex_decode_ascii, hex_encode_ascii};
use super::detect::Osc8Hyperlink;
use super::graphics::{
    GraphicProtocol, TerminalGraphicPlacement, decode_sixel_placement, expand_kitty_payload_to_rgba,
    strip_apc_and_dcs_envelopes,
};
use super::snapshot::{RgbTriplet, UnderlineKind};
use super::{Osc9Progress, Osc133Phase, TerminalNotification, VtParser};

impl VtParser {
    pub(super) fn record_escape_sequence(&mut self, seq: impl Into<String>) {
        if self.recent_escape_sequences.len() >= 16 {
            self.recent_escape_sequences.remove(0);
        }
        self.recent_escape_sequences.push(seq.into());
    }

    /// Scans raw bytes for `DECSET 47/1047/1049`, `DECSET 2004`, `DECRPM`, Kitty keyboard protocol,
    /// `OSC 4/10/11/12` color queries, `OSC 8` hyperlinks, `OSC 52`, `OSC 133;A/B/C/D`, `XTGETTCAP`, and `DECRQSS`.
    pub(super) fn scan_control_sequences(&mut self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);

        // Track CSI sequences in chronological order: DECSET/DECRST (?modes), DECRPM ($p),
        // DSR (5n), CPR (6n), Primary DA (c), and Kitty Keyboard Protocol (>u, <u, =u, ?u)
        for (idx, _) in text.match_indices("\x1b[") {
            let csi_rest = &text[idx + 2..];
            if let Some(rest) = csi_rest.strip_prefix('?') {
                if rest.starts_with("1049h") || rest.starts_with("1047h") || rest.starts_with("47h")
                {
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
                    // Color scheme DSR query: CSI ? 996 n -> CSI ? 997 ; 1|2 n (BEL-03)
                    let code = self.color_scheme_code();
                    self.pending_query_responses
                        .push(format!("\x1b[?997;{code}n").into_bytes());
                    self.record_escape_sequence(format!("CSI ?996n -> ?997;{code}n"));
                } else if rest.starts_with('u') {
                    // Kitty keyboard protocol query: CSI ? u -> CSI ? <flags> u
                    let flags = self.kitty_keyboard_flags();
                    self.pending_query_responses
                        .push(format!("\x1b[?{flags}u").into_bytes());
                    self.record_escape_sequence(format!("CSI ?u -> ?{flags}u"));
                } else {
                    // Check for DECRPM private mode query: CSI ? <mode> $ p
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
                continue;
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
                // Kitty keyboard push: CSI > <flags> u
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
                // Kitty keyboard pop: CSI < <count> u
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
                // Kitty keyboard set: CSI = <flags> ; <mode> u (or CSI = <flags> u)
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
                // ANSI DECRPM query: CSI <mode> $ p -> CSI <mode> ; <state> $ y
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if !digits.is_empty()
                    && rest[digits.len()..].starts_with("$p")
                    && let Ok(mode) = digits.parse::<u16>()
                {
                    let ps = match mode {
                        4 | 20 => 2u8, // IRM (Insert Mode) and LNM (Linefeed/Newline) recognized as reset
                        _ => 0u8,
                    };
                    self.pending_query_responses
                        .push(format!("\x1b[{mode};{ps}$y").into_bytes());
                } else {
                    // Check for extended SGR sequence: CSI <params> m (FND-09)
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

        // Track OSC 0 / OSC 2 window title sequences: \x1b]0;<title>(\x07 or \x1b\\) and \x1b]2;<title>(\x07 or \x1b\\)
        for prefix in ["\x1b]0;", "\x1b]2;"] {
            for (idx, _) in text.match_indices(prefix) {
                let rest = &text[idx + prefix.len()..];
                if let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\")) {
                    self.window_title = Some(rest[..end].to_string());
                }
            }
        }

        // Track OSC 4 / 10 / 11 / 12 dynamic color queries (\x1b]10;?(\x07|\x1b\\), etc., FND-05)
        for (code, rgb) in [
            (10u8, self.theme_fg_rgb),
            (11u8, self.theme_bg_rgb),
            (12u8, self.theme_cursor_rgb),
        ] {
            let prefix = format!("\x1b]{code};?");
            for (idx, _) in text.match_indices(&prefix) {
                let rest = &text[idx + prefix.len()..];
                if rest.starts_with('\x07') || rest.starts_with("\x1b\\") {
                    let reply = format_osc_color_reply(code, None, rgb);
                    self.pending_query_responses.push(reply.into_bytes());
                }
            }
        }
        for (idx, _) in text.match_indices("\x1b]4;") {
            let rest = &text[idx + 4..];
            if let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\")) {
                let body = &rest[..end];
                if let Some((idx_str, spec)) = body.split_once(';')
                    && spec.trim() == "?"
                    && let Ok(palette_idx) = idx_str.trim().parse::<u8>()
                {
                    let rgb = ansi_256_index_to_rgb(palette_idx);
                    let reply = format_osc_color_reply(4, Some(palette_idx), rgb);
                    self.pending_query_responses.push(reply.into_bytes());
                }
            }
        }

        // Track OSC 7 working directory sequences: \x1b]7;<file-uri-or-path>(\x07 or \x1b\\)
        for (idx, _) in text.match_indices("\x1b]7;") {
            let rest = &text[idx + 4..];
            if let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\")) {
                let raw_uri = rest[..end].trim();
                let path_part = if let Some(after_scheme) = raw_uri.strip_prefix("file://") {
                    if let Some(slash_idx) = after_scheme.find('/') {
                        &after_scheme[slash_idx..]
                    } else {
                        after_scheme
                    }
                } else {
                    raw_uri
                };
                if !path_part.is_empty() {
                    self.reported_cwd = Some(PathBuf::from(path_part));
                }
            }
        }

        // Track OSC 8 explicit hyperlinks: \x1b]8;<params>;<uri>(\x07 or \x1b\\) (VAL-03)
        for (idx, _) in text.match_indices("\x1b]8;") {
            let rest = &text[idx + 4..];
            if let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\")) {
                let payload = &rest[..end];
                if let Some((params, uri)) = payload.split_once(';')
                    && !uri.is_empty()
                {
                    let id = params
                        .split(':')
                        .find_map(|kv| kv.strip_prefix("id="))
                        .map(str::to_string);
                    self.osc8_links.push(Osc8Hyperlink {
                        id,
                        uri: uri.to_string(),
                    });
                    self.record_escape_sequence(format!("OSC 8 {uri}"));
                }
            }
        }

        // Track OSC 9;<msg> notifications (when not OSC 9;4;... progress) and OSC 777;notify;<title>;<body> (v0.7.0)
        for (idx, _) in text.match_indices("\x1b]9;") {
            let rest = &text[idx + 4..];
            if !rest.starts_with("4;")
                && let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\"))
            {
                let msg = rest[..end].trim();
                if !msg.is_empty() {
                    self.notifications.push(TerminalNotification {
                        title: "OSC 9".to_string(),
                        body: msg.to_string(),
                    });
                    self.unread_bell = true;
                    self.record_escape_sequence(format!("OSC 9 Notify: {msg}"));
                }
            }
        }
        for (idx, _) in text.match_indices("\x1b]777;notify;") {
            let rest = &text[idx + 13..];
            if let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\")) {
                let payload = &rest[..end];
                let (title, body) = payload.split_once(';').unwrap_or(("NOTIFY", payload));
                self.notifications.push(TerminalNotification {
                    title: title.trim().to_string(),
                    body: body.trim().to_string(),
                });
                self.unread_bell = true;
                self.record_escape_sequence(format!("OSC 777 Notify: {title}"));
            }
        }

        // Track OSC 9;4 progress bar reports: \x1b]9;4;<state>[;<pct>](\x07 or \x1b\\) (BEL-02)
        for (idx, _) in text.match_indices("\x1b]9;4;") {
            let rest = &text[idx + 6..];
            if let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\")) {
                let payload = rest[..end].trim();
                let mut parts = payload.split(';');
                let state = parts
                    .next()
                    .and_then(|s| s.trim().parse::<u8>().ok())
                    .unwrap_or(0);
                let pct = parts
                    .next()
                    .and_then(|s| s.trim().parse::<u8>().ok())
                    .unwrap_or(0)
                    .min(100);
                self.osc9_progress = match state {
                    0 => None,
                    1 => Some(Osc9Progress::Normal(pct)),
                    2 => Some(Osc9Progress::Error(pct)),
                    3 => Some(Osc9Progress::Indeterminate),
                    4 => Some(Osc9Progress::Paused(pct)),
                    _ => self.osc9_progress,
                };
                self.record_escape_sequence(format!("OSC 9;4;{payload}"));
            }
        }

        // Track OSC 52 clipboard sequences: \x1b]52;<target>;<base64>(\x07 or \x1b\\)
        for (idx, _) in text.match_indices("\x1b]52;") {
            let rest = &text[idx..];
            if let Some(end_bel) = rest.find('\x07') {
                self.pending_osc52.push(rest[..=end_bel].to_string());
            } else if let Some(end_st) = rest.find("\x1b\\") {
                self.pending_osc52.push(rest[..end_st + 2].to_string());
            }
        }

        // Track DCS XTGETTCAP (\x1bP+q<hex>\x1b\\), DECRQSS (\x1bP$q<pt>\x1b\\), and DEC Sixel (\x1bP...q...\x1b\\)
        for (idx, _) in text.match_indices("\x1bP") {
            let rest = &text[idx + 2..];
            if let Some(end) = rest.find("\x1b\\").or_else(|| rest.find('\x07')) {
                let dcs_body = &rest[..end];
                if !dcs_body.starts_with("+q")
                    && !dcs_body.starts_with("$q")
                    && !dcs_body.starts_with("tmux;")
                    && let Some(q_pos) = dcs_body.find('q')
                {
                    let params = &dcs_body[..q_pos];
                    if params.chars().all(|c| c.is_ascii_digit() || c == ';') {
                        let sixel_payload = &dcs_body[q_pos + 1..];
                        let (cur_row, cur_col) = self.parser.screen().cursor_position();
                        let id = self.next_graphic_id;
                        self.next_graphic_id = self.next_graphic_id.wrapping_add(1);
                        let placement = decode_sixel_placement(id, cur_row, cur_col, sixel_payload);
                        self.record_escape_sequence(format!(
                            "DCS Sixel id={id} {}x{}px",
                            placement.pixel_width, placement.pixel_height
                        ));
                        self.graphic_placements.push(placement);
                    }
                }
            }
        }

        // Track Kitty Graphics Protocol APC sequences: \x1b_G<control>;<base64>(\x1b\\ or \x07) (GFX-01)
        for (idx, _) in text.match_indices("\x1b_G") {
            let rest = &text[idx + 3..];
            if let Some(end) = rest.find("\x1b\\").or_else(|| rest.find('\x07')) {
                let body = &rest[..end];
                let (ctrl_str, b64_payload) = body.split_once(';').unwrap_or((body, ""));
                let mut action = 'T';
                let mut image_id: Option<u32> = None;
                let mut format_code: u32 = 32;
                let mut px_w: u16 = 0;
                let mut px_h: u16 = 0;
                let mut cell_c: u16 = 0;
                let mut cell_r: u16 = 0;
                let mut quiet: u8 = 0;
                for kv in ctrl_str.split(',') {
                    if let Some((k, v)) = kv.split_once('=') {
                        match k.trim() {
                            "a" => action = v.chars().next().unwrap_or('T'),
                            "i" => image_id = v.parse::<u32>().ok(),
                            "f" => format_code = v.parse::<u32>().unwrap_or(32),
                            "s" => px_w = v.parse::<u16>().unwrap_or(0),
                            "v" => px_h = v.parse::<u16>().unwrap_or(0),
                            "c" => cell_c = v.parse::<u16>().unwrap_or(0),
                            "r" => cell_r = v.parse::<u16>().unwrap_or(0),
                            "q" => quiet = v.parse::<u8>().unwrap_or(0),
                            _ => {}
                        }
                    }
                }
                let resolved_id = image_id.unwrap_or_else(|| {
                    let id = self.next_graphic_id;
                    self.next_graphic_id = self.next_graphic_id.wrapping_add(1);
                    id
                });
                match action {
                    'q' => {
                        if quiet < 2 {
                            self.pending_query_responses
                                .push(format!("\x1b_Gi={resolved_id};OK\x1b\\").into_bytes());
                        }
                        self.record_escape_sequence(format!("APC Kitty Query i={resolved_id}"));
                    }
                    'd' => {
                        if let Some(target_id) = image_id {
                            self.graphic_placements.retain(|g| g.id != target_id);
                        } else {
                            self.graphic_placements.clear();
                        }
                        self.record_escape_sequence("APC Kitty Delete");
                    }
                    't' | 'T' | 'p' => {
                        let raw_bytes = base64_decode(b64_payload);
                        let w = px_w.max(2);
                        let h = px_h.max(2);
                        let rgba = expand_kitty_payload_to_rgba(&raw_bytes, format_code, w, h);
                        let (cur_row, cur_col) = self.parser.screen().cursor_position();
                        let cols = if cell_c > 0 { cell_c } else { w.div_ceil(8).max(1) };
                        let rows = if cell_r > 0 { cell_r } else { h.div_ceil(16).max(1) };
                        self.graphic_placements.push(TerminalGraphicPlacement {
                            id: resolved_id,
                            protocol: GraphicProtocol::Kitty,
                            row: cur_row,
                            col: cur_col,
                            cols,
                            rows,
                            pixel_width: w,
                            pixel_height: h,
                            rgba_bytes: rgba,
                        });
                        if image_id.is_some() && quiet == 0 {
                            self.pending_query_responses
                                .push(format!("\x1b_Gi={resolved_id};OK\x1b\\").into_bytes());
                        }
                        self.record_escape_sequence(format!(
                            "APC Kitty {action} i={resolved_id} {w}x{h}px"
                        ));
                    }
                    _ => {}
                }
            }
        }

        // Track DCS XTGETTCAP (\x1bP+q<hex>\x1b\\) and DECRQSS (\x1bP$q<pt>\x1b\\) (FND-06)
        for (idx, _) in text.match_indices("\x1bP+q") {
            let rest = &text[idx + 4..];
            if let Some(end) = rest.find("\x1b\\").or_else(|| rest.find('\x07')) {
                let query_body = &rest[..end];
                for hex_cap in query_body.split(';') {
                    if let Some(cap_name) = hex_decode_ascii(hex_cap.trim()) {
                        let reply = match xtgettcap_lookup(&cap_name) {
                            Some(val) => {
                                let name_hex = hex_encode_ascii(cap_name.as_bytes());
                                let val_hex = hex_encode_ascii(val.as_bytes());
                                format!("\x1bP1+r{name_hex}={val_hex}\x1b\\")
                            }
                            None => {
                                let name_hex = hex_encode_ascii(cap_name.as_bytes());
                                format!("\x1bP0+r{name_hex}\x1b\\")
                            }
                        };
                        self.pending_query_responses.push(reply.into_bytes());
                    }
                }
            }
        }
        for (idx, _) in text.match_indices("\x1bP$q") {
            let rest = &text[idx + 4..];
            if let Some(end) = rest.find("\x1b\\").or_else(|| rest.find('\x07')) {
                let pt = rest[..end].trim();
                let reply = match pt {
                    "m" => "\x1bP1$r0m\x1b\\".to_string(),
                    "r" => {
                        let (rows, _) = self.parser.screen().size();
                        format!("\x1bP1$r1;{rows}r\x1b\\")
                    }
                    " q" => "\x1bP1$r2 q\x1b\\".to_string(),
                    _ => format!("\x1bP0$r{pt}\x1b\\"),
                };
                self.pending_query_responses.push(reply.into_bytes());
            }
        }

        // Track OSC 133 semantic shell markers: \x1b]133;<phase>...(\x07 or \x1b\\)
        for (idx, _) in text.match_indices("\x1b]133;") {
            let rest = &text[idx + 6..];
            if let Some(first_char) = rest.chars().next() {
                match first_char {
                    'A' => {
                        self.osc133_phase = Some(Osc133Phase::PromptStart);
                    }
                    'B' => {
                        self.osc133_phase = Some(Osc133Phase::CommandStart);
                    }
                    'C' => {
                        self.osc133_phase = Some(Osc133Phase::CommandExecuted);
                        if self.last_command_started_at.is_none() {
                            self.last_command_started_at = Some(Instant::now());
                        }
                    }
                    'D' => {
                        let exit_code = rest.strip_prefix("D;").and_then(|s| {
                            s.chars()
                                .take_while(|c| c.is_ascii_digit() || *c == '-')
                                .collect::<String>()
                                .parse::<i32>()
                                .ok()
                        });
                        let elapsed_ms = self
                            .last_command_started_at
                            .take()
                            .map(|t| t.elapsed().as_millis() as u64);
                        self.last_exit_code = exit_code;
                        self.last_duration_ms = elapsed_ms;
                        self.osc133_phase = Some(Osc133Phase::CommandFinished(exit_code));
                        if let Some(last) = self.command_blocks.last_mut() {
                            last.exit_code = exit_code;
                            if last.duration_ms.is_none() {
                                last.duration_ms = elapsed_ms;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn decrpm_private_mode_state(&self, mode: u16) -> u8 {
        // 1 = set, 2 = reset, 0 = not recognized
        let is_set = match mode {
            1 => self.is_application_cursor(),
            7 => true, // DECAWM auto-wrap mode
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
                // Colon-separated SGR 58:2::r:g:b or 58:2:r:g:b
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
                // Semicolon-separated SGR 58;2;r;g;b
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

fn format_osc_color_reply(code: u8, palette_idx: Option<u8>, (r, g, b): (u8, u8, u8)) -> String {
    let r16 = u16::from(r) * 0x0101;
    let g16 = u16::from(g) * 0x0101;
    let b16 = u16::from(b) * 0x0101;
    match palette_idx {
        Some(idx) => format!("\x1b]{code};{idx};rgb:{r16:04x}/{g16:04x}/{b16:04x}\x1b\\"),
        None => format!("\x1b]{code};rgb:{r16:04x}/{g16:04x}/{b16:04x}\x1b\\"),
    }
}

fn ansi_256_index_to_rgb(idx: u8) -> (u8, u8, u8) {
    match idx {
        0 => (0x00, 0x00, 0x00),
        1 => (0xAA, 0x00, 0x00),
        2 => (0x00, 0xAA, 0x00),
        3 => (0xAA, 0x55, 0x00),
        4 => (0x00, 0x00, 0xAA),
        5 => (0xAA, 0x00, 0xAA),
        6 => (0x00, 0xAA, 0xAA),
        7 => (0xAA, 0xAA, 0xAA),
        8 => (0x55, 0x55, 0x55),
        9 => (0xFF, 0x55, 0x55),
        10 => (0x55, 0xFF, 0x55),
        11 => (0xFF, 0xFF, 0x55),
        12 => (0x55, 0x55, 0xFF),
        13 => (0xFF, 0x55, 0xFF),
        14 => (0x55, 0xFF, 0xFF),
        15 => (0xFF, 0xFF, 0xFF),
        16..=231 => {
            let n = idx - 16;
            let r = (n / 36) % 6;
            let g = (n / 6) % 6;
            let b = n % 6;
            let map = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            (map(r), map(g), map(b))
        }
        232..=255 => {
            let gray = 8 + (idx - 232) * 10;
            (gray, gray, gray)
        }
    }
}

fn xtgettcap_lookup(cap: &str) -> Option<&'static str> {
    match cap {
        "TN" | "name" => Some("xterm-256color"),
        "Co" | "colors" => Some("256"),
        "RGB" => Some("8/8/8"),
        "Tc" | "setrgbf" | "setrgbb" => Some("1"),
        "Smulx" => Some("\x1b[4:%p1%dm"),
        "Setulc" => Some("\x1b[58:2::%p1%{65536}%/%d:%p1%{256}%/%{255}%&%d:%p1%{255}%&%dm"),
        "Ms" => Some("\x1b]52;%p1%s;%p2%s\x07"),
        _ => None,
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
