//! Port of litesearch's `sanskrit.py` folds: a Devanagari, IAST or plain token to one lowercase ASCII key.
use crate::tables::{cons, indep, matra, sign};
use unicode_normalization::UnicodeNormalization;
use unicode_properties::{GeneralCategory, UnicodeGeneralCategory};

const VIRAMA: char = '\u{94d}';
const NUKTA: char = '\u{93c}';

fn vedic(c: char) -> bool { matches!(c, '\u{951}'..='\u{954}' | '\u{1cd0}'..='\u{1cf9}' | '\u{a8e0}'..='\u{a8f0}') }

/// Devanagari block, Vedic extensions and Devanagari extended, as `DEVANAGARI` in sanskrit.py.
pub fn deva(c: char) -> bool { matches!(c, '\u{900}'..='\u{97f}' | '\u{1cd0}'..='\u{1cf9}' | '\u{a8e0}'..='\u{a8ff}') }

fn deva_digit(c: char) -> Option<char> { ('\u{966}'..='\u{96f}').contains(&c).then(|| (b'0' + (c as u32 - 0x966) as u8) as char) }

/// Remove Vedic tone marks and return NFC text.
pub fn strip_vedic(s: &str) -> String { s.nfd().filter(|&c| !vedic(c)).nfc().collect() }

/// Transliterate Devanagari to ASCII, adding the inherent `a` when no vowel sign or virama follows a consonant.
pub fn deva2ascii(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let (mut out, mut i) = (String::with_capacity(s.len()), 0);
    while i < cs.len() {
        let ch = cs[i];
        if let Some(c) = cons(ch) {
            out.push_str(c);
            i += 1;
            while i < cs.len() && cs[i] == NUKTA { i += 1 }
            if i < cs.len() && cs[i] == VIRAMA { i += 1; continue }
            if let Some(m) = cs.get(i).and_then(|&x| matra(x)) { out.push_str(m); i += 1; continue }
            out.push('a');
            continue;
        }
        match indep(ch).or_else(|| sign(ch)).or_else(|| matra(ch)) {
            Some(t) => out.push_str(t),
            None => match deva_digit(ch) {
                Some(d) => out.push(d),
                None if ch == VIRAMA => {}
                None => out.push(ch),
            },
        }
        i += 1;
    }
    out
}

fn latn_fold(s: &str) -> String {
    s.nfd().filter(|c| c.general_category() != GeneralCategory::NonspacingMark).collect::<String>().to_lowercase()
}

/// Fold one token, in Devanagari or Latin script, to its lowercase ASCII index key.
pub fn fold_token(s: &str) -> String {
    if s.is_ascii() { return s.to_ascii_lowercase() }
    let s = strip_vedic(s);
    latn_fold(&if s.chars().any(deva) { deva2ascii(&s) } else { s })
}

fn consonant(c: char) -> bool { "bcdfghjklmnpqstvxz".contains(c) }
fn aspirated(c: char) -> bool { "bcdfgjklmnpqrstvxz".contains(c) }

/// Collapse each run of one repeated char that passes `keep` to a single char.
fn squeeze(s: &str, keep: impl Fn(char) -> bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() { if !(keep(c) && out.ends_with(c)) { out.push(c) } }
    out
}

/// `ri` before a consonant is IAST `ṛ`: Krishna to krsna.
fn vocalic_r(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let (mut out, mut i) = (String::with_capacity(s.len()), 0);
    while i < cs.len() {
        if cs[i] == 'r' && cs.get(i + 1) == Some(&'i') && cs.get(i + 2).is_some_and(|&c| consonant(c)) { out.push('r'); i += 2 }
        else { out.push(cs[i]); i += 1 }
    }
    out
}

/// Drop an `h` after a consonant, judged on the text before any `h` is dropped.
fn deaspirate(s: &str) -> String {
    let mut prev = None;
    s.chars().filter(|&c| { let drop = c == 'h' && prev.is_some_and(aspirated); prev = Some(c); !drop }).collect()
}

const ENDINGS: [&str; 7] = ["ies", "ied", "ing", "ed", "es", "s", "y"];

/// Fold like `fold_token`, then match plain-English spellings, so `Bheeshma` and `भीष्म` both give `bisma`.
pub fn roman_fold(s: &str, doubles: bool, endings: bool, numbers: bool) -> String { roman_from(&fold_token(s), doubles, endings, numbers) }

/// `roman_fold` from a token already folded by `fold_token`.
pub fn roman_from(f: &str, doubles: bool, endings: bool, numbers: bool) -> String {
    let k = deaspirate(&vocalic_r(&f.replace('w', "v"))).replace("ee", "i").replace("oo", "u");
    let mut k = squeeze(&k, |c| "aeiou".contains(c));
    if doubles { k = squeeze(&k, |c| c.is_ascii_lowercase()) }
    if endings {
        let n = k.chars().count();
        if let Some(e) = ENDINGS.iter().find(|e| k.ends_with(*e) && n >= e.len() + 3) { k.truncate(k.len() - e.len()) }
    }
    if numbers && let Some(v) = decimal(&k) { k = v }
    k
}

fn nd(c: u32) -> bool { char::from_u32(c).is_some_and(|c| c.general_category() == GeneralCategory::DecimalNumber) }

/// Python's `str(int(k))` for a string of Unicode decimal digits; None otherwise.
/// Each Nd block is whole runs of 0 to 9, so a digit's value is its distance from the block start, mod 10.
fn decimal(k: &str) -> Option<String> {
    if k.is_empty() || !k.chars().all(|c| nd(c as u32)) { return None }
    let ds: String = k.chars().map(|c| { let (c, mut z) = (c as u32, c as u32); while nd(z - 1) { z -= 1 } (b'0' + ((c - z) % 10) as u8) as char }).collect();
    let t = ds.trim_start_matches('0');
    Some(if t.is_empty() { "0".into() } else { t.into() })
}

/// A folded word's first letter and its later consonants, repeats collapsed: `saundarya` and `soundarya` give `sndr`.
pub fn skeleton(k: &str) -> String {
    let mut cs = k.chars();
    let head: String = cs.next().into_iter().collect();
    let rest: String = cs.filter(|c| !"aeiouy".contains(*c)).collect();
    head + &squeeze(&rest, |c| c != '\n')
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn folds() {
        assert_eq!(fold_token("भीष्म"), "bhisma");
        assert_eq!(roman_fold("Bheeshma", false, false, false), "bisma");
        assert_eq!(roman_fold("भीष्म", false, false, false), "bisma");
        assert_eq!(fold_token("Kṛṣṇa"), "krsna");
        assert_eq!(roman_fold("Krishna", false, false, false), "krsna");
        assert_eq!(skeleton("saundarya"), "sndr");
        assert_eq!(roman_fold("duties", false, true, false), "dut");
        assert_eq!(roman_fold("02", false, false, true), "2");
    }
}
