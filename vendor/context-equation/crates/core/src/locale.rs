//! Writing conventions per language. The tree does not change; only how it is written and read.
//!
//! Every language is a JSON file in `src/locales/`, embedded in the binary: the conventions (decimal
//! and thousands separators, open intervals, how functions are written) and the speech phrases, as
//! templates with `{0}`, `{1}`… A phrase missing from a language falls back to English.

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Locale {
    #[default]
    En,
}

/// The source of each language, in the order of [`Locale::ALL`].
const SOURCES: [(Locale, &str); 1] = [
    (Locale::En, include_str!("locales/en.json")),
];

#[derive(Debug, Deserialize)]
struct Data {
    code: String,
    name: String,
    /// L-NUM-2
    decimal: char,
    /// L-NUM-3; none where thousands are grouped with spaces, which LaTeX math drops
    thousands: Option<char>,
    /// L-INT-1: `]a, b[` is an open interval
    #[serde(default)]
    open_interval: bool,
    /// L-FUN-1: function names written differently from LaTeX's English ones
    #[serde(default)]
    written: HashMap<String, String>,
    speech: HashMap<String, String>,
    /// Phrases a language may have, with no English fallback: without one, the general reading
    /// stays (`dot_product`, L-OPR-1: "dot" instead of "times")
    #[serde(default)]
    optional: HashMap<String, String>,
    /// Fractions read the way they are at school: "três quartos", "three quarters"
    #[serde(default)]
    fractions: Option<Fractions>,
    /// L-UNI-3: the names siunitx units are spoken by
    #[serde(default)]
    units: Option<Units>,
    /// L-LET-2: how double-struck letters are read: the number sets by name, the rest as letters
    #[serde(default)]
    double_struck: Option<DoubleStruck>,
}

/// Unit names, by the siunitx command names: `names.meter` is `["meter", "meters"]`, `prefixes.kilo`
/// is "kilo", and `compounds` holds the prefixed names that are not a plain concatenation
/// (pt-BR "quilômetro", "milissegundo").
#[derive(Debug, Deserialize)]
struct Units {
    per: String,
    /// When the unit after a value is singular: `one` (1 meter, 1.5 meters) or `below_two`
    /// (1,5 metro, 2 metros).
    singular: Singular,
    prefixes: HashMap<String, String>,
    names: HashMap<String, (String, String)>,
    #[serde(default)]
    compounds: HashMap<String, (String, String)>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Singular {
    One,
    BelowTwo,
}

/// `letter` is the template of any double-struck letter ("double-struck {0}"); `sets` names the
/// number sets by their letter; `contractions` joins a preposition with the article a set name
/// starts with (pt-BR "pertence a" + "os reais" → "pertence aos reais").
#[derive(Debug, Deserialize)]
struct DoubleStruck {
    letter: String,
    sets: HashMap<String, String>,
    #[serde(default)]
    contractions: HashMap<String, String>,
}

/// Numerals 0–10 and the denominators 0–10, singular and plural. Index 0 and 1 of the
/// denominators are unused.
#[derive(Debug, Deserialize)]
pub(crate) struct Fractions {
    pub numerals: Vec<String>,
    pub singular: Vec<String>,
    pub plural: Vec<String>,
}

fn table() -> &'static HashMap<Locale, Data> {
    static TABLE: OnceLock<HashMap<Locale, Data>> = OnceLock::new();
    TABLE.get_or_init(|| {
        SOURCES
            .iter()
            .map(|(locale, json)| {
                let file = format!("src/locales/{}.json", locale.code());
                let data: Data = serde_json::from_str(json).expect(&file);
                assert_eq!(data.code, locale.code(), "the file of {locale:?} declares another code");
                (*locale, data)
            })
            .collect()
    })
}

impl Locale {
    /// Every supported language, English first.
    pub const ALL: [Locale; 1] = [Locale::En];

