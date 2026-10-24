use ratatui::style::{Color, Modifier, Style};

/// 24-bit RGB color triplet used in renderer-agnostic [`GridSnapshot`] frames (`ARCH-01`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RgbTriplet {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbTriplet {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn to_ratatui_color(self) -> Color {
        Color::Rgb(self.r, self.g, self.b)
    }
}

/// Terminal cell color representation decoupled from any specific UI framework (`Ratatui`, `GPUI`, `egui`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CellColor {
    #[default]
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl CellColor {
    pub const fn to_ratatui_color(self) -> Color {
        match self {
            Self::Default => Color::Reset,
            Self::Indexed(idx) => Color::Indexed(idx),
            Self::Rgb(r, g, b) => Color::Rgb(r, g, b),
        }
    }
}

/// Extended SGR underline styles (`SGR 4`, `SGR 21`, `SGR 4:0..4:5`, `FND-09`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UnderlineKind {
    #[default]
    None,
    /// `SGR 4` or `SGR 4:1`
    Single,
    /// `SGR 21` or `SGR 4:2`
    Double,
    /// `SGR 4:3` (LSP diagnostic squiggly underline)
    Curly,
    /// `SGR 4:4`
    Dotted,
    /// `SGR 4:5`
    Dashed,
}

impl UnderlineKind {
    pub const fn is_underlined(self) -> bool {
        !matches!(self, Self::None)
    }
}

/// Visual cursor shape reported in a [`CursorSnapshot`] (`DECSCUSR`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorShape {
    #[default]
    Block,
    HollowBlock,
    Underline,
    Bar,
}

/// Point-in-time cursor state within a [`GridSnapshot`] (`ARCH-01`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorSnapshot {
    pub row: u16,
    pub col: u16,
    pub visible: bool,
    pub blinking: bool,
    pub shape: CursorShape,
    pub at_wide_tail: bool,
}

impl Default for CursorSnapshot {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            visible: true,
            blinking: false,
            shape: CursorShape::Block,
            at_wide_tail: false,
        }
    }
}

/// Point-in-time snapshot of a single terminal cell (`ARCH-01`, `FND-07`, `FND-09`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellSnapshot {
    /// Grapheme cluster string (`""` or `" "` for blank; full ZWJ sequence under `DECSET ?2027`).
    pub grapheme: String,
    /// Display column width (`0` = wide continuation tail, `1` = normal, `2` = wide CJK/Emoji).
    pub width: u8,
    pub fg: CellColor,
    pub bg: CellColor,
    /// Optional explicit underline RGB color (`SGR 58;2;r;g;b` or `SGR 58:2::r:g:b`, `FND-09`).
    pub underline_color: Option<RgbTriplet>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: UnderlineKind,
    /// `SGR 53` overline attribute (`FND-09`).
    pub overline: bool,
    pub inverse: bool,
    pub strikethrough: bool,
}

impl Default for CellSnapshot {
    fn default() -> Self {
        Self {
            grapheme: " ".to_string(),
            width: 1,
            fg: CellColor::Default,
            bg: CellColor::Default,
            underline_color: None,
            bold: false,
            dim: false,
            italic: false,
            underline: UnderlineKind::None,
            overline: false,
            inverse: false,
            strikethrough: false,
        }
    }
}

impl CellSnapshot {
    /// Converts this cell's visual attributes into a `ratatui::style::Style`.
    pub fn to_ratatui_style(&self) -> Style {
        let mut style = Style::default()
            .fg(self.fg.to_ratatui_color())
            .bg(self.bg.to_ratatui_color());

        let mut mods = Modifier::empty();
        if self.bold {
            mods |= Modifier::BOLD;
        }
        if self.dim {
            mods |= Modifier::DIM;
        }
        if self.italic {
            mods |= Modifier::ITALIC;
        }
        if self.underline.is_underlined() {
            mods |= Modifier::UNDERLINED;
        }
        if self.inverse {
            mods |= Modifier::REVERSED;
        }
        if self.strikethrough {
            mods |= Modifier::CROSSED_OUT;
        }
        style = style.add_modifier(mods);
        if let Some(ul_rgb) = self.underline_color {
            style = style.underline_color(ul_rgb.to_ratatui_color());
        }
        style
    }

    pub fn is_wide(&self) -> bool {
        self.width == 2
    }

