//! Reserved-chord data for the key-conflict engine (`KEY-02`, Phase 63-03). Data only.
//!
//! Seeded from 63-RESEARCH conflict matrices A and B. Every table carries a source tag per block:
//! `CITED` (vendor documentation named in the research) or `ASSUMED` (training knowledge that a
//! maintainer should confirm). Tables:
//! - [`RESERVED_HOST`]: chords consumed by the OS, desktop shell or terminal host (`Block` means the
//!   chord never reaches Bilterm, so binding it is rejected; `Warn` means it often does not).
//! - [`RESERVED_CONSUMERS`]: chords foreign programs inside a pane expect (always `Warn`).
//! - [`RESERVED_INTERNAL`]: Bilterm's own non-rebindable chords.
//! - [`ACCEPTED_DEFAULT_SHADOWS`]: shipped default bindings whose host/consumer clash is accepted.

use super::keyenv::{HostOs, Surface};

/// How serious a finding is. Ordered most severe first so a sort puts blockers on top.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Severity {
    /// The chord never reaches Bilterm or must never be rebound; user bindings are rejected.
    Block,
    /// The chord reaches Bilterm but shadows a common consumer or breaks typing on common layouts.
    Warn,
    /// Informational (an override, or an accepted shipped shadow with its reason).
    Info,
}

impl Severity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Warn => "warn",
            Self::Info => "info",
        }
    }
}

/// Which lookup produced a finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Class {
    Internal,
    Host,
    Consumer,
}

impl Class {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Internal => "internal",
            Self::Host => "host",
            Self::Consumer => "consumer",
        }
    }
}

