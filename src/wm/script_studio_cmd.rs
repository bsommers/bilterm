//! SCRIPT STUDIO command operations and log types (Plan 51.2-03, WIN-03).

use serde::{Deserialize, Serialize};

/// Script Studio commands / operations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScriptStudioOp {
    Open,
    Close,
    Toggle,
    Dismiss,
    Blur,
    Pin(Option<bool>),
    Dock(Option<bool>),
    RunAll,
    StepNext,
    ResetStep,
    Record(Option<bool>),
    EditMode(Option<bool>),
    CursorUp,
    CursorDown,
    CursorLeft,
    CursorRight,
    SelectLine(usize),
    InsertChar(char),
    Backspace,
    NewLine,
    DeleteLine,
    AppendLine(String),
    ClearScript,
    ClearLog,
    Save(Option<String>),
    Load(String),
    ListSaved,
    Move { dx: i16, dy: i16 },
    Resize { dw: i16, dh: i16 },
    SetRect { x: u16, y: u16, w: u16, h: u16 },
}

impl ScriptStudioOp {
    /// Canonical `:colon` string representation.
    pub fn colon_form(&self) -> String {
        match self {
            Self::Open => ":script open".into(),
            Self::Close => ":script close".into(),
            Self::Toggle => ":script toggle".into(),
            Self::Dismiss => ":script dismiss".into(),
            Self::Blur => ":script blur".into(),
            Self::Pin(Some(true)) => ":script pin on".into(),
            Self::Pin(Some(false)) => ":script pin off".into(),
            Self::Pin(None) => ":script pin".into(),
            Self::Dock(Some(true)) => ":script dock on".into(),
            Self::Dock(Some(false)) => ":script dock off".into(),
            Self::Dock(None) => ":script dock".into(),
            Self::RunAll => ":script run".into(),
            Self::StepNext => ":script step".into(),
            Self::ResetStep => ":script reset".into(),
            Self::Record(Some(true)) => ":script record on".into(),
            Self::Record(Some(false)) => ":script record off".into(),
            Self::Record(None) => ":script record".into(),
            Self::EditMode(Some(true)) => ":script edit on".into(),
            Self::EditMode(Some(false)) => ":script edit off".into(),
            Self::EditMode(None) => ":script edit".into(),
            Self::CursorUp => ":script up".into(),
            Self::CursorDown => ":script down".into(),
            Self::CursorLeft => ":script left".into(),
            Self::CursorRight => ":script right".into(),
            Self::SelectLine(idx) => format!(":script select {idx}"),
            Self::InsertChar(c) => format!(":script insert {c}"),
            Self::Backspace => ":script backspace".into(),
            Self::NewLine => ":script newline".into(),
            Self::DeleteLine => ":script delete".into(),
            Self::AppendLine(line) => format!(":script add {line}"),
            Self::ClearScript => ":script clear".into(),
            Self::ClearLog => ":script clear-log".into(),
            Self::Save(Some(name)) => format!(":script save {name}"),
            Self::Save(None) => ":script save".into(),
            Self::Load(name) => format!(":script load {name}"),
            Self::ListSaved => ":script list".into(),
            Self::Move { dx, dy } => format!(":script move {dx} {dy}"),
            Self::Resize { dw, dh } => format!(":script resize {dw} {dh}"),
            Self::SetRect { x, y, w, h } => format!(":script rect {x} {y} {w} {h}"),
        }
    }
}

/// A single execution log line produced by running a script line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptLogLine {
    pub line_no: usize,
    pub source: String,
    pub output: String,
    pub ok: bool,
}
