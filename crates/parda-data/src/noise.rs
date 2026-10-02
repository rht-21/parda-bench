//! Realistic damage to the text around entities: OCR confusions, lost or doubled spaces, dropped punctuation.
//! Entity values are never touched, and each text keeps its first and last character so the separators next
//! to an entity survive.

use rand::RngExt;

use crate::DataRng;

const OCR_SWAP: f64 = 0.05;
const DROP_SPACE: f64 = 0.08;
const DOUBLE_SPACE: f64 = 0.04;
const DROP_PUNCTUATION: f64 = 0.2;

/// Characters a scanner or a hurried typist confuses.
fn ocr_twin(c: char) -> Option<char> {
    match c {
        'O' | 'o' => Some('0'),
        'l' | 'I' => Some('1'),
        'S' => Some('5'),
        'B' => Some('8'),
        'e' => Some('c'),
        _ => None,
    }
}

#[must_use]
pub fn add_noise(text: &str, rng: &mut DataRng) -> String {
    let chars: Vec<char> = text.chars().collect();
    let last = chars.len().saturating_sub(1);
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        if i == 0 || i == last {
            out.push(c);
            continue;
        }
        match c {
            ' ' if rng.random_bool(DROP_SPACE) => {}
            ' ' if rng.random_bool(DOUBLE_SPACE) => out.push_str("  "),
            '.' | ',' | ';' | ':' if rng.random_bool(DROP_PUNCTUATION) => {}
            _ => match ocr_twin(c) {
                Some(twin) if rng.random_bool(OCR_SWAP) => out.push(twin),
                _ => out.push(c),
            },
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;

    const TEXT: &str = " Please, Sir: send the Original Bill to my email ID before Monday. Also note the old address. ";

    #[test]
    fn keeps_first_and_last_character() {
        let mut rng = DataRng::seed_from_u64(1);
        for _ in 0..200 {
            let noisy = add_noise(TEXT, &mut rng);
            assert!(noisy.starts_with(' ') && noisy.ends_with(' '), "{noisy:?}");
        }
    }

    #[test]
    fn damages_long_text_and_is_deterministic() {
        let a = add_noise(TEXT, &mut DataRng::seed_from_u64(2));
        let b = add_noise(TEXT, &mut DataRng::seed_from_u64(2));
        assert_eq!(a, b);
        assert_ne!(a, TEXT);
    }

    #[test]
    fn leaves_devanagari_letters_alone() {
        let text = "मेरा नाम है और पता लिखा है";
        let noisy = add_noise(text, &mut DataRng::seed_from_u64(3));
        let letters = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        assert_eq!(letters(&noisy), letters(text));
    }

    #[test]
    fn short_texts_pass_through() {
        let mut rng = DataRng::seed_from_u64(4);
        assert_eq!(add_noise("", &mut rng), "");
        assert_eq!(add_noise(": ", &mut rng), ": ");
    }
}
