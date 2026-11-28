//! Words -> timed phoneme events with stress and phrase structure.

use super::phoneme::{Class, Ph};
use super::rules::word_to_phonemes;
use super::text::{Boundary, Item};

/// One timed event: a phoneme, or a silence (`ph == None`).
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub ph: Option<Ph>,
    /// Duration in milliseconds (speed already applied).
    pub dur_ms: f64,
    /// 0 unstressed, 1 primary stress.
    pub stress: u8,
    /// Boundary kind for phrase-ending pauses.
    pub boundary: Option<Boundary>,
}

const FUNCTION_WORDS: &[&str] = &[
    "a", "an", "the", "of", "to", "in", "on", "at", "by", "for", "and", "or", "but", "is", "are", "was", "be", "it",
    "as", "if", "so", "i", "he", "she", "we", "me", "my", "you", "do", "that", "this", "with", "from",
];

fn pause_ms(b: Boundary) -> f64 {
    match b {
        Boundary::Minor => 170.0,
        Boundary::Statement => 380.0,
        Boundary::Question | Boundary::Exclaim => 420.0,
    }
}

/// Builds events for `items`. Stops once the running time would exceed `max_ms` and reports truncation.
pub fn build_events(items: &[Item], speed: f64, max_ms: f64) -> (Vec<Event>, bool) {
    let mut events: Vec<Event> = Vec::new();
    let mut total = 0.0;
    let mut truncated = false;
    let scale = 1.0 / speed;
    let push = |events: &mut Vec<Event>, e: Event, total: &mut f64| -> bool {
        if *total + e.dur_ms > max_ms {
            return false;
        }
        *total += e.dur_ms;
        events.push(e);
        true
    };
    'outer: for (idx, item) in items.iter().enumerate() {
        match item {
            Item::Pause(b) => {
                // Fold repeated punctuation ("...", "?!") into one pause.
                if matches!(events.last(), Some(Event { boundary: Some(_), .. })) {
                    continue;
                }
                if events.is_empty() {
                    continue;
                }
                let e = Event { ph: None, dur_ms: (pause_ms(*b) * scale).max(60.0), stress: 0, boundary: Some(*b) };
                if !push(&mut events, e, &mut total) {
                    truncated = true;
                    break 'outer;
                }
            }
            Item::Word(w) => {
                let phones = word_to_phonemes(w);
                if phones.is_empty() {
                    continue;
                }
                let content = !FUNCTION_WORDS.contains(&w.as_str());
                let first_vowel = phones.iter().position(|p| p.is_vowel());
                let word_end = idx + 1 >= items.len() || matches!(items[idx + 1], Item::Pause(_));
                let last_vowel = phones.iter().rposition(|p| p.is_vowel());
                for (i, ph) in phones.iter().enumerate() {
                    let stressed = content && Some(i) == first_vowel;
                    let mut d = f64::from(ph.spec().dur_ms) * scale;
                    if ph.is_vowel() {
                        d *= if stressed { 1.25 } else { 0.85 };
                        if word_end && Some(i) == last_vowel {
                            d *= 1.2;
                        }
                    }
                    let e = Event { ph: Some(*ph), dur_ms: d, stress: u8::from(stressed), boundary: None };
                    if !push(&mut events, e, &mut total) {
                        truncated = true;
                        break 'outer;
                    }
                }
                let gap = Event { ph: None, dur_ms: 16.0 * scale, stress: 0, boundary: None };
                if !push(&mut events, gap, &mut total) {
                    truncated = true;
                    break 'outer;
                }
            }
        }
    }
    if !events.iter().any(|e| e.ph.is_some()) {
        events.clear();
    }
    (events, truncated)
}

/// True when `ph` should borrow the formants of the following vowel (aspiration, stop release).
pub fn takes_vowel_context(ph: Ph) -> bool {
    matches!(ph.spec().class, Class::Aspirate | Class::Stop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::text::normalize;

    #[test]
    fn builds_events_with_stress_and_pauses() {
        let (ev, trunc) = build_events(&normalize("hello world.", 100), 1.0, 60_000.0);
        assert!(!trunc);
        assert!(ev.iter().any(|e| e.stress == 1));
        assert_eq!(ev.last().unwrap().boundary, Some(Boundary::Statement));
        assert!(ev.iter().filter(|e| e.ph.is_some()).count() >= 8);
    }

    #[test]
    fn speed_scales_duration() {
        let items = normalize("testing speed", 100);
        let total = |s| build_events(&items, s, 1e9).0.iter().map(|e| e.dur_ms).sum::<f64>();
        assert!(total(2.0) < total(1.0) * 0.6);
    }

    #[test]
    fn truncates_at_the_time_cap() {
        let items = normalize(&"banana ".repeat(200), 5_000);
        let (ev, trunc) = build_events(&items, 1.0, 2_000.0);
        assert!(trunc);
        assert!(ev.iter().map(|e| e.dur_ms).sum::<f64>() <= 2_000.0);
    }

    #[test]
    fn punctuation_only_yields_nothing() {
        assert!(build_events(&normalize("...?!", 100), 1.0, 1e9).0.is_empty());
    }
}
