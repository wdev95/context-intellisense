//! The fonts an equation can be set in.

use context_equation_core::font::MathFont;
use context_equation_core::{convert, convert_in_font, Options};

#[test]
fn stix_two_is_the_default() {
    assert_eq!(MathFont::default(), MathFont::StixTwo);
    assert_eq!(MathFont::StixTwo.name(), "STIX Two Math");
    let a = convert("x^2", &Options::default()).unwrap().svg;
    let b = convert_in_font("x^2", &Options::default(), MathFont::StixTwo).unwrap().svg;
    assert_eq!(a, b);
    assert!(a.contains("<contextEquation:font>STIX Two Math</contextEquation:font>"));
}

#[test]
fn scripts_are_drawn_in_the_fonts_script_forms() {
    // the `ssty` forms, as TeX draws a script in cmr7 and a script of a script in cmr5
    let f = MathFont::StixTwo.get();
    let one = f.glyph('1').unwrap();
    let (st, sts) = (f.script_form(one, 1), f.script_form(one, 2));
    assert!(st != one && sts != one && st != sts && f.script_form(one, 0) == one);
    let drawn = |tex: &str, id| context_equation_core::typeset(tex, &Options::default()).unwrap().0.items.iter().any(|i| matches!(i, context_equation_core::layout::Item::Glyph { id: g, .. } if *g == id));
    assert!(drawn("x_1", st) && !drawn("x_1", one));
    assert!(drawn("x_{y_1}", sts));
}

#[cfg(feature = "latin-modern")]
mod latin_modern {
    use super::*;
    use context_equation_core::{typeset, typeset_in_font};

    #[test]
    fn has_its_own_metrics() {
        let options = Options { display: false, ..Default::default() };
        let (stix, _) = typeset(r"\text{Hello} \sum_{i=1}^n \sqrt{x_i}", &options).unwrap();
        let (lm, _) = typeset_in_font(r"\text{Hello} \sum_{i=1}^n \sqrt{x_i}", &options, MathFont::LatinModern).unwrap();
        assert!((stix.width - lm.width).abs() > 1.0, "{} {}", stix.width, lm.width);
        assert!(lm.width > 0.0 && lm.height > 0.0);
    }

    #[test]
    fn words_are_a_third_of_an_em_apart() {
        assert_eq!(MathFont::StixTwo.word_space(), 0.25);
        assert!((MathFont::LatinModern.word_space() - 0.332).abs() < 1e-6);
        let options = Options { display: false, ..Default::default() };
        let w = |t: &str| typeset_in_font(t, &options, MathFont::LatinModern).unwrap().0.width;
        assert!((w(r"\text{a b}") - w(r"\text{ab}") - 332.0).abs() < 1e-3);
    }

    #[test]
    fn draws_its_own_glyphs() {
        let svg = convert_in_font(r"\left( \frac{a}{b} \right)", &Options::default(), MathFont::LatinModern).unwrap().svg;
        assert!(svg.contains("<contextEquation:font>Latin Modern Math</contextEquation:font>"));
        let stix = convert(r"\left( \frac{a}{b} \right)", &Options::default()).unwrap().svg;
        assert_ne!(svg, stix);
        assert_eq!(MathFont::LatinModern.get().units_per_em(), 1000.0);
    }

    #[test]
    fn scripts_go_where_pdftex_puts_them() {
        // cmsy10's sub1 (0.15 em) for a subscript alone, sub2 (its MATH table's 0.247 em) under a
        // superscript; sup1 (0.413 em) for a superscript in display
        let depth = |t: &str| typeset_in_font(t, &Options { display: false, ..Default::default() }, MathFont::LatinModern).unwrap().0.depth;
        assert!(depth("x_i") < 170.0 + depth("x") && depth("x_i^2") > 240.0, "{} {}", depth("x_i"), depth("x_i^2"));
        let height = |t: &str, display| typeset_in_font(t, &Options { display, ..Default::default() }, MathFont::LatinModern).unwrap().0.height;
        assert!((height("x^2", true) - height("x^2", false) - 50.0).abs() < 1.0, "{} {}", height("x^2", true), height("x^2", false));
    }

