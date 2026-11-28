//! Text normalisation: raw (untrusted) text -> a flat list of lowercase words and pauses.
//!
//! Numbers become words, a few symbols are spoken, everything else that is not an ASCII letter
//! or punctuation is treated as a word separator. Control characters never reach the synthesizer.

/// How strongly a phrase boundary is marked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boundary {
    /// Comma, semicolon, colon: short pause, pitch holds.
    Minor,
    /// Sentence end `.`: long pause, pitch falls.
    Statement,
    /// `?`: long pause, pitch rises.
    Question,
    /// `!`: long pause, emphatic fall.
    Exclaim,
}

/// One normalised item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Word(String),
    Pause(Boundary),
}

const ONES: [&str; 20] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve",
    "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen",
];
const TENS: [&str; 10] = ["", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];

fn push_below_1000(n: u64, out: &mut Vec<String>) {
    let (h, r) = (n / 100, n % 100);
    if h > 0 {
        out.push(ONES[h as usize].to_string());
        out.push("hundred".to_string());
    }
    if r >= 20 {
        out.push(TENS[(r / 10) as usize].to_string());
        if r % 10 > 0 {
            out.push(ONES[(r % 10) as usize].to_string());
        }
    } else if r > 0 || (h == 0 && n == 0) {
        out.push(ONES[r as usize].to_string());
    }
}

/// Spells a non-negative integer (up to 12 digits) as English words.
pub fn number_words(digits: &str) -> Vec<String> {
    let mut out = Vec::new();
    let digit_by_digit = digits.len() > 12 || (digits.len() > 1 && digits.starts_with('0'));
    if digit_by_digit {
        out.extend(digits.bytes().map(|b| ONES[(b - b'0') as usize].to_string()));
        return out;
    }
    let n: u64 = digits.parse().unwrap_or(0);
    if n == 0 {
        return vec!["zero".to_string()];
    }
    for (scale, name) in [(1_000_000_000, "billion"), (1_000_000, "million"), (1_000, "thousand")] {
        let block = (n / scale) % 1000;
        if block > 0 {
            push_below_1000(block, &mut out);
            out.push(name.to_string());
        }
    }
    if n % 1000 > 0 {
        push_below_1000(n % 1000, &mut out);
    }
    out
}

fn symbol_word(c: char) -> Option<&'static str> {
    Some(match c {
        '&' => "and",
        '%' => "percent",
        '@' => "at",
        '+' => "plus",
        '=' => "equals",
        '#' => "number",
        '/' => "slash",
        '$' => "dollar",
        _ => return None,
    })
}

/// Normalises `text` into items. At most `max_items` items are produced.
pub fn normalize(text: &str, max_items: usize) -> Vec<Item> {
    let chars: Vec<char> = text.chars().collect();
    let mut items: Vec<Item> = Vec::new();
    let mut i = 0;
    while i < chars.len() && items.len() < max_items {
        let c = chars[i];
        if c.is_ascii_alphabetic() {
            let mut w = String::new();
            while i < chars.len() && (chars[i].is_ascii_alphabetic() || (chars[i] == '\'' && i + 1 < chars.len() && chars[i + 1].is_ascii_alphabetic())) {
                if chars[i] != '\'' {
                    w.push(chars[i].to_ascii_lowercase());
                }
                i += 1;
            }
            items.push(Item::Word(w));
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let int: String = chars[start..i].iter().collect();
            items.extend(number_words(&int).into_iter().map(Item::Word));
            if i + 1 < chars.len() && chars[i] == '.' && chars[i + 1].is_ascii_digit() {
                items.push(Item::Word("point".to_string()));
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    items.push(Item::Word(ONES[(chars[i] as u8 - b'0') as usize].to_string()));
                    i += 1;
                }
            }
        } else {
            match c {
                ',' | ';' | ':' => items.push(Item::Pause(Boundary::Minor)),
                '.' => items.push(Item::Pause(Boundary::Statement)),
                '?' => items.push(Item::Pause(Boundary::Question)),
                '!' => items.push(Item::Pause(Boundary::Exclaim)),
                '\n' | '\r' => items.push(Item::Pause(Boundary::Minor)),
                other => {
                    if let Some(w) = symbol_word(other) {
                        items.push(Item::Word(w.to_string()));
                    }
                }
            }
            i += 1;
        }
    }
    items.truncate(max_items);
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(items: &[Item]) -> Vec<String> {
        items
            .iter()
            .map(|i| match i {
                Item::Word(w) => w.clone(),
                Item::Pause(b) => format!("<{b:?}>"),
            })
            .collect()
    }

    #[test]
    fn numbers() {
        assert_eq!(number_words("0"), ["zero"]);
        assert_eq!(number_words("13"), ["thirteen"]);
        assert_eq!(number_words("42"), ["forty", "two"]);
        assert_eq!(number_words("100"), ["one", "hundred"]);
        assert_eq!(number_words("1984"), ["one", "thousand", "nine", "hundred", "eighty", "four"]);
        assert_eq!(number_words("007"), ["zero", "zero", "seven"]);
        assert_eq!(number_words("1000000"), ["one", "million"]);
        assert_eq!(number_words("1234567890123").len(), 13, "too long: digit by digit");
    }

    #[test]
    fn sentence_structure() {
        let w = words(&normalize("Hello, World! It's 3.14?", 100));
        assert_eq!(w, ["hello", "<Minor>", "world", "<Exclaim>", "its", "three", "point", "one", "four", "<Question>"]);
    }

    #[test]
    fn symbols_and_junk() {
        assert_eq!(words(&normalize("a & b", 10)), ["a", "and", "b"]);
        assert_eq!(words(&normalize("\u{1b}[31mred\u{7}\u{0}", 10)), ["thirty", "one", "mred"]);
    }

    #[test]
    fn item_cap() {
        let long = "word ".repeat(1000);
        assert_eq!(normalize(&long, 50).len(), 50);
    }
}