/// What a table row matches.
#[derive(Clone, Copy, Debug)]
pub enum Pattern {
    /// One chord in the `KeyChord` grammar (`Alt+F4`). Shift is significant; letter case is not.
    Chord(&'static str),
    /// Any chord carrying the Super/Win modifier.
    AnySuper,
    /// `Ctrl+Alt+<letter>` (AltGr on many European layouts).
    CtrlAltLetter,
    /// `Alt+<printable>` (Option composes a symbol on macOS).
    AltPrintable,
}

/// One host-level row.
#[derive(Clone, Copy, Debug)]
pub struct ReservedChord {
    pub pattern: Pattern,
    /// `None` = every OS.
    pub os: Option<HostOs>,
    /// `None` = both surfaces.
    pub surface: Option<Surface>,
    pub severity: Severity,
    pub consumer: &'static str,
    pub note: &'static str,
}

const fn host(pattern: Pattern, os: Option<HostOs>, severity: Severity, consumer: &'static str, note: &'static str) -> ReservedChord {
    ReservedChord { pattern, os, surface: None, severity, consumer, note }
}

const fn tui_host(chord: &'static str, os: HostOs, consumer: &'static str, note: &'static str) -> ReservedChord {
    ReservedChord { pattern: Pattern::Chord(chord), os: Some(os), surface: Some(Surface::Tui), severity: Severity::Warn, consumer, note }
}

const fn c(chord: &'static str) -> Pattern {
    Pattern::Chord(chord)
}

const MAC: Option<HostOs> = Some(HostOs::MacOs);
const WIN: Option<HostOs> = Some(HostOs::Windows);
const LIN: Option<HostOs> = Some(HostOs::Linux);

/// Host-level reservations (matrix A, B rows macOS / Windows / GNOME-KDE-XFCE / Windows Terminal).
pub const RESERVED_HOST: &[ReservedChord] = &[
    // macOS. CITED (support.apple.com via search; defaults not individually confirmed).
    host(c("Cmd+Space"), MAC, Severity::Block, "macOS", "Spotlight"),
    host(c("Cmd+Tab"), MAC, Severity::Block, "macOS", "application switcher"),
    host(c("Cmd+H"), MAC, Severity::Block, "macOS", "hide application"),
    host(c("Cmd+M"), MAC, Severity::Block, "macOS", "minimize window"),
    host(c("Ctrl+Up"), MAC, Severity::Block, "macOS", "Mission Control"),
    host(c("Ctrl+Down"), MAC, Severity::Block, "macOS", "application windows"),
    host(c("Ctrl+Left"), MAC, Severity::Block, "macOS", "previous Space"),
    host(c("Ctrl+Right"), MAC, Severity::Block, "macOS", "next Space"),
    // ASSUMED: Option composes a symbol (Option+M is U+00B5) unless Option-as-Meta is enabled.
    host(Pattern::AltPrintable, MAC, Severity::Warn, "macOS Option key", "Option+key types a symbol unless Option is Meta"),
    // ASSUMED: Ctrl+Option is the VoiceOver modifier; Rectangle uses Ctrl+Option+Arrows.
    host(c("Ctrl+Alt+Left"), MAC, Severity::Warn, "macOS VoiceOver/Rectangle", "window tiling"),
    host(c("Ctrl+Alt+Right"), MAC, Severity::Warn, "macOS VoiceOver/Rectangle", "window tiling"),
    host(c("Ctrl+Alt+Up"), MAC, Severity::Warn, "macOS VoiceOver/Rectangle", "window tiling"),
    host(c("Ctrl+Alt+Down"), MAC, Severity::Warn, "macOS VoiceOver/Rectangle", "window tiling"),
    // macOS function row: media/Mission Control keys without Fn. VERIFIED: docs/LESSONS_LEARNED.md:42.
    host(c("F9"), MAC, Severity::Warn, "macOS", "Mission Control / media key without Fn"),
    host(c("F10"), MAC, Severity::Warn, "macOS", "media key without Fn"),
    host(c("F11"), MAC, Severity::Warn, "macOS", "show desktop / volume without Fn"),
    host(c("F12"), MAC, Severity::Warn, "macOS", "volume key without Fn"),
    // Windows. CITED (learn.microsoft.com/windows/terminal actions) for Alt+Space.
    host(c("Alt+F4"), WIN, Severity::Block, "Windows", "close window"),
    host(c("Alt+Tab"), WIN, Severity::Block, "Windows", "task switcher"),
    host(c("Ctrl+Alt+Delete"), WIN, Severity::Block, "Windows", "secure attention sequence"),
    host(Pattern::AnySuper, WIN, Severity::Block, "Windows", "Win-key chords belong to the shell"),
    host(c("Alt+Space"), WIN, Severity::Warn, "Windows", "system menu (Windows Terminal openSystemMenu)"),
    // ASSUMED: AltGr arrives as Ctrl+Alt on many European layouts.
    host(Pattern::CtrlAltLetter, WIN, Severity::Warn, "Windows AltGr", "AltGr letters type symbols on European layouts"),
    host(c("F10"), WIN, Severity::Warn, "Windows", "activates the window menu bar"),
    host(c("F11"), WIN, Severity::Warn, "Windows terminal hosts", "fullscreen"),
    // GNOME / KDE / XFCE. ASSUMED. Block only the chords every desktop consumes.
    host(c("Alt+F4"), LIN, Severity::Block, "Linux desktop", "close window"),
    host(c("Alt+Tab"), LIN, Severity::Block, "Linux desktop", "window switcher"),
    host(c("Alt+F2"), LIN, Severity::Block, "Linux desktop", "run dialog"),
    host(c("Ctrl+Alt+Delete"), LIN, Severity::Block, "Linux desktop", "logout / session dialog"),
    host(c("Ctrl+Alt+F1"), LIN, Severity::Block, "Linux console", "virtual terminal switch"),
    host(c("Ctrl+Alt+F2"), LIN, Severity::Block, "Linux console", "virtual terminal switch"),
    host(c("Ctrl+Alt+F3"), LIN, Severity::Block, "Linux console", "virtual terminal switch"),
    host(c("Ctrl+Alt+F4"), LIN, Severity::Block, "Linux console", "virtual terminal switch"),
    host(c("Ctrl+Alt+F5"), LIN, Severity::Block, "Linux console", "virtual terminal switch"),
    host(c("Ctrl+Alt+F6"), LIN, Severity::Block, "Linux console", "virtual terminal switch"),
    host(c("Ctrl+Alt+F7"), LIN, Severity::Block, "Linux console", "virtual terminal switch"),
    host(Pattern::AnySuper, LIN, Severity::Block, "Linux desktop", "Super chords belong to the compositor"),
    host(c("Alt+F1"), LIN, Severity::Warn, "Linux desktop", "application menu / overview"),
    host(c("Alt+F7"), LIN, Severity::Warn, "Linux desktop", "move window"),
    host(c("Alt+F8"), LIN, Severity::Warn, "Linux desktop", "resize window"),
    host(c("Alt+Space"), LIN, Severity::Warn, "Linux desktop", "window menu"),
    host(c("Alt+`"), LIN, Severity::Warn, "Linux desktop", "same-application window switch"),
    host(c("Ctrl+Alt+T"), LIN, Severity::Warn, "Linux desktop", "open terminal"),
    host(c("Ctrl+Alt+L"), LIN, Severity::Warn, "Linux desktop", "lock screen"),
    host(c("Ctrl+Alt+D"), LIN, Severity::Warn, "Linux desktop", "show desktop"),
    host(c("Ctrl+Alt+Left"), LIN, Severity::Warn, "Linux desktop", "switch workspace"),
    host(c("Ctrl+Alt+Right"), LIN, Severity::Warn, "Linux desktop", "switch workspace"),
    host(c("Ctrl+Alt+Up"), LIN, Severity::Warn, "Linux desktop", "switch workspace"),
    host(c("Ctrl+Alt+Down"), LIN, Severity::Warn, "Linux desktop", "switch workspace"),
    host(c("F11"), LIN, Severity::Warn, "Linux terminal hosts", "fullscreen"),
    // Windows Terminal as the host of a TUI. CITED (learn.microsoft.com/windows/terminal actions).
    tui_host("Alt+Left", HostOs::Windows, "Windows Terminal", "moveFocus"),
    tui_host("Alt+Right", HostOs::Windows, "Windows Terminal", "moveFocus"),
    tui_host("Alt+Up", HostOs::Windows, "Windows Terminal", "moveFocus"),
    tui_host("Alt+Down", HostOs::Windows, "Windows Terminal", "moveFocus"),
    tui_host("Alt+Shift+Left", HostOs::Windows, "Windows Terminal", "resizePane"),
    tui_host("Alt+Shift+Right", HostOs::Windows, "Windows Terminal", "resizePane"),
    tui_host("Alt+Shift+Up", HostOs::Windows, "Windows Terminal", "resizePane"),
    tui_host("Alt+Shift+Down", HostOs::Windows, "Windows Terminal", "resizePane"),
    tui_host("Alt+Enter", HostOs::Windows, "Windows Terminal", "fullscreen"),
    tui_host("Ctrl+Shift+W", HostOs::Windows, "Windows Terminal", "close pane"),
    tui_host("Ctrl+Shift+P", HostOs::Windows, "Windows Terminal", "command palette"),
    tui_host("Ctrl+Shift+F", HostOs::Windows, "Windows Terminal", "find"),
    tui_host("Ctrl+Shift+T", HostOs::Windows, "Windows Terminal", "new tab"),
    tui_host("Ctrl+Shift+N", HostOs::Windows, "Windows Terminal", "new window"),
    tui_host("Ctrl+Shift+D", HostOs::Windows, "Windows Terminal", "duplicate tab"),
    tui_host("Ctrl+Shift+1", HostOs::Windows, "Windows Terminal", "new tab from profile"),
    tui_host("Ctrl+Shift+2", HostOs::Windows, "Windows Terminal", "new tab from profile"),
    tui_host("Ctrl+Shift+3", HostOs::Windows, "Windows Terminal", "new tab from profile"),
    tui_host("Ctrl+Shift+4", HostOs::Windows, "Windows Terminal", "new tab from profile"),
    tui_host("Ctrl+Shift+5", HostOs::Windows, "Windows Terminal", "new tab from profile"),
    tui_host("Ctrl+Shift+6", HostOs::Windows, "Windows Terminal", "new tab from profile"),
    tui_host("Ctrl+Shift+7", HostOs::Windows, "Windows Terminal", "new tab from profile"),
    tui_host("Ctrl+Shift+8", HostOs::Windows, "Windows Terminal", "new tab from profile"),
    tui_host("Ctrl+Shift+9", HostOs::Windows, "Windows Terminal", "new tab from profile"),
    // GNOME Terminal / Konsole as the host of a TUI. ASSUMED.
    tui_host("Ctrl+Shift+T", HostOs::Linux, "terminal emulator", "new tab"),
    tui_host("Ctrl+Shift+N", HostOs::Linux, "terminal emulator", "new window"),
    tui_host("Ctrl+Shift+W", HostOs::Linux, "terminal emulator", "close tab"),
    tui_host("Ctrl+Shift+Q", HostOs::Linux, "terminal emulator", "close window"),
    tui_host("Ctrl+Shift+C", HostOs::Linux, "terminal emulator", "copy"),
    tui_host("Ctrl+Shift+V", HostOs::Linux, "terminal emulator", "paste"),
    tui_host("Ctrl+Shift+F", HostOs::Linux, "terminal emulator", "find"),
    tui_host("Ctrl+Shift+PageUp", HostOs::Linux, "terminal emulator", "previous tab"),
    tui_host("Ctrl+Shift+PageDown", HostOs::Linux, "terminal emulator", "next tab"),
];

/// One consumer row: every chord in `chords` is expected by `consumer` (severity is always Warn).
#[derive(Clone, Copy, Debug)]
pub struct ReservedSet {
    pub consumer: &'static str,
    pub note: &'static str,
    /// `None` = every OS.
    pub os: Option<HostOs>,
    pub chords: &'static [&'static str],
}