    pub fn code(self) -> &'static str {
        "en"
    }

    fn data(self) -> &'static Data {
        &table()[&self]
    }

    /// The language's own name for itself: "Português (Brasil)", "日本語".
    pub fn name(self) -> &'static str {
        &self.data().name
    }

    /// L-NUM-2: decimal separator in the output.
    pub fn decimal_separator(self) -> char {
        self.data().decimal
    }

    /// L-NUM-3: thousands separator recognized in the input and kept in the drawing.
    pub fn thousands_separator(self) -> Option<char> {
        self.data().thousands
    }

    /// L-NUM-1: is a comma between digits a decimal separator in the input?
    pub fn comma_is_decimal(self) -> bool {
        self.data().decimal == ','
    }

    /// L-INT-1: is `]a, b[` an open interval?
    pub fn open_interval(self) -> bool {
        self.data().open_interval
    }

    /// L-FUN-1: how the function is written, from its canonical name.
    pub fn function_name(self, canonical: &str) -> String {
        self.data().written.get(canonical).cloned().unwrap_or_else(|| canonical.into())
    }

    /// How the function is read aloud, with its link to the argument: "sine of", "seno de". A function
    /// without a spoken name is read as written, without the link: "sinh".
    pub fn speak_function(self, canonical: &str) -> String {
        match self.function_spoken(canonical) {
            Some(name) => self.fill("function_of", &[name]),
            None => self.function_name(canonical),
        }
    }

    /// The spoken name of a function alone: "sine", "seno"; `None` when only its written name exists.
    pub(crate) fn function_spoken(self, canonical: &str) -> Option<&'static str> {
        self.lookup(&format!("fn.{canonical}"))
    }

    pub(crate) fn fractions(self) -> Option<&'static Fractions> {
        self.data().fractions.as_ref()
    }

    /// L-UNI-3: "per" between units, in the languages that name the units.
    pub(crate) fn unit_per(self) -> Option<&'static str> {
        self.data().units.as_ref().map(|u| u.per.as_str())
    }

    /// L-UNI-3: does a unit after this value (canonical, with a point) take the plural?
    pub(crate) fn unit_plural(self, value: &str) -> bool {
        let Some(u) = &self.data().units else { return true };
        let Ok(v) = value.parse::<f64>() else { return true };
        match u.singular {
            Singular::One => v != 1.0,
            Singular::BelowTwo => v.abs() >= 2.0,
        }
    }

    /// L-UNI-3: the spoken name of a unit with its prefix: "kilograms", "quilômetro".
    pub(crate) fn unit_name(self, prefix: Option<&str>, name: &str, plural: bool) -> Option<String> {
        let u = self.data().units.as_ref()?;
        let pick = |pair: &(String, String)| if plural { pair.1.clone() } else { pair.0.clone() };
        match prefix {
            None => u.names.get(name).map(pick),
            Some(p) => u.compounds.get(&format!("{p}{name}")).map(pick).or_else(|| Some(format!("{}{}", u.prefixes.get(p)?, pick(u.names.get(name)?)))),
        }
    }

    /// L-LET-2: a double-struck letter as it is read: "the real numbers", "double-struck P"; `None`
    /// in the languages without the names, which read the letter alone.
    pub(crate) fn double_struck(self, letter: &str) -> Option<String> {
        let d = self.data().double_struck.as_ref()?;
        Some(d.sets.get(letter).cloned().unwrap_or_else(|| d.letter.replace("{0}", letter)))
    }

    /// L-LET-2: the preposition `before` joined with the article that starts `name`, when the
    /// language contracts them: `("pertence a", "os reais")` → `("pertence aos", "reais")`.
    pub(crate) fn contract(self, before: &str, name: &str) -> Option<(String, String)> {
        let d = self.data().double_struck.as_ref()?;
        d.contractions.iter().find_map(|(pair, joined)| {
            let (preposition, article) = pair.split_once(' ')?;
            let rest = name.strip_prefix(article)?.strip_prefix(' ')?;
            let head = before.strip_suffix(preposition).filter(|h| h.ends_with(' '))?;
            Some((format!("{head}{joined}"), rest.to_string()))
        })
    }

    /// A phrase of `optional`, only in this language: `None` keeps the general reading.
    pub(crate) fn own_phrase(self, key: &str) -> Option<&'static str> {
        self.data().optional.get(key).map(String::as_str)
    }

    fn lookup(self, key: &str) -> Option<&'static str> {
        self.data().speech.get(key).or_else(|| Locale::En.data().speech.get(key)).map(String::as_str)
    }

    /// A speech phrase; English when the language lacks it.
    pub(crate) fn phrase(self, key: &str) -> &'static str {
        self.lookup(key).unwrap_or_else(|| panic!("speech phrase '{key}' missing from src/locales/en.json"))
    }

    /// A speech template with `{0}`, `{1}`… replaced by `args`.
    pub(crate) fn fill(self, key: &str, args: &[&str]) -> String {
        let mut s = self.phrase(key).to_string();
        for (i, a) in args.iter().enumerate() {
            s = s.replace(&format!("{{{i}}}"), a);
        }
        s
    }

    /// The phrase of an optional key, for symbols: `None` when no language names it.
    pub(crate) fn optional(self, key: &str) -> Option<&'static str> {
        self.lookup(key)
    }

    /// Phrases English has and this language lacks (it falls back to English for them).
    pub fn missing_phrases(self) -> Vec<&'static str> {
        let own = &self.data().speech;
        let mut keys: Vec<&'static str> = Locale::En.data().speech.keys().map(String::as_str).filter(|k| !own.contains_key(*k)).collect();
        keys.sort_unstable();
        keys
    }
}

impl FromStr for Locale {
    type Err = String;

    /// The language code, with or without a region: `pt-BR`, `pt`, `es-MX`, `zh_CN`, `EN`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lower = s.to_ascii_lowercase().replace('_', "-");
        let language = lower.split('-').next().unwrap_or_default();
        Locale::ALL.into_iter().find(|l| l.code().to_ascii_lowercase() == lower || l.code().split('-').next() == Some(language)).ok_or_else(|| {
            let codes: Vec<&str> = Locale::ALL.iter().map(|l| l.code()).collect();
            format!("unknown locale: {s} (use one of {})", codes.join(", "))
        })
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    #[should_panic(expected = "speech phrase 'no.such.phrase' missing from src/locales/en.json")]
    fn a_phrase_no_language_has_is_a_bug_named_by_its_key() {
        Locale::En.phrase("no.such.phrase");
    }

    #[test]
    fn an_optional_phrase_no_language_has_is_none() {
        assert_eq!(Locale::En.own_phrase("dot_product"), Some("dot"));
        assert_eq!(Locale::En.optional("no.such.phrase"), None);
    }
}