    #[test]
    fn limits_have_room_above_and_below_as_in_tex() {
        // big_op_spacing5: 0.1 em between the box's edge and the ink of an upper or lower limit
        let f = MathFont::LatinModern.get();
        let b = typeset_in_font(r"\sum_{i=0}^N x", &Options::default(), MathFont::LatinModern).unwrap().0;
        let (mut top, mut bottom) = (f32::MIN, f32::MAX);
        for item in &b.items {
            if let context_equation_core::layout::Item::Glyph { id, y, scale, .. } = item {
                let (_, y0, _, y1) = f.limits(*id);
                (top, bottom) = (top.max(y + y1 * scale), bottom.min(y + y0 * scale));
            }
        }
        assert!((b.height - top - 100.0).abs() < 1.0 && (b.depth + bottom - 100.0).abs() < 1.0, "{} {top} {} {bottom}", b.height, b.depth);
    }

    #[test]
    fn moustaches_are_built_from_brace_halves() {
        // Latin Modern Math has no ⎰ ⎱: they are made of the braces' halves, as cmex makes them,
        // at least the two halves tall and growing with what they enclose
        let options = Options { display: false, ..Default::default() };
        for d in [r"\lmoustache", r"\rmoustache"] {
            let (b, w) = typeset_in_font(&format!(r"\left{d} x \right."), &options, MathFont::LatinModern).unwrap();
            assert!(w.is_empty() && b.height + b.depth >= 1480.0, "{d}: {w:?} {}", b.height + b.depth);
            let (tall, _) = typeset_in_font(&format!(r"\left{d} \rule{{0pt}}{{5em}} \right."), &options, MathFont::LatinModern).unwrap();
            assert!(tall.height + tall.depth > 4500.0, "{d}");
        }
    }
}

/// The embedded fonts are subsets of the originals in tools/fonts (tools/fonts/subset.sh): every
/// character keeps its glyph ID, outline and metrics, and so do the glyphs the MATH table reaches
/// from it (size variants, assembly parts). Glyph IDs name the paths in the SVG.
mod subsets {
    use std::collections::BTreeSet;
    use ttf_parser::{Face, GlyphId, OutlineBuilder};

    struct Path(String);

    impl OutlineBuilder for Path {
        fn move_to(&mut self, x: f32, y: f32) {
            self.0 += &format!("M{x} {y}");
        }
        fn line_to(&mut self, x: f32, y: f32) {
            self.0 += &format!("L{x} {y}");
        }
        fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
            self.0 += &format!("Q{x1} {y1} {x} {y}");
        }
        fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
            self.0 += &format!("C{x1} {y1} {x2} {y2} {x} {y}");
        }
        fn close(&mut self) {
            self.0 += "Z";
        }
    }

    fn outline(face: &Face, id: GlyphId) -> String {
        let mut p = Path(String::new());
        face.outline_glyph(id, &mut p);
        p.0
    }

    fn check(file: &str) {
        let dir = env!("CARGO_MANIFEST_DIR");
        let original = std::fs::read(format!("{dir}/../../tools/fonts/{file}")).unwrap();
        let embedded = std::fs::read(format!("{dir}/fonts/{}", file.replace(".otf", "-subset.otf"))).unwrap();
        let (a, b) = (Face::parse(&original, 0).unwrap(), Face::parse(&embedded, 0).unwrap());
        // (not a smaller file: with its outlines desubroutinized and the script forms kept, Latin
        // Modern's subset is larger than the original)
        assert!(a.tables().gpos.is_some() && b.tables().gpos.is_none(), "{file} is not a subset: run tools/fonts/subset.sh");
        // --retain-gids keeps every ID in place, though it may drop unused glyphs at the very end
        assert!(b.number_of_glyphs() <= a.number_of_glyphs());
        let (ma, mb) = (a.tables().math.unwrap(), b.tables().math.unwrap());

        let mut reached = BTreeSet::new();
        for subtable in a.tables().cmap.unwrap().subtables {
            if !subtable.is_unicode() {
                continue;
            }
            subtable.codepoints(|c| {
                let Some(c) = char::from_u32(c) else { return };
                // a character the original maps to .notdef is missing in the subset, as in any font
                if let Some(id) = a.glyph_index(c).filter(|id| id.0 != 0) {
                    assert_eq!(b.glyph_index(c), Some(id), "{file}: {c:?} lost its glyph");
                    reached.insert(id);
                }
            });
        }
        for id in reached.clone() {
            for coverage in [ma.variants.unwrap().vertical_constructions, ma.variants.unwrap().horizontal_constructions] {
                if let Some(construction) = coverage.get(id) {
                    reached.extend(construction.variants.into_iter().map(|v| v.variant_glyph));
                    if let Some(assembly) = construction.assembly {
                        reached.extend(assembly.parts.into_iter().map(|p| p.glyph_id));
                    }
                }
            }
        }
        let info = |m: ttf_parser::math::Table<'_>, id| {
            let g = m.glyph_info.unwrap();
            (g.italic_corrections.and_then(|c| c.get(id)).map(|v| v.value), g.top_accent_attachments.and_then(|c| c.get(id)).map(|v| v.value))
        };
        for &id in &reached {
            assert_eq!(outline(&a, id), outline(&b, id), "{file}: glyph {} changed its outline", id.0);
            assert_eq!(a.glyph_hor_advance(id), b.glyph_hor_advance(id), "{file}: glyph {} changed its advance", id.0);
            assert_eq!(info(ma, id), info(mb, id), "{file}: glyph {} changed its MATH info", id.0);
        }
        assert!(reached.len() > 2500, "{file}: only {} glyphs reached", reached.len());
    }

    #[test]
    fn stix_two_keeps_every_glyph_it_draws() {
        check("STIXTwoMath-Regular.otf");
    }

    #[test]
    fn latin_modern_keeps_every_glyph_it_draws() {
        check("latinmodern-math.otf");
    }
}