/// Foreign consumers inside a pane (matrix B). Source tags per block.
pub const RESERVED_CONSUMERS: &[ReservedSet] = &[
    // ASSUMED. bash/zsh emacs mode.
    ReservedSet {
        consumer: "readline",
        note: "line editing and history",
        os: None,
        chords: &[
            "Ctrl+A", "Ctrl+E", "Ctrl+B", "Ctrl+F", "Ctrl+P", "Ctrl+N", "Ctrl+K", "Ctrl+U", "Ctrl+W", "Ctrl+Y", "Ctrl+R",
            "Ctrl+S", "Ctrl+L", "Ctrl+D", "Ctrl+T", "Ctrl+H", "Ctrl+_", "Ctrl+X", "Alt+B", "Alt+F", "Alt+D", "Alt+T",
            "Alt+U", "Alt+L", "Alt+C", "Alt+R", "Alt+.", "Alt+Y", "Alt+<", "Alt+>", "Alt+Backspace",
        ],
    },
    // ASSUMED. emacs -nw.
    ReservedSet {
        consumer: "emacs",
        note: "C-x prefix, M-x, mark and undo",
        os: None,
        chords: &["Ctrl+X", "Alt+X", "Ctrl+G", "Ctrl+Space", "Ctrl+_", "Alt+M", "Ctrl+S"],
    },
    // ASSUMED. vim / neovim.
    ReservedSet {
        consumer: "vim",
        note: "window prefix, scrolling, completion",
        os: None,
        chords: &[
            "Ctrl+W", "Ctrl+B", "Ctrl+F", "Ctrl+D", "Ctrl+U", "Ctrl+E", "Ctrl+Y", "Ctrl+P", "Ctrl+N", "Ctrl+V", "Ctrl+O",
            "Ctrl+]", "Ctrl+Q",
        ],
    },
    // ASSUMED. tmux default prefix and Meta bindings.
    ReservedSet {
        consumer: "tmux",
        note: "prefix and layout keys",
        os: None,
        chords: &[
            "Ctrl+B", "Alt+1", "Alt+2", "Alt+3", "Alt+4", "Alt+5", "Alt+N", "Alt+P", "Alt+O", "Alt+Left", "Alt+Right",
            "Alt+Up", "Alt+Down",
        ],
    },
    // ASSUMED. GNU screen.
    ReservedSet { consumer: "GNU screen", note: "command prefix", os: None, chords: &["Ctrl+A"] },
    // ASSUMED. zellij.
    ReservedSet {
        consumer: "zellij",
        note: "mode keys",
        os: None,
        chords: &[
            "Ctrl+P", "Ctrl+T", "Ctrl+N", "Ctrl+H", "Ctrl+S", "Ctrl+O", "Ctrl+G", "Alt+H", "Alt+J", "Alt+K", "Alt+L",
            "Alt+N", "Alt+F", "Alt+[", "Alt+]", "Alt+=", "Alt+-",
        ],
    },
    // CITED (herdr.dev agent guide via web search). Configurable under [keys] in herdr's config.
    ReservedSet { consumer: "herdr", note: "default prefix", os: None, chords: &["Ctrl+B"] },
    // CITED (code.claude.com/docs/en/interactive-mode).
    ReservedSet {
        consumer: "Claude Code",
        note: "interactive-mode shortcuts",
        os: None,
        chords: &[
            "Ctrl+C", "Ctrl+D", "Ctrl+L", "Ctrl+O", "Ctrl+R", "Ctrl+T", "Ctrl+B", "Ctrl+G", "Ctrl+J", "Ctrl+V", "Ctrl+X",
            "Shift+Tab", "Alt+P", "Alt+T", "Alt+O", "Alt+B", "Alt+F", "Alt+D", "Alt+Y", "Alt+V",
        ],
    },
    ReservedSet { consumer: "Claude Code (Windows)", note: "Alt+M cycles permission mode when VT input is off", os: WIN, chords: &["Alt+M"] },
    // ASSUMED. Input-method switch on many desktops.
    ReservedSet { consumer: "input method", note: "input-source / IME toggle", os: None, chords: &["Ctrl+Space"] },
    // ASSUMED. SIGQUIT in a cooked tty, vim <C-\><C-n>.
    ReservedSet { consumer: "tty", note: "SIGQUIT / vim escape", os: None, chords: &["Ctrl+\\"] },
];

