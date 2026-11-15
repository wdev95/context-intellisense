//! Letters with a variant (italic, bold, double-struck…) through Unicode's math blocks (U+1D400).

use crate::tree::Variant;

/// L-LET-1: Latin and lowercase Greek letters in math italic (block U+1D400).
pub fn math_italic(c: char) -> char {
    character(c, Variant::Italic)
}

pub fn character(c: char, variant: Variant) -> char {
    use Variant::*;
    // historical exceptions: letters that already existed in the Letterlike Symbols block
    let exception = match (variant, c) {
        (Italic, 'h') => Some('ℎ'),
        (Italic, 'ı') => Some('𝚤'),
        (Italic, 'ȷ') => Some('𝚥'),
        (Calligraphic, 'B') => Some('ℬ'),
        (Calligraphic, 'E') => Some('ℰ'),
        (Calligraphic, 'F') => Some('ℱ'),
        (Calligraphic, 'H') => Some('ℋ'),
        (Calligraphic, 'I') => Some('ℐ'),
        (Calligraphic, 'L') => Some('ℒ'),
        (Calligraphic, 'M') => Some('ℳ'),
        (Calligraphic, 'R') => Some('ℛ'),
        (Calligraphic, 'e') => Some('ℯ'),
        (Calligraphic, 'g') => Some('ℊ'),
        (Calligraphic, 'o') => Some('ℴ'),
        (Fraktur, 'C') => Some('ℭ'),
        (Fraktur, 'H') => Some('ℌ'),
        (Fraktur, 'I') => Some('ℑ'),
        (Fraktur, 'R') => Some('ℜ'),
        (Fraktur, 'Z') => Some('ℨ'),
        (DoubleStruck, 'C') => Some('ℂ'),
        (DoubleStruck, 'H') => Some('ℍ'),
        (DoubleStruck, 'N') => Some('ℕ'),
        (DoubleStruck, 'P') => Some('ℙ'),
        (DoubleStruck, 'Q') => Some('ℚ'),
        (DoubleStruck, 'R') => Some('ℝ'),
        (DoubleStruck, 'Z') => Some('ℤ'),
        _ => None,
    };
    if let Some(e) = exception {
        return e;
    }
    let (uppercase, lowercase, digit, greek_upper, greek_lower) = match variant {
        Upright => return c,
        Italic => (0x1D434, 0x1D44E, None, Some(0x1D6E2), Some(0x1D6FC)),
        Bold => (0x1D400, 0x1D41A, Some(0x1D7CE), Some(0x1D6A8), Some(0x1D6C2)),
        BoldItalic => (0x1D468, 0x1D482, Some(0x1D7CE), Some(0x1D71C), Some(0x1D736)),
        Calligraphic => (0x1D49C, 0x1D4B6, None, None, None),
        Fraktur => (0x1D504, 0x1D51E, None, None, None),
        DoubleStruck => (0x1D538, 0x1D552, Some(0x1D7D8), None, None),
        SansSerif => (0x1D5A0, 0x1D5BA, Some(0x1D7E2), None, None),
        Monospace => (0x1D670, 0x1D68A, Some(0x1D7F6), None, None),
        // #173 (unicode-math-table.tex: \mbfscrA, \mbffrakA, \mbfsansA, \mitsansA, \mbfitsansA and
        // their Greek and digits); the italic ones take the digits of their upright alphabet
        BoldScript => (0x1D4D0, 0x1D4EA, None, None, None),
        BoldFraktur => (0x1D56C, 0x1D586, None, None, None),
        BoldSansSerif => (0x1D5D4, 0x1D5EE, Some(0x1D7EC), Some(0x1D756), Some(0x1D770)),
        SansSerifItalic => (0x1D608, 0x1D622, Some(0x1D7E2), None, None),
        SansSerifBoldItalic => (0x1D63C, 0x1D656, Some(0x1D7EC), Some(0x1D790), Some(0x1D7AA)),
    };
    let code = match c {
        'A'..='Z' => Some(uppercase + (c as u32 - 'A' as u32)),
        'a'..='z' => Some(lowercase + (c as u32 - 'a' as u32)),
        '0'..='9' => digit.map(|d| d + (c as u32 - '0' as u32)),
        'Α'..='Ω' => greek_upper.map(|g| g + (c as u32 - 'Α' as u32)),
        'α'..='ω' => greek_lower.map(|g| g + (c as u32 - 'α' as u32)),
        // the variant letters come after ω and ∂ in each block: ϵ ϑ ϰ ϕ ϱ ϖ (\epsilon, \vartheta,
        // \varkappa, \phi, \varrho, \varpi), set in italic by TeX like the other lowercase Greek letters
        'ϵ' | 'ϑ' | 'ϰ' | 'ϕ' | 'ϱ' | 'ϖ' => {
            let from_alpha = match c {
                'ϵ' => 26,
                'ϑ' => 27,
                'ϰ' => 28,
                'ϕ' => 29,
                'ϱ' => 30,
                _ => 31,
            };
            greek_lower.map(|g| g + from_alpha)
        }
        _ => None,
    };
    code.and_then(char::from_u32).unwrap_or(c)
}
