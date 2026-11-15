//! The reference page (site/reference.html) shows `corpus/reference.json`: every row must convert in
//! strict mode with no warning, and an AsciiMath or MathML row must mean the same as its LaTeX column
//! (the same speech), so the table never shows something contextEquation does not do.

use context_equation_core::{convert, BrailleCode, Input, Options};
use serde_json::Value;

fn reference() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/reference.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn strict(source: &str, input: Input) -> context_equation_core::Output {
    let r = convert(source, &Options { input, strict: true, ..Options::default() }).unwrap_or_else(|e| panic!("{input} {source}: {e}"));
    let tolerant = convert(source, &Options { input, ..Options::default() }).unwrap();
    assert!(tolerant.warnings.is_empty(), "{input} {source}: {:?}", tolerant.warnings);
    r
}

fn rows<'a>(data: &'a Value, group: &str) -> Vec<(&'a str, &'a Value)> {
    data[group].as_array().unwrap().iter().flat_map(|s| s["rows"].as_array().unwrap().iter().map(move |r| (s["id"].as_str().unwrap(), r))).collect()
}

#[test]
fn asciimath_rows_mean_the_same_as_their_latex() {
    let data = reference();
    let rows = rows(&data, "asciimath");
    assert!(rows.len() > 150, "the asciimath.org tables: {}", rows.len());
    for (section, row) in rows {
        let (am, tex) = (row["am"].as_str().unwrap(), row["tex"].as_str().unwrap());
        let a = strict(am, Input::AsciiMath);
        let b = strict(tex, Input::Latex);
        if row.get("same_speech") != Some(&Value::Bool(false)) {
            assert_eq!(a.speech, b.speech, "{section}: {am} vs {tex}");
        }
    }
}

#[test]
fn latex_and_science_rows_convert_cleanly() {
    let data = reference();
    for group in ["latex", "science"] {
        for (section, row) in rows(&data, group) {
            // the chemfig section needs the `chem` feature (on by default, as on the site)
            if section == "structures" && !cfg!(feature = "chem") {
                continue;
            }
            let tex = row["tex"].as_str().unwrap();
            let r = strict(tex, Input::Latex);
            assert!(!r.speech.is_empty(), "{section}: {tex}");
        }
    }
}

#[test]
fn mathml_rows_mean_the_same_as_their_latex() {
    let data = reference();
    for (section, row) in rows(&data, "mathml") {
        let (mml, tex) = (row["mml"].as_str().unwrap(), row["tex"].as_str().unwrap());
        assert_eq!(strict(mml, Input::MathMl).speech, strict(tex, Input::Latex).speech, "{section}: {mml}");
    }
}

#[test]
fn every_row_has_braille_in_both_codes() {
    let data = reference();
    for group in ["asciimath", "latex", "science", "mathml"] {
        for (section, row) in rows(&data, group) {
            let (source, input) = match (row.get("am"), row.get("mml")) {
                (Some(am), _) => (am.as_str().unwrap(), Input::AsciiMath),
                (_, Some(mml)) => (mml.as_str().unwrap(), Input::MathMl),
                _ => (row["tex"].as_str().unwrap(), Input::Latex),
            };
            for code in BrailleCode::ALL {
                let r = convert(source, &Options { input, braille: Some(code), ..Options::default() }).unwrap();
                assert!(!r.braille.is_empty(), "{section} {code}: {source}");
            }
        }
    }
}

#[test]
fn ids_are_unique() {
    let data = reference();
    let mut seen = std::collections::HashSet::new();
    for group in ["asciimath", "latex", "science", "mathml"] {
        for s in data[group].as_array().unwrap() {
            assert!(seen.insert(s["id"].as_str().unwrap().to_string()), "{}", s["id"]);
        }
    }
}