/// Bilterm's own non-rebindable chords. User bindings on these are `Block` findings.
pub const RESERVED_INTERNAL: &[(&str, &str)] = &[("Ctrl+Q", "quit always goes through the Exit Guard and cannot be rebound")];

/// Shipped default chords whose host/consumer clash is accepted, with the reason. A pattern ending
/// in `*` is a prefix match on the canonical ASCII form (`Alt+*`, `Ctrl+Alt+*`). Shipped defaults
/// may carry no unlisted `Warn` finding; the unit test fails on one.
pub const ACCEPTED_DEFAULT_SHADOWS: &[(&str, &str)] = &[
    ("Alt+*", "Alt layer: kept on windows/linux for one release (legacy_alt = auto); readline/tmux/Claude Code Meta chords are shadowed only while it is on; mac and portable have no Alt layer"),
    ("Ctrl+Alt+*", "Ctrl+Alt launchers are secondary paths only; leader and Primary are the documented routes (63-RESEARCH matrix A)"),
    ("Ctrl+Shift+*", "Primary on Windows/Linux needs a GUI or kitty keyboard protocol; hosts that consume it fall back to the leader and Window mode"),
    ("Ctrl+X", "default leader key; Ctrl+X prefix of readline/emacs is reachable by double-tap and the leader is configurable (leader_key)"),
    ("Ctrl+P", "palette shortcut: kept for compatibility, but Primary+P and Leader Space are the documented paths"),
    ("Ctrl+Q", "quit: must stay reachable; the Exit Guard protects it"),
    ("Ctrl+G", "raw toggle limited to the command prompt outside the legacy profile"),
    ("Ctrl+Space", "quake toggle on the legacy profile only; non-legacy profiles drop it"),
    ("Ctrl+\\", "quake toggle alias; F10 and the leader are the documented paths"),
    ("Ctrl+_", "font zoom out on the legacy profile only; non-legacy profiles drop it"),
    ("Shift+Tab", "reverse deck cycle; Claude Code inside a pane still gets it when the pane is in raw mode"),
    ("Cmd+Alt+*", "macOS focus aliases replace Alt+Arrow; no host reserves Cmd+Option+Arrows"),
    ("F9", "inspector: never the only path (:inspector, leader)"),
    ("F10", "quake toggle: never the only path (leader, palette)"),
    ("F11", "visual style: never the only path (:visual)"),
    ("F12", "raw toggle: never the only path (leader g)"),
];
