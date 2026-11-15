//! The stmaryrd and mathabx symbols #124 did not add (#176): the ones Unicode has a character for,
//! each in the class its package declares and drawn by the font.

use context_equation_core::tree::{Class, Node};
use context_equation_core::{convert, font, Options};

use Class::*;

/// (name, character, class): stmaryrd.dtx and mathabx.dcl for the classes; unicode-math-table.tex,
/// unimathsymbols.txt and, for \minuso, KaTeX for the characters.
const SYMBOLS: &[(&str, char, Class)] = &[
    ("minuso", '⦵', Bin),
    ("Yup", '⅄', Bin),
    ("boxbar", '◫', Bin),
    ("leftslice", '⪦', Bin),
    ("rightslice", '⪧', Bin),
    ("binampersand", '&', Open),
    ("bindnasrepma", '⅋', Close),
    ("subsetplus", '⪿', Rel),
    ("supsetplus", '⫀', Rel),
    ("leftarrowtriangle", '⇽', Rel),
    ("rightarrowtriangle", '⇾', Rel),
    ("leftrightarrowtriangle", '⇿', Rel),
    ("ndivides", '∤', Rel),
    ("notdivides", '∤', Rel),
    ("leftsquigarrow", '⇜', Rel),
    ("Sun", '☉', Ord),
];

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
fn every_symbol_reads_in_its_class() {
    for &(name, symbol, class) in SYMBOLS {
        let tex = format!("\\{name}");
        let out = convert(&tex, &strict()).unwrap_or_else(|e| panic!("{tex}: {e}"));
        assert_eq!(only(&out.tree), &Node::Operator { symbol, class }, "{tex}");
        let tolerant = convert(&tex, &Options::default()).unwrap();
        assert!(tolerant.warnings.is_empty(), "{tex}: {:?}", tolerant.warnings);
        assert!(font::font().glyph(symbol).is_some(), "{tex}: STIX Two Math has no {symbol}");
    }
}

#[test]
fn the_big_operators_take_limits_like_bigsqcup() {
    for (name, symbol) in [("bigsqcap", '⨅'), ("biginterleave", '⫼')] {
        let tex = format!("\\{name}_{{i}} A_i");
        let out = convert(&tex, &strict()).unwrap_or_else(|e| panic!("{tex}: {e}"));
        assert!(format!("{:?}", out.tree).contains(&format!("LargeOperator {{ symbol: '{symbol}'")), "{tex}: {:?}", out.tree);
        assert!(font::font().glyph(symbol).is_some(), "{tex}");
        // limits under in display, as \bigsqcup's: deeper than the limit at the side
        let depth = |tex: &str| -> f32 {
            let (b, _) = context_equation_core::typeset(tex, &strict()).unwrap();
            b.depth
        };
        let side = format!("\\{name}\\nolimits_{{i}} A_i");
        assert!(depth(&tex) > depth(&side) + 0.1, "{tex}: {} vs {}", depth(&tex), depth(&side));
    }
}

#[test]
fn the_issue_examples_that_unicode_can_draw_read_without_warnings() {
    for tex in [r"a \minuso b", r"\bigsqcap_{i} A_i", r"f \leftsquigarrow g", r"a \ndivides b"] {
        let out = convert(tex, &Options::default()).unwrap();
        assert!(out.warnings.is_empty(), "{tex}: {:?}", out.warnings);
    }
}
