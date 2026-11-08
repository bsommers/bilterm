//! Parser for `:voice` and `:stt` colon commands (`STT-03`, `UNI-R01`).

use crate::action::{Action, VoiceOp};
use crate::colon_command::ColonCommand;
use crate::stt::SttBackendKind;

pub fn parse_voice_colon_command(head: &str, parts: &[&str]) -> Option<Result<ColonCommand, String>> {
    if head != "voice" && head != "stt" {
        return None;
    }

    let sub = parts.get(1).copied().unwrap_or("toggle");
    let result = match sub {
        "toggle" => Ok(Action::Voice(VoiceOp::Toggle)),
        "start" => {
            let pane_id = if let Some(id_str) = parts.get(2) {
                match id_str.parse::<usize>() {
                    Ok(id) => Some(id),
                    Err(_) => return Some(Err(format!("Invalid pane id '{id_str}'"))),
                }
            } else {
                None
            };
            Ok(Action::Voice(VoiceOp::Start { pane_id }))
        }
        "stop" => Ok(Action::Voice(VoiceOp::Stop)),
        "cancel" | "abort" => Ok(Action::Voice(VoiceOp::Cancel)),
        "status" => Ok(Action::Voice(VoiceOp::Status)),
        "provider" => {
            if let Some(kind_str) = parts.get(2) {
                let kind = match kind_str.to_ascii_lowercase().as_str() {
                    "openai" | "openai_compatible" | "whisper" | "faster-whisper" | "speaches" => {
                        SttBackendKind::OpenAiCompatible
                    }
                    "ollama" => SttBackendKind::Ollama,
                    "cli" | "local_cli" | "whisper-cli" => SttBackendKind::LocalCli,
                    "mock" => SttBackendKind::Mock,
                    other => {
                        return Some(Err(format!(
                            "Unknown STT provider '{other}' (expected openai, ollama, cli, or mock)"
                        )));
                    }
                };
                Ok(Action::Voice(VoiceOp::SetProvider { provider: kind }))
            } else {
                Ok(Action::Voice(VoiceOp::Status))
            }
        }
        other => Err(format!(
            "Unknown voice subcommand '{other}' (usage: :voice [toggle|start|stop|cancel|status|provider <kind>])"
        )),
    };

    Some(result)
}
