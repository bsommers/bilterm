//! Single-pass OSC, DCS, and APC control sequence handlers (`ARCH-R04`, `P2.2`).

use std::path::PathBuf;
use std::time::Instant;

use super::codec::{
    base64_decode, hex_decode_ascii, hex_encode_ascii, is_forwardable_osc52_write,
};
use super::detect::Osc8Hyperlink;
use super::graphics::{
    GraphicProtocol, MAX_GRAPHIC_PLACEMENTS, MAX_KITTY_PAYLOAD_B64_BYTES, TerminalGraphicPlacement,
    decode_sixel_placement, expand_kitty_payload_to_rgba,
};
use super::{
    MAX_NOTIFICATIONS, MAX_OSC8_LINKS, Osc9Progress, Osc133Phase, TerminalNotification, VtParser,
};

impl VtParser {
    /// Dispatches a single `OSC` (`\x1b]...`) sequence starting at `osc_rest` (`&text[idx + 2..]`).
    pub(super) fn scan_osc_sequence(&mut self, osc_rest: &str) {
        if let Some(rest) = osc_rest
            .strip_prefix("0;")
            .or_else(|| osc_rest.strip_prefix("2;"))
        {
            if let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\")) {
                self.window_title = Some(rest[..end].to_string());
            }
            return;
        }

        for (prefix, code, rgb) in [
            ("10;?", 10u8, self.theme_fg_rgb),
            ("11;?", 11u8, self.theme_bg_rgb),
            ("12;?", 12u8, self.theme_cursor_rgb),
        ] {
            if let Some(rest) = osc_rest.strip_prefix(prefix) {
                if rest.starts_with('\x07') || rest.starts_with("\x1b\\") {
                    let reply = format_osc_color_reply(code, None, rgb);
                    self.pending_query_responses.push(reply.into_bytes());
                }
                return;
            }
        }

        if let Some(rest) = osc_rest.strip_prefix("4;") {
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
            return;
        }

        if let Some(rest) = osc_rest.strip_prefix("7;") {
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
            return;
        }

