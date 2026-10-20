use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Semantic category for a shell syntax token in the `FloatingCommandPrompt` (`PANE-04`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxTokenKind {
    Command,
    Flag,
    PipeOrOperator,
    StringLiteral,
    Variable,
    Comment,
    ErrorUnclosedQuote,
    Plain,
}

/// A single lexical token produced by the shell syntax highlighter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxToken {
    pub text: String,
    pub kind: SyntaxTokenKind,
}

impl SyntaxTokenKind {
    /// Maps a `SyntaxTokenKind` to the Borland Turbo Vision / IBM 3270 syntax palette (`02-UI-SPEC.md`).
    pub fn style(self) -> Style {
        match self {
            Self::Command => Style::default()
                .fg(Color::Rgb(85, 255, 85))
                .add_modifier(Modifier::BOLD),
            Self::Flag => Style::default().fg(Color::Rgb(85, 255, 255)),
            Self::PipeOrOperator => Style::default()
                .fg(Color::Rgb(255, 255, 85))
                .add_modifier(Modifier::BOLD),
            Self::StringLiteral => Style::default().fg(Color::Rgb(255, 176, 0)),
            Self::Variable => Style::default().fg(Color::Rgb(255, 85, 255)),
            Self::Comment => Style::default().fg(Color::Rgb(170, 170, 170)),
            Self::ErrorUnclosedQuote => Style::default()
                .fg(Color::Rgb(255, 85, 85))
                .add_modifier(Modifier::UNDERLINED),
            Self::Plain => Style::default().fg(Color::Rgb(255, 255, 255)),
        }
    }
}

/// Tokenizes a single line of shell input into semantic `SyntaxToken`s.
///
/// Tracks command-position resets across pipes (`|`, `||`, `&&`, `;`), flags (`-f`, `--flag`),
/// environment variables (`$VAR`, `${VAR}`), comments (`#...`), and quoted strings (`'...'`, `"..."`),
/// highlighting unclosed quotes as `SyntaxTokenKind::ErrorUnclosedQuote`.
pub fn tokenize_shell_line(line: &str) -> Vec<SyntaxToken> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut expect_command = true;

    while i < n {
        let ch = chars[i];

        // 1. Whitespace
        if ch.is_whitespace() {
            let start = i;
            while i < n && chars[i].is_whitespace() {
                i += 1;
            }
            tokens.push(SyntaxToken {
                text: chars[start..i].iter().collect(),
                kind: SyntaxTokenKind::Plain,
            });
            continue;
        }

        // 2. Comment at start of word
        if ch == '#' {
            tokens.push(SyntaxToken {
                text: chars[i..].iter().collect(),
                kind: SyntaxTokenKind::Comment,
            });
            break;
        }

        // 3. Operators / Pipes / Redirects (|, ||, &&, ;, >>, >, <, &)
        if matches!(ch, '|' | '&' | ';' | '>' | '<') {
            let start = i;
            if i + 1 < n
                && ((ch == '|' && chars[i + 1] == '|')
                    || (ch == '&' && chars[i + 1] == '&')
                    || (ch == '>' && chars[i + 1] == '>'))
            {
                i += 2;
            } else {
                i += 1;
            }
            let op_text: String = chars[start..i].iter().collect();
            if matches!(op_text.as_str(), "|" | "||" | "&&" | ";") {
                expect_command = true;
            }
            tokens.push(SyntaxToken {
                text: op_text,
                kind: SyntaxTokenKind::PipeOrOperator,
            });
            continue;
        }

        // 4. Quoted strings ('...' or "...")
        if ch == '\'' || ch == '"' {
            let quote = ch;
            let start = i;
            i += 1;
            let mut closed = false;
            while i < n {
                if quote == '"' && chars[i] == '\\' && i + 1 < n {
                    i += 2;
                    continue;
                }
                if chars[i] == quote {
                    i += 1;
                    closed = true;
                    break;
                }
                i += 1;
            }
            expect_command = false;
            tokens.push(SyntaxToken {
                text: chars[start..i].iter().collect(),
                kind: if closed {
                    SyntaxTokenKind::StringLiteral
                } else {
                    SyntaxTokenKind::ErrorUnclosedQuote
                },
            });
            continue;
        }

        // 5. Variables ($VAR or ${VAR})
        if ch == '$' {
            let start = i;
            i += 1;
            if i < n && chars[i] == '{' {
                while i < n && chars[i] != '}' {
                    i += 1;
                }
                if i < n && chars[i] == '}' {
                    i += 1;
                }
            } else {
                while i < n && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '?') {
                    i += 1;
                }
            }
            expect_command = false;
            tokens.push(SyntaxToken {
                text: chars[start..i].iter().collect(),
                kind: SyntaxTokenKind::Variable,
            });
            continue;
        }

        // 6. Standard word (command, flag, or argument)
        let start = i;
        while i < n
            && !chars[i].is_whitespace()
            && !matches!(chars[i], '|' | '&' | ';' | '>' | '<' | '\'' | '"' | '$')
        {
            i += 1;
        }
        let word: String = chars[start..i].iter().collect();
        let kind = if expect_command {
            // Check for ENV=val assignment before command
            if word.contains('=') && !word.starts_with('=') {
                SyntaxTokenKind::Variable
            } else {
                expect_command = false;
                SyntaxTokenKind::Command
            }
        } else if word.starts_with('-') {
            SyntaxTokenKind::Flag
        } else {
            SyntaxTokenKind::Plain
        };

        tokens.push(SyntaxToken { text: word, kind });
    }

    tokens
}

/// Converts a shell line into a styled Ratatui `Line` with syntax highlighting.
pub fn highlight_shell_line(line: &str) -> Line<'static> {
    let spans: Vec<Span<'static>> = tokenize_shell_line(line)
        .into_iter()
        .map(|tok| Span::styled(tok.text, tok.kind.style()))
        .collect();
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_commands_flags_pipes_strings_variables_and_unclosed_quotes() {
        let tokens = tokenize_shell_line("git log --oneline | grep \"fix\" $USER 'unclosed");
        let non_ws: Vec<(String, SyntaxTokenKind)> = tokens
            .into_iter()
            .filter(|t| !t.text.trim().is_empty())
            .map(|t| (t.text, t.kind))
            .collect();

        assert_eq!(
            non_ws,
            vec![
                ("git".to_string(), SyntaxTokenKind::Command),
                ("log".to_string(), SyntaxTokenKind::Plain),
                ("--oneline".to_string(), SyntaxTokenKind::Flag),
                ("|".to_string(), SyntaxTokenKind::PipeOrOperator),
                ("grep".to_string(), SyntaxTokenKind::Command),
                ("\"fix\"".to_string(), SyntaxTokenKind::StringLiteral),
                ("$USER".to_string(), SyntaxTokenKind::Variable),
                ("'unclosed".to_string(), SyntaxTokenKind::ErrorUnclosedQuote),
            ]
        );
    }
}
