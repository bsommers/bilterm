//! Letter-to-sound engine: applies [`super::rules_data::RULES`] plus a small exception lexicon.

use super::phoneme::Ph;
use super::rules_data::RULES;
use std::collections::HashMap;
use std::sync::OnceLock;

struct Rule {
    left: Vec<u8>,
    mat: Vec<u8>,
    right: Vec<u8>,
    out: Vec<Ph>,
}

fn table() -> &'static HashMap<u8, Vec<Rule>> {
    static T: OnceLock<HashMap<u8, Vec<Rule>>> = OnceLock::new();
    T.get_or_init(|| {
        let mut map: HashMap<u8, Vec<Rule>> = HashMap::new();
        for line in RULES.lines() {
            let (Some(lb), Some(rb), Some(eq)) = (line.find('['), line.find(']'), line.find('=')) else {
                continue;
            };
            let mat = line[lb + 1..rb].to_ascii_lowercase().into_bytes();
            let rule = Rule {
                left: line[..lb].to_ascii_lowercase().into_bytes(),
                right: line[rb + 1..eq].to_ascii_lowercase().into_bytes(),
                out: line[eq + 1..].split_whitespace().filter_map(Ph::from_symbol).collect(),
                mat: mat.clone(),
            };
            map.entry(mat[0]).or_default().push(rule);
        }
        map
    })
}

fn is_vowel(c: u8) -> bool {
    matches!(c, b'a' | b'e' | b'i' | b'o' | b'u' | b'y')
}
fn is_cons(c: u8) -> bool {
    c.is_ascii_lowercase() && !is_vowel(c)
}
fn is_voiced(c: u8) -> bool {
    matches!(c, b'b' | b'd' | b'g' | b'j' | b'l' | b'm' | b'n' | b'r' | b'v' | b'w' | b'z')
}

/// Matches the right context `pat` against `w` starting at `pos`.
fn match_right(w: &[u8], mut pos: usize, pat: &[u8]) -> bool {
    let mut i = 0;
    while i < pat.len() {
        let p = pat[i];
        let c = w.get(pos).copied().unwrap_or(b'_');
        match p {
            b'#' => {
                if !is_vowel(c) {
                    return false;
                }
                while w.get(pos).copied().is_some_and(is_vowel) {
                    pos += 1;
                }
                i += 1;
                continue;
            }
            b':' => {
                while w.get(pos).copied().is_some_and(is_cons) {
                    pos += 1;
                }
                i += 1;
                continue;
            }
            b'%' => {
                for suf in [&b"ely"[..], b"ing", b"er", b"es", b"ed", b"e"] {
                    if w[pos.min(w.len())..].starts_with(suf) && w.get(pos + suf.len()).copied().unwrap_or(b'_') == b'_' {
                        pos += suf.len();
                        i += 1;
                        // `%` must be last-ish; remaining pattern continues after the suffix.
                        return match_right(w, pos, &pat[i..]);
                    }
                }
                return false;
            }
            _ => {
                if !class_ok(p, c) {
                    return false;
                }
            }
        }
        pos += 1;
        i += 1;
    }
    true
}

/// Matches the left context `pat` (read right to left) ending just before `pos`.
fn match_left(w: &[u8], pos: usize, pat: &[u8]) -> bool {
    let mut pos = pos as isize;
    for &p in pat.iter().rev() {
        let at = |i: isize| if i < 0 { b'_' } else { w.get(i as usize).copied().unwrap_or(b'_') };
        match p {
            b'#' => {
                if !is_vowel(at(pos - 1)) {
                    return false;
                }
                while is_vowel(at(pos - 1)) {
                    pos -= 1;
                }
            }
            b':' => {
                while is_cons(at(pos - 1)) {
                    pos -= 1;
                }
            }
            _ => {
                if !class_ok(p, at(pos - 1)) {
                    return false;
                }
                pos -= 1;
            }
        }
    }
    true
}

