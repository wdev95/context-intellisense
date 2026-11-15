//! The fonts as values (names, debugging, hashing), a math font standing in as a fallback twice, and
//! fonts missing the tables contextEquation reads: outlines in a fallback, the x-height in a math font.

use std::collections::HashSet;

use context_equation_core::font::{FallbackFont, FontError, MathFont};
use context_equation_core::{convert, Options};

fn bytes(path: &str) -> Vec<u8> {
    std::fs::read(format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// The font's bytes with the tag of one table renamed: the font without that table.
fn without_table(mut bytes: Vec<u8>, tag: &[u8; 4]) -> Vec<u8> {
    let at = bytes.windows(4).position(|w| w == tag).expect("a table directory entry");
    bytes[at] = b'X';
    bytes
}

#[test]
fn a_font_is_named_and_debugged_by_its_family_name() {
    let f = MathFont::StixTwo.get();
    assert_eq!(f.name(), "STIX Two Math");
    assert_eq!(format!("{f:?}"), r#"Font { name: "STIX Two Math" }"#);
}

#[test]
fn a_font_hashes_as_itself() {
    let mut set = HashSet::new();
    set.insert(MathFont::StixTwo.get());
    set.insert(MathFont::StixTwo.get());
    assert_eq!(set.len(), 1);
}

#[cfg(feature = "emoji")]
#[test]
fn a_fallback_font_is_debugged_by_its_name() {
    assert_eq!(format!("{:?}", FallbackFont::noto_emoji()), r#"FallbackFont("Noto Emoji")"#);
}

#[test]
fn a_math_font_stands_in_as_the_same_fallback_every_time() {
    let a: FallbackFont = MathFont::StixTwo.into();
    let b: FallbackFont = MathFont::StixTwo.into();
    assert_eq!(a, b);
    assert_eq!(a.name(), "STIX Two Math");
}

#[test]
fn a_fallback_font_without_outlines_is_refused() {
    let e = FallbackFont::load(without_table(bytes("tests/fonts/fallback-a.ttf"), b"glyf")).unwrap_err();
    assert!(matches!(e, FontError::NotAFont(_)), "{e:?}");
    assert_eq!(e.to_string(), "not a font: fallback-a has no outlines (glyf, CFF or CFF2)");
}

#[test]
fn a_math_font_without_an_x_height_takes_its_accent_base_height() {
    // STIX Two Math without its OS/2 table: no sxHeight to read
    let font = MathFont::load(without_table(bytes("fonts/STIXTwoMath-Regular-subset.otf"), b"OS/2")).unwrap();
    let f = font.get();
    let accent_base = f.c(|c| c.accent_base_height());
    assert!(accent_base > 0.0);
    assert_eq!(f.x_height(), accent_base);
    // the embedded font has its own, which differs
    assert_ne!(MathFont::StixTwo.get().x_height(), f.x_height());
    // and the equation is still set
    assert!(convert(r"\hat{x}", &Options { font, ..Options::default() }).unwrap().svg.contains("<use "));
}

/// The `ssty` coverage of tests/fonts/ssty.ttf (tools/fonts/test_fonts.py): format 1, two glyphs, A (1)
/// and B (3).
const SSTY_COVERAGE: [u8; 8] = [0, 1, 0, 2, 0, 1, 0, 3];

#[test]
fn script_forms_come_from_a_coverage_that_lists_its_glyphs() {
    // the embedded fonts give their ssty coverage as ranges (format 2); a font may list glyphs instead
    let font = FallbackFont::load(bytes("tests/fonts/ssty.ttf")).unwrap().get();
    let (a, b) = (font.glyph('A').unwrap(), font.glyph('B').unwrap());
    assert_eq!((a.0, b.0), (1, 3));
    assert_eq!(font.script_form(a, 1).0, 2);
    assert_eq!(font.script_form(b, 1).0, 4);
    // past the last form, the last one
    assert_eq!(font.script_form(b, 2).0, 4);
}

#[test]
fn script_forms_past_the_end_of_their_coverage_are_left_out() {
    // the coverage cut to one glyph while two alternate sets follow: the second set covers nothing
    let mut ssty = bytes("tests/fonts/ssty.ttf");
    assert_eq!(ssty.windows(8).filter(|w| *w == SSTY_COVERAGE).count(), 1);
    let at = ssty.windows(8).position(|w| w == SSTY_COVERAGE).expect("the ssty coverage");
    ssty[at + 3] = 1;
    let font = FallbackFont::load(ssty).unwrap().get();
    let (a, b) = (font.glyph('A').unwrap(), font.glyph('B').unwrap());
    assert_eq!(font.script_form(a, 1).0, 2);
    assert_eq!(font.script_form(b, 1), b);
}