/// A font from outside: any OpenType math font, loaded from its bytes (here the full Latin Modern
/// Math of tools/fonts, not the embedded subset, and without the `latin-modern` feature).
mod loaded {
    use context_equation_core::font::{FontError, MathFont};
    use context_equation_core::{convert, Options};

    fn latin_modern() -> MathFont {
        let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/fonts/latinmodern-math.otf")).unwrap();
        MathFont::load(bytes).unwrap()
    }

    #[test]
    fn is_named_by_the_font_and_loaded_once() {
        let f = latin_modern();
        assert_eq!(f.name(), "Latin Modern Math");
        assert_eq!(f, latin_modern());
        assert_ne!(f, MathFont::StixTwo);
        assert!(MathFont::loaded().contains(&f));
    }

    #[test]
    fn sets_the_equation_in_its_glyphs() {
        let options = Options { font: latin_modern(), ..Options::default() };
        let theirs = convert(r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}", &options).unwrap().svg;
        let ours = convert(r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}", &Options::default()).unwrap().svg;
        assert!(theirs.contains("<contextEquation:font>Latin Modern Math</contextEquation:font>"));
        // other outlines, and other ids: both can be on one page
        let id = |svg: &str| svg.split(r#"id=""#).nth(1).unwrap().split('-').take(2).collect::<Vec<_>>().join("-");
        assert_ne!(id(&theirs), id(&ours));
        assert_ne!(theirs.split("<defs>").nth(1), ours.split("<defs>").nth(1));
    }

    #[test]
    fn draws_the_gallery() {
        // every equation of the gallery the embedded font draws, a loaded math font draws too
        let gallery: serde_json::Value = serde_json::from_str(include_str!("../../../corpus/gallery.json")).unwrap();
        let font = latin_modern();
        let (mut failed, mut drawn) = (Vec::new(), 0);
        for row in gallery.as_array().unwrap().iter().flat_map(|g| g["rows"].as_array().unwrap()) {
            let tex = row["tex"].as_str().unwrap();
            let locale = row["locale"].as_str().unwrap_or("en").parse().unwrap();
            let input = row.get("input").and_then(|i| i.as_str()).unwrap_or("latex").parse().unwrap();
            let options = Options { locale, input, ..Options::default() };
            if convert(tex, &options).is_ok() {
                drawn += 1;
                if let Err(e) = convert(tex, &Options { font, ..options }) {
                    failed.push(format!("{}: {e}", row["name"]));
                }
            }
        }
        assert!(drawn >= 40, "only {drawn} equations");
        assert!(failed.is_empty(), "{failed:#?}");
    }

    #[test]
    fn the_pen_writes_in_it() {
        let options = Options { font: latin_modern(), ..Options::default() };
        let svg = context_equation_core::pen("x^2", &options, &Default::default()).unwrap();
        assert!(svg.contains("<animateMotion") && svg.contains("context-equation-pen-"));
    }

    #[test]
    fn what_is_not_a_math_font_is_refused() {
        assert!(matches!(MathFont::load(b"not a font".to_vec()), Err(FontError::NotAFont(_))));
        // a font without a MATH table: the embedded STIX Two Math with its MATH table renamed
        let mut bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/STIXTwoMath-Regular-subset.otf")).unwrap();
        let at = bytes.windows(4).position(|w| w == b"MATH").expect("a table directory entry");
        bytes[at..at + 4].copy_from_slice(b"XATH");
        let e = MathFont::load(bytes).unwrap_err();
        assert!(matches!(e, FontError::NoMathTable { .. }) && e.to_string().contains("no OpenType MATH table"), "{e}");
    }
}