        if let Some(rest) = osc_rest.strip_prefix("8;") {
            if let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\")) {
                let payload = &rest[..end];
                if let Some((params, uri)) = payload.split_once(';')
                    && !uri.is_empty()
                {
                    let id = params
                        .split(':')
                        .find_map(|kv| kv.strip_prefix("id="))
                        .map(str::to_string);
                    push_bounded(
                        &mut self.osc8_links,
                        Osc8Hyperlink {
                            id,
                            uri: uri.to_string(),
                        },
                        MAX_OSC8_LINKS,
                    );
                    self.record_escape_sequence(format!("OSC 8 {uri}"));
                }
            }
            return;
        }

        if let Some(rest) = osc_rest.strip_prefix("9;4;") {
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
            return;
        }

        if let Some(rest) = osc_rest.strip_prefix("9;") {
            if let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\")) {
                let msg = rest[..end].trim();
                if !msg.is_empty() {
                    self.push_notification(TerminalNotification {
                        title: "OSC 9".to_string(),
                        body: msg.to_string(),
                    });
                    self.unread_bell = true;
                    self.record_escape_sequence(format!("OSC 9 Notify: {msg}"));
                }
            }
            return;
        }

        if let Some(rest) = osc_rest.strip_prefix("777;notify;") {
            if let Some(end) = rest.find('\x07').or_else(|| rest.find("\x1b\\")) {
                let payload = &rest[..end];
                let (title, body) = payload.split_once(';').unwrap_or(("NOTIFY", payload));
                self.push_notification(TerminalNotification {
                    title: title.trim().to_string(),
                    body: body.trim().to_string(),
                });
                self.unread_bell = true;
                self.record_escape_sequence(format!("OSC 777 Notify: {title}"));
            }
            return;
        }

        if osc_rest.starts_with("52;") {
            // Terminate at the first BEL or ESC of *this* OSC; an ESC that does not begin ST
            // aborts the sequence so no foreign escapes can ride along to the host terminal.
            if let Some(end) = osc_rest.find(['\x07', '\x1b']) {
                let terminator = match &osc_rest[end..] {
                    t if t.starts_with('\x07') => "\x07",
                    t if t.starts_with("\x1b\\") => "\x1b\\",
                    _ => return,
                };
                let body = &osc_rest[..end];
                if is_forwardable_osc52_write(body) {
                    self.pending_osc52.push(format!("\x1b]{body}{terminator}"));
                }
            }
            return;
        }

        if let Some(rest) = osc_rest.strip_prefix("133;")
            && let Some(first_char) = rest.chars().next()
        {
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

    /// Dispatches a single `DCS` (`\x1bP...`) sequence starting at `dcs_rest` (`&text[idx + 2..]`).
    pub(super) fn scan_dcs_sequence(&mut self, dcs_rest: &str) {
        let Some(end) = dcs_rest.find("\x1b\\").or_else(|| dcs_rest.find('\x07')) else {
            return;
        };
        let dcs_body = &dcs_rest[..end];

        if let Some(query_body) = dcs_body.strip_prefix("+q") {
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
            return;
        }

        if let Some(pt_raw) = dcs_body.strip_prefix("$q") {
            let pt = pt_raw.trim();
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
            return;
        }

        if !dcs_body.starts_with("tmux;")
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
                push_bounded(&mut self.graphic_placements, placement, MAX_GRAPHIC_PLACEMENTS);
            }
        }
    }

    /// Dispatches a single `APC` (`\x1b_...`) sequence starting at `apc_rest` (`&text[idx + 2..]`).
    pub(super) fn scan_apc_sequence(&mut self, apc_rest: &str) {
        let Some(rest) = apc_rest.strip_prefix('G') else {
            return;
        };
        let Some(end) = rest.find("\x1b\\").or_else(|| rest.find('\x07')) else {
            return;
        };
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
                let w = px_w.max(2);
                let h = px_h.max(2);
                if b64_payload.len() > MAX_KITTY_PAYLOAD_B64_BYTES {
                    self.record_escape_sequence(format!("APC Kitty rejected i={resolved_id}"));
                    return;
                }
                let raw_bytes = base64_decode(b64_payload);
                let Some(rgba) = expand_kitty_payload_to_rgba(&raw_bytes, format_code, w, h)
                else {
                    self.record_escape_sequence(format!(
                        "APC Kitty rejected i={resolved_id} {w}x{h}px"
                    ));
                    return;
                };
                let (cur_row, cur_col) = self.parser.screen().cursor_position();
                let cols = if cell_c > 0 {
                    cell_c
                } else {
                    w.div_ceil(8).max(1)
                };
                let rows = if cell_r > 0 {
                    cell_r
                } else {
                    h.div_ceil(16).max(1)
                };
                let placement = TerminalGraphicPlacement {
                    id: resolved_id,
                    protocol: GraphicProtocol::Kitty,
                    row: cur_row,
                    col: cur_col,
                    cols,
                    rows,
                    pixel_width: w,
                    pixel_height: h,
                    rgba_bytes: rgba,
                };
                push_bounded(&mut self.graphic_placements, placement, MAX_GRAPHIC_PLACEMENTS);
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

impl VtParser {
    fn push_notification(&mut self, notification: TerminalNotification) {
        push_bounded(&mut self.notifications, notification, MAX_NOTIFICATIONS);
        self.notifications_received = self.notifications_received.wrapping_add(1);
    }
}

/// Appends `item`, dropping the oldest entries so at most `cap` remain.
fn push_bounded<T>(items: &mut Vec<T>, item: T, cap: usize) {
    if items.len() >= cap {
        items.drain(..=items.len() - cap);
    }
    items.push(item);
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
