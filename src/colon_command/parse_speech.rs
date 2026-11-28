//! Parser for `:say` / `:speak` / `:tts` (`TTS-01`, `UNI-R01`).

use crate::action::{Action, SpeechOp};
use crate::colon_command::ColonCommand;
use crate::speech::cli::{parse_say_args, usage};

/// `parts[0]` is the command word. `:say stop|status|voices` are verbs; use `:say -- stop` to speak the word.
pub fn parse_speech_colon_command(parts: &[&str]) -> Option<Result<ColonCommand, String>> {
    let args = &parts[1..];
    let op = match args {
        [] => return Some(Err(format!("usage: :say [--voice V] [--speed X] [--pitch HZ] [--wav NAME] TEXT | stop | status | voices\n{}", usage()))),
        ["stop"] | ["cancel"] | ["shutup"] => Ok(SpeechOp::Stop),
        ["status"] => Ok(SpeechOp::Status),
        ["voices"] => Ok(SpeechOp::Voices),
        _ => parse_say_args(args).and_then(|a| {
            SpeechOp::say(&a.text, a.voice.as_deref(), a.speed, a.pitch, a.wav.as_deref())
        }),
    };
    Some(op.map(Action::Speech))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colon_command::parse_colon_command;

    fn p(s: &str) -> Result<ColonCommand, String> {
        parse_colon_command(s).unwrap()
    }

    #[test]
    fn say_with_options() {
        let Ok(Action::Speech(SpeechOp::Say { text, voice, speed_pct, pitch_hz, wav })) =
            p(":say --voice Tron --speed 1.5 --pitch 90 --wav hi hello there")
        else {
            panic!("not a say");
        };
        assert_eq!((text.as_str(), voice.as_deref(), speed_pct, pitch_hz, wav.as_deref()), ("hello there", Some("tron"), Some(150), Some(90), Some("hi")));
    }

    #[test]
    fn verbs_aliases_and_literal_words() {
        assert_eq!(p(":say stop"), Ok(Action::Speech(SpeechOp::Stop)));
        assert_eq!(p(":speak status"), Ok(Action::Speech(SpeechOp::Status)));
        assert_eq!(p(":tts voices"), Ok(Action::Speech(SpeechOp::Voices)));
        assert!(matches!(p(":say -- stop"), Ok(Action::Speech(SpeechOp::Say { .. }))));
        assert!(matches!(p(":say stop the presses"), Ok(Action::Speech(SpeechOp::Say { .. }))));
    }

    #[test]
    fn errors_are_reported_not_swallowed() {
        assert!(p(":say").is_err());
        assert!(p(":say --voice hal hi").unwrap_err().contains("unknown voice"));
        assert!(p(":say --speed 9 hi").unwrap_err().contains("out of range"));
        assert!(p(":say --wav ../x hi").unwrap_err().contains("wav name"));
        assert!(p(":say --voice tron").is_err());
        assert!(p(&format!(":say {}", "a".repeat(2001))).unwrap_err().contains("too long"));
    }
}
