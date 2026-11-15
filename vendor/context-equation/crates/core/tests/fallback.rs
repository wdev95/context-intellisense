//! Fallback fonts: a character the math font lacks comes from the first fallback font that has it
//! (`Options::fallback_fonts`), with its outline in the SVG as every other glyph's (S-SVG-5). The
//! two tiny fonts in tests/fonts are written by tools/fonts/test_fonts.py: fallback-a.ttf (glyf,
//! 1000 units per em) has a square 中 and a diamond 😀; fallback-b.otf (CFF, 2048) a triangle 😀.

use context_equation_core::font::{FallbackFont, MathFont};
use context_equation_core::layout::Item;
use context_equation_core::{rules, Error, Options};

fn load(name: &str) -> FallbackFont {
    let bytes = std::fs::read(format!("{}/tests/fonts/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    FallbackFont::load(bytes).unwrap()
}

fn with(fallbacks: &[FallbackFont]) -> Options {
    Options { fallback_fonts: fallbacks.to_vec(), ..Options::default() }
}

/// The `<path>`s of the SVG's `<defs>` (S-SVG-5).
fn defs(svg: &str) -> Vec<&str> {
    let defs = svg.split("<defs>").nth(1).unwrap().split("</defs>").next().unwrap();
    defs.split("<path ").skip(1).collect()
}

#[test]
fn a_character_no_font_has_is_still_a_box_or_refused() {
    let none = with(&[]);
    for tex in ["E = mc^2 😀", r"\text{中}"] {
        let r = context_equation_core::convert(tex, &none).unwrap();
        assert!(r.warnings.iter().any(|w| w.rule == rules::E_REC_8), "{tex}: {:?}", r.warnings);
        assert!(matches!(context_equation_core::convert(tex, &Options { strict: true, ..none.clone() }), Err(Error::MissingGlyph { .. })), "{tex}");
    }
    // a fallback without the character changes nothing
    let b = with(&[load("fallback-b.otf")]);
    assert!(context_equation_core::convert(r"\text{中}", &b).unwrap().warnings.iter().any(|w| w.rule == rules::E_REC_8));
    assert!(matches!(context_equation_core::convert(r"\text{中}", &Options { strict: true, ..b }), Err(Error::MissingGlyph { character: '中' })));
}

#[test]
fn cjk_text_from_a_fallback_font() {
    let options = with(&[load("fallback-a.ttf")]);
    for strict in [false, true] {
        let r = context_equation_core::convert(r"\text{nota: 中}", &Options { strict, ..options.clone() }).unwrap();
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
        // the square, at the math font's em (STIX Two Math has 1000 units per em, as fallback-a)
        assert!(defs(&r.svg).iter().any(|p| p.contains(r#"d="M100 -100 L900 -100 L900 700 L100 700 L100 -100 Z""#)), "{}", r.svg);
        // speech and text keep the character
        assert!(r.speech.contains('中') && r.text.contains('中'));
    }
    // inside a name of several letters, letter by letter
    let r = context_equation_core::convert("<math><mi>a中</mi></math>", &options).unwrap();
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    assert_eq!(r.svg.matches("<use ").count(), 2);
}

#[test]
fn fallbacks_are_tried_in_order() {
    let (a, b) = (load("fallback-a.ttf"), load("fallback-b.otf"));
    let diamond = r#"d="M500 -100 L900 300 L500 700 L100 300 L500 -100 Z""#;
    let ab = context_equation_core::convert("😀", &with(&[a, b])).unwrap().svg;
    let ba = context_equation_core::convert("😀", &with(&[b, a])).unwrap().svg;
    assert!(defs(&ab).iter().any(|p| p.contains(diamond)), "{ab}");
    // fallback-b's triangle, CFF outlines
    assert!(defs(&ba).iter().all(|p| !p.contains(diamond)) && defs(&ba).iter().any(|p| p.contains(r#"d="M200 0 L1848 0 L1024 1600 Z""#)), "{ba}");
    // and only the character the math font lacks: x is STIX Two Math's in both
    assert_eq!(context_equation_core::convert("x😀", &with(&[a, b])).unwrap().svg.matches(r##"href="#context-equation-"##).count(), 2);
}

#[test]
fn a_fallback_glyph_is_scaled_to_the_em_and_sits_on_the_baseline() {
    let b = load("fallback-b.otf");
    let (math_box, warnings) = context_equation_core::typeset("😀", &with(&[b])).unwrap();
    assert!(warnings.is_empty());
    let em = MathFont::default().get().units_per_em();
    // 2048 units wide in a 2048-unit em: one em of the math font; the triangle stands on the baseline
    assert!((math_box.width - em).abs() < 0.01, "{}", math_box.width);
    assert!((math_box.height - 1600.0 * em / 2048.0).abs() < 0.01 && math_box.depth == 0.0, "{math_box:?}");
    let Item::Glyph { font, scale, .. } = &math_box.items[0] else { panic!("{math_box:?}") };
    assert_eq!(*font, Some(b));
    assert!((scale - em / 2048.0).abs() < 1e-6);
    // in a script, at the script's size
    let (sup, _) = context_equation_core::typeset("x^{😀}", &with(&[b])).unwrap();
    assert!(sup.width < math_box.width + 600.0);
}

#[test]
fn a_fallback_glyph_is_an_ordinary_symbol_in_one_color() {
    let a = load("fallback-a.ttf");
    let r = context_equation_core::convert("😀", &with(&[a])).unwrap();
    assert!(r.tree.to_json().contains(r#""class":"ord""#), "{}", r.tree.to_json());
    // drawn in the group's currentColor, as every glyph; \color applies to it too
    assert!(r.svg.contains(r#"<g fill="currentColor""#));
    let red = context_equation_core::convert(r"\color{red}{😀}", &with(&[a])).unwrap().svg;
    assert!(red.contains(r#"fill="red""#), "{red}");
}

#[test]
fn ids_set_fallback_glyphs_apart() {
    let a = load("fallback-a.ttf");
    let svg = context_equation_core::convert("😀 + 😀 = 2 \\cdot 😀", &with(&[a])).unwrap().svg;
    let fallback: Vec<&str> = defs(&svg).into_iter().filter(|p| p.contains("-f")).collect();
    // S-SVG-5: one path for the three uses
    assert_eq!(fallback.len(), 1, "{svg}");
    let id = fallback[0].split('"').nth(1).unwrap();
    assert_eq!(svg.matches(&format!(r##"href="#{id}""##)).count(), 3, "{svg}");
    // D-DET-1: the same equation, the same SVG
    assert_eq!(svg, context_equation_core::convert("😀 + 😀 = 2 \\cdot 😀", &with(&[a])).unwrap().svg);
}

#[test]
fn loading_fallback_fonts() {
    assert!(FallbackFont::load(b"not a font".to_vec()).is_err());
    let a = load("fallback-a.ttf");
    assert_eq!(a.name(), "fallback-a");
    // the same bytes, the same font
    assert_eq!(a, load("fallback-a.ttf"));
    assert!(FallbackFont::loaded().contains(&a));
    // a math font can stand in for another
    let stix: FallbackFont = MathFont::default().into();
    assert_eq!(stix.name(), "STIX Two Math");
}

#[test]
fn the_pen_draws_fallback_glyphs() {
    let a = load("fallback-a.ttf");
    let svg = context_equation_core::pen(r"\text{中}", &with(&[a]), &Default::default()).unwrap();
    assert!(svg.contains("M100 -100 L900 -100 L900 700 L100 700 L100 -100 Z"), "{svg}");
}

/// The issue's examples (#125) with the embedded Noto Emoji.
#[cfg(feature = "emoji")]
#[test]
fn emoji_from_noto_emoji() {
    assert_eq!(Options::default().fallback_fonts, vec![FallbackFont::noto_emoji()]);
    for tex in ["E = mc^2 😀", "🍎 + 🍎 = 2 \\cdot 🍎", r"\text{nota: 10 ⭐}", "😀 + ∑ ∫ α → ℝ"] {
        let r = context_equation_core::convert(tex, &Options::default()).unwrap();
        assert!(r.warnings.is_empty(), "{tex}: {:?}", r.warnings);
        assert!(context_equation_core::convert(tex, &Options { strict: true, ..Options::default() }).is_ok(), "{tex}");
    }
    let r = context_equation_core::convert("🍎 + 🍎 = 2 \\cdot 🍎", &Options::default()).unwrap();
    assert_eq!(defs(&r.svg).iter().filter(|p| p.contains("-f")).count(), 1);
    assert!(r.svg.contains("<contextEquation:font>STIX Two Math</contextEquation:font>"));
    // speech, text and braille keep the character, as for any character without a name
    assert_eq!(r.speech, "🍎 plus 🍎 equals 2 times 🍎");
    assert_eq!(r.text, "🍎 + 🍎 = 2 · 🍎");
}
