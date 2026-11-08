//! JSON-RPC handlers for `browser.*` and `video.*` APIs (`WEB-01..02`, `VID-01..02`).

use std::collections::BTreeMap;

use super::json::JsonValue;
use super::rpc::AutomationControlPlane;
use crate::action::{BrowserOp, VideoOp};

impl AutomationControlPlane {
    pub(super) fn rpc_browser_open(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let url = params
            .get("url")
            .and_then(JsonValue::as_str)
            .unwrap_or("https://example.com");
        let pane_id = params.get("pane_id").and_then(JsonValue::as_usize);

        let target_id = pane_id.unwrap_or_else(|| self.wm.active_pane_id());
        let op = BrowserOp::Open {
            url: url.to_string(),
            pane_id: Some(target_id),
        };
        crate::colon_command::execute_media::execute_browser_command(&mut self.wm, op);

        let mut res = BTreeMap::new();
        res.insert("ok".to_string(), JsonValue::Bool(true));
        res.insert("url".to_string(), JsonValue::String(url.to_string()));
        res.insert("pane_id".to_string(), JsonValue::Number(target_id as i64));
        Ok(JsonValue::Object(res))
    }

    pub(super) fn rpc_browser_control(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let action_str = params
            .get("action")
            .and_then(JsonValue::as_str)
            .unwrap_or("toggle");
        let pane_id = params.get("pane_id").and_then(JsonValue::as_usize);
        let target_id = pane_id.unwrap_or_else(|| self.wm.active_pane_id());

        let op = match action_str {
            "open" => {
                let url = params
                    .get("url")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("https://example.com");
                BrowserOp::Open {
                    url: url.to_string(),
                    pane_id: Some(target_id),
                }
            }
            "back" => BrowserOp::Back {
                pane_id: Some(target_id),
            },
            "forward" | "fwd" => BrowserOp::Forward {
                pane_id: Some(target_id),
            },
            "reload" | "refresh" => BrowserOp::Reload {
                pane_id: Some(target_id),
            },
            "stop" => BrowserOp::Stop {
                pane_id: Some(target_id),
            },
            "focus" => {
                let focused = params.get("focused").and_then(JsonValue::as_bool);
                BrowserOp::FocusViewport {
                    pane_id: Some(target_id),
                    focused,
                }
            }
            "zoom" => {
                let delta = params.get("delta_pct").and_then(JsonValue::as_i64).unwrap_or(10) as i16;
                BrowserOp::Zoom {
                    delta_pct: delta,
                    pane_id: Some(target_id),
                }
            }
            "close" => BrowserOp::Close {
                pane_id: Some(target_id),
            },
            _ => BrowserOp::Toggle,
        };

        crate::colon_command::execute_media::execute_browser_command(&mut self.wm, op);

        let mut res = BTreeMap::new();
        res.insert("ok".to_string(), JsonValue::Bool(true));
        res.insert("action".to_string(), JsonValue::String(action_str.to_string()));
        res.insert("pane_id".to_string(), JsonValue::Number(target_id as i64));
        Ok(JsonValue::Object(res))
    }

    pub(super) fn rpc_browser_state(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let pane_id = params.get("pane_id").and_then(JsonValue::as_usize);
        let target_id = pane_id.unwrap_or_else(|| self.wm.active_pane_id());

        let pane = self
            .wm
            .pane(target_id)
            .ok_or_else(|| format!("Pane {target_id} not found"))?;

        let mut res = BTreeMap::new();
        if let Some(b) = pane.browser_canvas() {
            res.insert("active".to_string(), JsonValue::Bool(true));
            res.insert("url".to_string(), JsonValue::String(b.current_url().to_string()));
            res.insert("title".to_string(), JsonValue::String(b.page_title().to_string()));
            res.insert("is_loading".to_string(), JsonValue::Bool(b.is_loading()));
            res.insert("backend".to_string(), JsonValue::String(b.backend_name().to_string()));
            res.insert("zoom_pct".to_string(), JsonValue::Number(b.zoom_pct as i64));
            res.insert("focused".to_string(), JsonValue::Bool(b.viewport_focused));
            let seq = b.cached_frame().map(|f| f.seq).unwrap_or(0);
            res.insert("frame_seq".to_string(), JsonValue::Number(seq as i64));
        } else {
            res.insert("active".to_string(), JsonValue::Bool(false));
        }
        res.insert("pane_id".to_string(), JsonValue::Number(target_id as i64));
        Ok(JsonValue::Object(res))
    }