    pub fn is_wide_continuation(&self) -> bool {
        self.width == 0
    }

    /// Returns a zero-allocation borrowed [`CellView`] of this snapshot cell (`ARCH-R03`).
    pub fn as_view(&self) -> CellView<'_> {
        CellView {
            grapheme: self.grapheme.as_str(),
            width: self.width,
            fg: self.fg,
            bg: self.bg,
            bold: self.bold,
            italic: self.italic,
            underline: self.underline,
            inverse: self.inverse,
        }
    }
}

/// Zero-allocation borrowed view of a single terminal cell (`ARCH-R03`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellView<'a> {
    pub grapheme: &'a str,
    pub width: u8,
    pub fg: CellColor,
    pub bg: CellColor,
    pub bold: bool,
    pub italic: bool,
    pub underline: UnderlineKind,
    pub inverse: bool,
}

impl<'a> CellView<'a> {
    pub const fn is_wide(&self) -> bool {
        self.width == 2
    }

    pub const fn is_wide_continuation(&self) -> bool {
        self.width == 0
    }

    pub fn to_ratatui_style(&self) -> Style {
        let style = Style::default()
            .fg(self.fg.to_ratatui_color())
            .bg(self.bg.to_ratatui_color());
        let mut modifiers = Modifier::empty();
        if self.bold {
            modifiers |= Modifier::BOLD;
        }
        if self.italic {
            modifiers |= Modifier::ITALIC;
        }
        if self.underline.is_underlined() {
            modifiers |= Modifier::UNDERLINED;
        }
        if self.inverse {
            modifiers |= Modifier::REVERSED;
        }
        style.add_modifier(modifiers)
    }
}

/// Point-in-time snapshot of a single terminal row (`ARCH-01`, `FND-08`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowSnapshot {
    pub cells: Vec<CellSnapshot>,
    /// Whether this row changed since the last frame render.
    pub dirty: bool,
    /// Whether this row soft-wrapped onto the next row (`FND-08`).
    pub wrapped: bool,
}

impl RowSnapshot {
    pub fn new_blank(cols: u16) -> Self {
        Self {
            cells: vec![CellSnapshot::default(); usize::from(cols)],
            dirty: true,
            wrapped: false,
        }
    }
}

/// Immutable, renderer-agnostic point-in-time snapshot of a terminal viewport (`ARCH-01`).
///
/// Consumed identically by the `Ratatui` TUI compositor and native GPU window frontends (`GPUI` / `egui`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridSnapshot {
    pub rows: u16,
    pub cols: u16,
    pub grid_rows: Vec<RowSnapshot>,
    pub cursor: CursorSnapshot,
    pub alternate_screen: bool,
    pub scrollback_offset: usize,
    pub total_scrollback_rows: usize,
    pub generation: u64,
}

impl GridSnapshot {
    pub fn empty(rows: u16, cols: u16) -> Self {
        let r = rows.max(1);
        let c = cols.max(1);
        Self {
            rows: r,
            cols: c,
            grid_rows: (0..r).map(|_| RowSnapshot::new_blank(c)).collect(),
            cursor: CursorSnapshot::default(),
            alternate_screen: false,
            scrollback_offset: 0,
            total_scrollback_rows: 0,
            generation: 0,
        }
    }

    /// Returns a reference to the [`CellSnapshot`] at `(row, col)`.
    pub fn cell(&self, row: u16, col: u16) -> Option<&CellSnapshot> {
        self.grid_rows
            .get(usize::from(row))
            .and_then(|r| r.cells.get(usize::from(col)))
    }

    /// Returns the plain text contents of `row` with trailing spaces trimmed.
    pub fn row_text(&self, row: u16) -> String {
        let Some(r) = self.grid_rows.get(usize::from(row)) else {
            return String::new();
        };
        let mut out = String::with_capacity(r.cells.len());
        for cell in &r.cells {
            if cell.is_wide_continuation() {
                continue;
            }
            if cell.grapheme.is_empty() {
                out.push(' ');
            } else {
                out.push_str(&cell.grapheme);
            }
        }
        out.trim_end().to_string()
    }

    /// Returns the full viewport text contents as newline-joined rows with trailing blank rows trimmed.
    pub fn contents(&self) -> String {
        let mut lines: Vec<String> = (0..self.rows).map(|r| self.row_text(r)).collect();
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        lines.join("\n")
    }
}