fn class_ok(p: u8, c: u8) -> bool {
    match p {
        b'^' => is_cons(c),
        b'.' => is_voiced(c),
        b'+' => matches!(c, b'e' | b'i' | b'y'),
        b'&' => matches!(c, b's' | b'c' | b'g' | b'z' | b'x' | b'j'),
        b'@' => matches!(c, b't' | b's' | b'r' | b'd' | b'l' | b'z' | b'n' | b'j'),
        b'_' => c == b'_',
        lit => lit == c,
    }
}

/// Exception lexicon (lowercase, apostrophes removed). Values are space-separated ARPAbet.
const LEXICON: &[(&str, &str)] = &[
    ("a", "AX"), ("the", "DH AX"), ("of", "AX V"), ("to", "T UW"), ("you", "Y UW"), ("your", "Y AO R"),
    ("are", "AA R"), ("one", "W AH N"), ("two", "T UW"), ("do", "D UW"), ("does", "D AH Z"), ("said", "S EH D"),
    ("they", "DH EY"), ("have", "HH AE V"), ("from", "F R AH M"), ("there", "DH EH R"), ("where", "W EH R"),
    ("what", "W AH T"), ("who", "HH UW"), ("come", "K AH M"), ("some", "S AH M"), ("could", "K UH D"),
    ("would", "W UH D"), ("should", "SH UH D"), ("was", "W AH Z"), ("is", "IH Z"), ("his", "HH IH Z"),
    ("as", "AE Z"), ("has", "HH AE Z"), ("be", "B IY"), ("he", "HH IY"), ("she", "SH IY"), ("we", "W IY"),
    ("me", "M IY"), ("my", "M AY"), ("by", "B AY"), ("no", "N OW"), ("so", "S OW"), ("go", "G OW"),
    ("yes", "Y EH S"), ("hello", "HH AX L OW"), ("world", "W ER L D"), ("robot", "R OW B AA T"),
    ("bilterm", "B IH L T ER M"), ("tron", "T R AA N"), ("ok", "OW K EY"), ("okay", "OW K EY"),
    ("mr", "M IH S T ER"), ("mrs", "M IH S IH Z"), ("dr", "D AA K T ER"), ("vs", "V ER S AX S"),
    ("dont", "D OW N T"), ("cant", "K AE N T"), ("wont", "W OW N T"), ("isnt", "IH Z AX N T"),
    ("arent", "AA R AX N T"), ("doesnt", "D AH Z AX N T"), ("didnt", "D IH D AX N T"), ("im", "AY M"),
    ("ive", "AY V"), ("youre", "Y UH R"), ("thats", "DH AE T S"), ("whats", "W AH T S"), ("lets", "L EH T S"),
    ("theres", "DH EH R Z"), ("its", "IH T S"), ("into", "IH N T UW"), ("onto", "AA N T UW"),
    ("and", "AE N D"), ("or", "AO R"), ("for", "F AO R"), ("computer", "K AX M P Y UW T ER"),
    ("terminal", "T ER M AX N AX L"), ("speech", "S P IY CH"), ("voice", "V OY S"),
    ("zero", "Z IY R OW"), ("one", "W AH N"), ("three", "TH R IY"), ("four", "F AO R"), ("five", "F AY V"),
    ("six", "S IH K S"), ("seven", "S EH V AX N"), ("eight", "EY T"), ("nine", "N AY N"), ("ten", "T EH N"),
    ("eleven", "IH L EH V AX N"), ("twelve", "T W EH L V"), ("thirteen", "TH ER T IY N"),
    ("fourteen", "F AO R T IY N"), ("fifteen", "F IH F T IY N"), ("sixteen", "S IH K S T IY N"),
    ("seventeen", "S EH V AX N T IY N"), ("eighteen", "EY T IY N"), ("nineteen", "N AY N T IY N"),
    ("twenty", "T W EH N T IY"), ("thirty", "TH ER T IY"), ("forty", "F AO R T IY"), ("fifty", "F IH F T IY"),
    ("sixty", "S IH K S T IY"), ("seventy", "S EH V AX N T IY"), ("eighty", "EY T IY"), ("ninety", "N AY N T IY"),
    ("hundred", "HH AH N D R AX D"), ("thousand", "TH AW Z AX N D"), ("million", "M IH L Y AX N"),
    ("billion", "B IH L Y AX N"), ("point", "P OY N T"), ("minus", "M AY N AX S"), ("percent", "P ER S EH N T"),
    ("and", "AE N D"), ("plus", "P L AH S"), ("equals", "IY K W AX L Z"), ("at", "AE T"), ("number", "N AH M B ER"),
    ("slash", "S L AE SH"), ("dollar", "D AA L ER"), ("dollars", "D AA L ER Z"),
];

