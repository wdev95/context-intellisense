//! unicode-math's alphabets and `\sym…` commands, and bbm's (#173): the letters of Unicode's
//! Mathematical Alphanumeric Symbols, drawn by the font, written to MathML and read back.

use context_equation_core::layout::alphabet::character;
use context_equation_core::tree::{Node, Variant};
use context_equation_core::{convert, font, Input, Options};

/// (command, alphabet), after the unicode-math manual's table of math style commands.
const COMMANDS: &[(&str, Variant)] = &[
    ("mathbfsf", Variant::BoldSansSerif),
    ("mathsfit", Variant::SansSerifItalic),
    ("mathbfsfit", Variant::SansSerifBoldItalic),
    ("mathbfcal", Variant::BoldScript),
    ("mathbffrak", Variant::BoldFraktur),
    ("mathbfscr", Variant::BoldScript),
    ("mathbfup", Variant::Bold),
    ("mathsfup", Variant::SansSerif),
    ("mathbfsfup", Variant::BoldSansSerif),
    ("symup", Variant::Upright),
    ("symit", Variant::Italic),
    ("symcal", Variant::Calligraphic),
    ("symscr", Variant::Calligraphic),
    ("symfrak", Variant::Fraktur),
    ("symsf", Variant::SansSerif),
    ("symtt", Variant::Monospace),
    ("symbb", Variant::DoubleStruck),
    ("symnormal", Variant::Italic),
    // bbm: no Unicode letters of their own, drawn as the double-struck ones
    ("mathbbmss", Variant::DoubleStruck),
    ("mathbbmtt", Variant::DoubleStruck),
];

const NEW: [Variant; 5] = [Variant::BoldScript, Variant::BoldFraktur, Variant::BoldSansSerif, Variant::SansSerifItalic, Variant::SansSerifBoldItalic];

fn strict() -> Options {
    Options { strict: true, ..Options::default() }
}

fn only(node: &Node) -> &Node {
    match node {
        Node::Row { children } if children.len() == 1 => only(&children[0]),
        other => other,
    }
}

#[test]
fn every_command_sets_its_argument_in_its_alphabet() {
    for &(name, variant) in COMMANDS {
        let tex = format!("\\{name}{{L}}");
        let out = convert(&tex, &strict()).unwrap_or_else(|e| panic!("{tex}: {e}"));
        match only(&out.tree) {
            Node::Identifier { name: letter, variant: v } => assert_eq!((letter.as_str(), *v), ("L", variant), "{tex}"),
            other => panic!("{tex}: {other:?}"),
        }
        let tolerant = convert(&tex, &Options::default()).unwrap();
        assert!(tolerant.warnings.is_empty(), "{tex}: {:?}", tolerant.warnings);
    }
}

/// The letters, checked against unicode-math-table.tex (\mbfscrL, \mbffrakg, \mbfsansA, \mitsansx,
/// \mbfitsansy, \mbfsansalpha, \mbfitsansOmega, \mbfsansz, \mbfscrz, \mbffrakZ, \mbfitsansa).
#[test]
fn the_letters_are_unicode_maths() {
    for (variant, plain, expected) in [
        (Variant::BoldScript, 'L', '\u{1D4DB}'),
        (Variant::BoldScript, 'z', '\u{1D503}'),
        (Variant::BoldFraktur, 'g', '\u{1D58C}'),
        (Variant::BoldFraktur, 'Z', '\u{1D585}'),
        (Variant::BoldSansSerif, 'A', '\u{1D5D4}'),
        (Variant::BoldSansSerif, 'z', '\u{1D607}'),
        (Variant::BoldSansSerif, 'α', '\u{1D770}'),
        (Variant::SansSerifItalic, 'x', '\u{1D639}'),
        (Variant::SansSerifBoldItalic, 'y', '\u{1D66E}'),
        (Variant::SansSerifBoldItalic, 'a', '\u{1D656}'),
        (Variant::SansSerifBoldItalic, 'Ω', '\u{1D7A8}'),
    ] {
        assert_eq!(character(plain, variant), expected, "{variant:?} {plain}");
    }
    assert_eq!(convert(r"\mathbfcal{L} \mathbffrak{g} \mathsfit{x} \mathbfsfit{y} \mathbfsf{A}", &strict()).unwrap().text, "𝓛𝖌𝘹𝙮𝗔");
}

#[test]
fn the_font_draws_every_letter_of_the_new_alphabets() {
    let greek = ('Α'..='Ω').filter(|c| *c != '\u{3A2}').chain('α'..='ω');
    for variant in NEW {
        for c in ('A'..='Z').chain('a'..='z').chain('0'..='9').chain(greek.clone()) {
            let mapped = character(c, variant);
            if mapped != c {
                assert!(font::font().glyph(mapped).is_some(), "{variant:?} {c}: STIX Two Math has no {mapped}");
            }
        }
    }
}

#[test]
fn every_alphabet_round_trips_through_mathml() {
    for &(name, _) in COMMANDS {
        for letters in ["Lg", r"\alpha", r"\Omega", "x"] {
            let tex = format!("\\{name}{{{letters}}}");
            let first = convert(&tex, &strict()).unwrap();
            let back = convert(&first.mathml, &Options { input: Input::MathMl, ..strict() }).unwrap_or_else(|e| panic!("{tex}: {e}\n{}", first.mathml));
            assert_eq!(back.tree, first.tree, "{tex}: {}", first.mathml);
        }
    }
    // the letters Unicode has are written as themselves, the rest with their mathvariant
    let mml = convert(r"\mathbfcal{L} \mathsfit{\alpha}", &strict()).unwrap().mathml;
    assert!(mml.contains("<mi>𝓛</mi>"), "{mml}");
    assert!(mml.contains(r#"<mi mathvariant="sans-serif-italic">α</mi>"#), "{mml}");
}

#[test]
fn the_issue_examples_read_without_warnings() {
    for tex in [r"\mathbfcal{L} = \symcal{L}", r"\mathsfit{x} + \mathbfsfit{y}", r"\symup{d}x"] {
        let out = convert(tex, &Options::default()).unwrap();
        assert!(out.warnings.is_empty(), "{tex}: {:?}", out.warnings);
    }
    assert_eq!(convert(r"\mathbfcal{L}", &Options::default()).unwrap().text, "𝓛");
}
