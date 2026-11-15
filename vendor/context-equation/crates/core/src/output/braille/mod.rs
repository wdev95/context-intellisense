//! Mathematics in braille, as Unicode braille cells (U+2800–U+28FF), from the tree of meaning.
//!
//! Two codes: the Código Matemático Unificado (CMU), used in Portuguese and Spanish, and the Nemeth
//! code, used in English. The language chooses one by default; `Options::braille` forces either.
//! The signs of each code are written as dot numbers ("1456"), the way the official documents list
//! them, so every one can be checked against its source (cited in `spec/braille.md`).

mod cmu;
mod nemeth;

use std::fmt;
use std::str::FromStr;

use crate::locale::Locale;
use crate::tree::Node;

/// A braille code for mathematics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrailleCode {
    /// Código Matemático Unificado (Brazil, Portugal and the Spanish-speaking countries).
    Cmu,
    /// Nemeth Braille Code for Mathematics (United States, Canada).
    Nemeth,
}

impl BrailleCode {
    pub const ALL: [BrailleCode; 2] = [BrailleCode::Cmu, BrailleCode::Nemeth];

    pub fn code(self) -> &'static str {
        match self {
            BrailleCode::Cmu => "cmu",
            BrailleCode::Nemeth => "nemeth",
        }
    }

    /// The code of a language: CMU in Portuguese and Spanish, Nemeth elsewhere.
    pub fn for_locale(locale: Locale) -> BrailleCode {
        match locale {
            Locale::PtBr | Locale::Es => BrailleCode::Cmu,
            _ => BrailleCode::Nemeth,
        }
    }
}

impl fmt::Display for BrailleCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl FromStr for BrailleCode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "cmu" => Ok(BrailleCode::Cmu),
            "nemeth" => Ok(BrailleCode::Nemeth),
            other => Err(format!("unknown braille code: {other} (use cmu or nemeth)")),
        }
    }
}

/// The equation in braille, and the symbols the code has no sign for (written as the full cell ⠿).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Braille {
    pub text: String,
    pub missing: Vec<char>,
}

pub fn transcribe(node: &Node, code: BrailleCode) -> Braille {
    match code {
        BrailleCode::Cmu => cmu::transcribe(node),
        BrailleCode::Nemeth => nemeth::transcribe(node),
    }
}

/// One cell from its dot numbers: "1456" → ⠹. "0" is the blank cell.
pub(crate) fn cell(dots: &str) -> char {
    let bits = dots.chars().filter_map(|d| d.to_digit(10)).filter(|d| (1..=8).contains(d)).fold(0u32, |acc, d| acc | 1 << (d - 1));
    char::from_u32(0x2800 + bits).expect("a braille pattern")
}

/// Several cells, separated by spaces: "46 13" → ⠨⠅.
pub(crate) fn cells(dots: &str) -> String {
    dots.split_whitespace().map(cell).collect()
}

/// The letters a–z by their cells, the base of every braille alphabet.
pub(crate) fn letter(c: char) -> Option<&'static str> {
    Some(match c.to_ascii_lowercase() {
        'a' => "1",
        'b' => "12",
        'c' => "14",
        'd' => "145",
        'e' => "15",
        'f' => "124",
        'g' => "1245",
        'h' => "125",
        'i' => "24",
        'j' => "245",
        'k' => "13",
        'l' => "123",
        'm' => "134",
        'n' => "1345",
        'o' => "135",
        'p' => "1234",
        'q' => "12345",
        'r' => "1235",
        's' => "234",
        't' => "2345",
        'u' => "136",
        'v' => "1236",
        'w' => "2456",
        'x' => "1346",
        'y' => "13456",
        'z' => "1356",
        _ => return None,
    })
}