fn lexicon() -> &'static HashMap<&'static str, Vec<Ph>> {
    static L: OnceLock<HashMap<&'static str, Vec<Ph>>> = OnceLock::new();
    L.get_or_init(|| {
        LEXICON
            .iter()
            .map(|(w, p)| (*w, p.split_whitespace().filter_map(Ph::from_symbol).collect()))
            .collect()
    })
}

/// Spoken names of the letters, used for acronyms and vowel-less words.
const LETTER_NAMES: [&str; 26] = [
    "EY", "B IY", "S IY", "D IY", "IY", "EH F", "JH IY", "EY CH", "AY", "JH EY", "K EY", "EH L", "EH M",
    "EH N", "OW", "P IY", "K Y UW", "AA R", "EH S", "T IY", "Y UW", "V IY", "D AH B AX L Y UW", "EH K S",
    "W AY", "Z IY",
];

fn spell(word: &str) -> Vec<Ph> {
    let mut out = Vec::new();
    for b in word.bytes().filter(u8::is_ascii_lowercase) {
        out.extend(LETTER_NAMES[(b - b'a') as usize].split_whitespace().filter_map(Ph::from_symbol));
    }
    out
}

/// Converts one lowercase ASCII word (letters only) to phonemes.
pub fn word_to_phonemes(word: &str) -> Vec<Ph> {
    if let Some(p) = lexicon().get(word) {
        return p.clone();
    }
    if !word.bytes().any(is_vowel) {
        return spell(word);
    }
    let mut w = Vec::with_capacity(word.len() + 2);
    w.push(b'_');
    w.extend(word.bytes());
    w.push(b'_');
    let t = table();
    let mut out = Vec::new();
    let mut i = 1;
    while i < w.len() - 1 {
        let Some(rules) = t.get(&w[i]) else {
            i += 1;
            continue;
        };
        let mut matched = false;
        for r in rules {
            let end = i + r.mat.len();
            if end < w.len()
                && w[i..end] == r.mat[..]
                && match_left(&w, i, &r.left)
                && match_right(&w, end, &r.right)
            {
                out.extend_from_slice(&r.out);
                i = end;
                matched = true;
                break;
            }
        }
        if !matched {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(word: &str) -> String {
        word_to_phonemes(word).iter().map(|p| p.symbol()).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn rules_parse_into_a_table() {
        let t = table();
        assert!(t.len() >= 24, "every letter has rules");
        assert!(RULES.lines().count() > 300);
    }

    #[test]
    fn common_words() {
        assert_eq!(p("cat"), "K AE T");
        assert_eq!(p("hello"), "HH AX L OW");
        assert_eq!(p("the"), "DH AX");
        assert_eq!(p("speech"), "S P IY CH");
        assert_eq!(p("make"), "M EY K");
        assert_eq!(p("this"), "DH IH S");
        assert_eq!(p("dog"), "D AA G");
        assert_eq!(p("ship"), "SH IH P");
        assert_eq!(p("phone"), "F OW N");
        assert_eq!(p("sing"), "S IH NG");
    }

    #[test]
    fn vowelless_words_are_spelled() {
        assert_eq!(p("tcp"), "T IY S IY P IY");
        assert_eq!(p("ssh"), "EH S EH S EY CH");
    }

    #[test]
    fn deterministic_and_total() {
        for w in ["rhythm", "queue", "xylophone", "strength", "bilterm", "zzz", "a", "ii"] {
            assert_eq!(word_to_phonemes(w), word_to_phonemes(w));
        }
        assert!(!p("synthesizer").is_empty());
    }
}