    pub(super) fn rpc_video_open(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let source = params
            .get("source")
            .and_then(JsonValue::as_str)
            .unwrap_or("demo://smpte");
        let pane_id = params.get("pane_id").and_then(JsonValue::as_usize);
        let target_id = pane_id.unwrap_or_else(|| self.wm.active_pane_id());

        let op = VideoOp::Open {
            source: source.to_string(),
            pane_id: Some(target_id),
        };
        crate::colon_command::execute_media::execute_video_command(&mut self.wm, op);

        let mut res = BTreeMap::new();
        res.insert("ok".to_string(), JsonValue::Bool(true));
        res.insert("source".to_string(), JsonValue::String(source.to_string()));
        res.insert("pane_id".to_string(), JsonValue::Number(target_id as i64));
        Ok(JsonValue::Object(res))
    }

    pub(super) fn rpc_video_control(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let action_str = params
            .get("action")
            .and_then(JsonValue::as_str)
            .unwrap_or("toggle");
        let pane_id = params.get("pane_id").and_then(JsonValue::as_usize);
        let target_id = pane_id.unwrap_or_else(|| self.wm.active_pane_id());

        let op = match action_str {
            "open" => {
                let src = params
                    .get("source")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("demo://smpte");
                VideoOp::Open {
                    source: src.to_string(),
                    pane_id: Some(target_id),
                }
            }
            "play" => VideoOp::Play {
                pane_id: Some(target_id),
            },
            "pause" => VideoOp::Pause {
                pane_id: Some(target_id),
            },
            "seek" => {
                let secs = params.get("delta_secs").and_then(JsonValue::as_f64).unwrap_or(5.0);
                let delta_ms = (secs * 1000.0) as i64;
                VideoOp::SeekRelative {
                    delta_ms,
                    pane_id: Some(target_id),
                }
            }
            "seek_pct" => {
                let pct = params.get("pct").and_then(JsonValue::as_usize).unwrap_or(0) as u8;
                VideoOp::SeekPercent {
                    pct,
                    pane_id: Some(target_id),
                }
            }
            "volume" => {
                let delta = params.get("volume_delta").and_then(JsonValue::as_i64).unwrap_or(5) as i8;
                VideoOp::VolumeDelta {
                    delta_pct: delta,
                    pane_id: Some(target_id),
                }
            }
            "mute" => VideoOp::ToggleMute {
                pane_id: Some(target_id),
            },
            "loop" => VideoOp::ToggleLoop {
                pane_id: Some(target_id),
            },
            "speed" => {
                let speed_x100 = params
                    .get("speed")
                    .and_then(JsonValue::as_usize)
                    .unwrap_or(100) as u16;
                VideoOp::SetSpeed {
                    speed_x100,
                    pane_id: Some(target_id),
                }
            }
            "close" => VideoOp::Close {
                pane_id: Some(target_id),
            },
            _ => VideoOp::TogglePlayPause {
                pane_id: Some(target_id),
            },
        };

        crate::colon_command::execute_media::execute_video_command(&mut self.wm, op);

        let mut res = BTreeMap::new();
        res.insert("ok".to_string(), JsonValue::Bool(true));
        res.insert("action".to_string(), JsonValue::String(action_str.to_string()));
        res.insert("pane_id".to_string(), JsonValue::Number(target_id as i64));
        Ok(JsonValue::Object(res))
    }

    pub(super) fn rpc_video_state(&mut self, params: &JsonValue) -> Result<JsonValue, String> {
        let pane_id = params.get("pane_id").and_then(JsonValue::as_usize);
        let target_id = pane_id.unwrap_or_else(|| self.wm.active_pane_id());

        let pane = self
            .wm
            .pane(target_id)
            .ok_or_else(|| format!("Pane {target_id} not found"))?;

        let mut res = BTreeMap::new();
        if let Some(v) = pane.video_canvas() {
            let snap = v.snapshot();
            res.insert("active".to_string(), JsonValue::Bool(true));
            res.insert("source".to_string(), JsonValue::String(snap.source));
            res.insert("playing".to_string(), JsonValue::Bool(snap.playing));
            res.insert("position_ms".to_string(), JsonValue::Number(snap.position_ms as i64));
            res.insert("duration_ms".to_string(), JsonValue::Number(snap.duration_ms as i64));
            res.insert("volume_pct".to_string(), JsonValue::Number(snap.volume_pct as i64));
            res.insert("muted".to_string(), JsonValue::Bool(snap.muted));
            res.insert("looping".to_string(), JsonValue::Bool(snap.looping));
            res.insert("speed_x100".to_string(), JsonValue::Number(snap.speed_x100 as i64));
            res.insert("width_px".to_string(), JsonValue::Number(snap.width_px as i64));
            res.insert("height_px".to_string(), JsonValue::Number(snap.height_px as i64));
            res.insert("frame_seq".to_string(), JsonValue::Number(snap.frame_seq as i64));
            res.insert("backend".to_string(), JsonValue::String(snap.backend_name.to_string()));
        } else {
            res.insert("active".to_string(), JsonValue::Bool(false));
        }
        res.insert("pane_id".to_string(), JsonValue::Number(target_id as i64));
        Ok(JsonValue::Object(res))
    }
}
