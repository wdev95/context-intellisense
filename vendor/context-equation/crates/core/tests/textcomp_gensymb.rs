//! textcomp's and gensymb's symbols (#174): in math, as MathJax's textcomp sets them, and in `\text`.

use context_equation_core::tree::{Class, Node};
use context_equation_core::{convert, font, Options};

use Class::*;

/// (name, character, class in math). The characters are those of LaTeX's utf8ienc.dtx (its TS1 lines), or
/// of MathJax's TextcompMappings.ts for the names LaTeX does not map.
const SYMBOLS: &[(&str, char, Class)] = &[
    // the kernel's text symbols, which used to work only in \text
    ("textasciicircum", '^', Ord),
    ("textasciitilde", '~', Ord),
    ("textbar", '|', Ord),
    ("textbardbl", '‖', Ord),
    ("textbraceleft", '{', Open),
    ("textbraceright", '}', Close),
    ("textbullet", '•', Ord),
    ("textdagger", '†', Ord),
    ("textdaggerdbl", '‡', Ord),
    ("textdollar", '$', Ord),
    ("textellipsis", '…', Ord),
    ("textemdash", '—', Ord),
    ("textendash", '–', Ord),
    ("textexclamdown", '¡', Ord),
    ("textordfeminine", 'ª', Ord),
    ("textordmasculine", 'º', Ord),
    ("textparagraph", '¶', Ord),
    ("textperiodcentered", '·', Bin),
    ("textquestiondown", '¿', Ord),
    ("textquotedblleft", '“', Ord),
    ("textquotedblright", '”', Ord),
    ("textquoteleft", '‘', Ord),
    ("textquoteright", '’', Ord),
    ("textsection", '§', Ord),
    ("textunderscore", '_', Ord),
    ("textvisiblespace", '\u{2423}', Ord),
    // diacritics
    ("textacutedbl", '\u{02DD}', Ord),
    ("textasciiacute", '\u{00B4}', Ord),
    ("textasciibreve", '\u{02D8}', Ord),
    ("textasciicaron", '\u{02C7}', Ord),
    ("textasciidieresis", '\u{00A8}', Ord),
    ("textasciimacron", '\u{00AF}', Ord),
    ("texttildelow", '\u{02F7}', Ord),
    // currency
    ("textcurrency", '\u{00A4}', Ord),
    ("textdong", '\u{20AB}', Ord),
    ("textflorin", '\u{0192}', Ord),
    ("textlira", '\u{20A4}', Ord),
    ("textyen", '\u{00A5}', Ord),
    // legal
    ("textcircledP", '\u{2117}', Ord),
    ("textservicemark", '\u{2120}', Ord),
    ("texttrademark", '\u{2122}', Ord),
    // miscellaneous
    ("textbigcircle", '\u{25EF}', Ord),
    ("textblank", '\u{2422}', Ord),
    ("textbrokenbar", '\u{00A6}', Ord),
    ("textdiscount", '\u{2052}', Ord),
    ("textestimated", '\u{212E}', Ord),
    ("textmusicalnote", '\u{266A}', Ord),
    ("textnumero", '\u{2116}', Ord),
    ("textopenbullet", '\u{25E6}', Ord),
    ("textpertenthousand", '\u{2031}', Ord),
    ("textrecipe", '\u{211E}', Ord),
    ("textreferencemark", '\u{203B}', Ord),
    // delimiters
    ("textlangle", '\u{27E8}', Open),
    ("textrangle", '\u{27E9}', Close),
    ("textlbrackdbl", '\u{27E6}', Open),
    ("textrbrackdbl", '\u{27E7}', Close),
    // math and science
    ("textcelsius", '\u{2103}', Ord),
    ("textdiv", '÷', Bin),
    ("textdownarrow", '↓', Rel),
    ("textfractionsolidus", '\u{2044}', Ord),
    ("textleftarrow", '←', Rel),
    ("textlnot", '¬', Ord),
    ("textmho", '\u{2127}', Ord),
    ("textminus", '\u{2212}', Bin),
    ("textohm", '\u{2126}', Ord),
    ("textonehalf", '\u{00BD}', Ord),
    ("textonequarter", '\u{00BC}', Ord),
    ("textonesuperior", '\u{00B9}', Ord),
    ("textpm", '±', Bin),
    ("textrightarrow", '→', Rel),
    ("textsurd", '√', Ord),
    ("textthreequarters", '\u{00BE}', Ord),
    ("textthreesuperior", '\u{00B3}', Ord),
    ("texttimes", '×', Bin),
    ("texttwosuperior", '\u{00B2}', Ord),
    ("textuparrow", '↑', Rel),
    // genealogy
    ("textdied", '†', Ord),
    ("textmarried", '\u{26AD}', Ord),
    ("textdivorced", '\u{26AE}', Ord),
    // gensymb
    ("perthousand", '\u{2030}', Ord),
    ("ohm", 'Ω', Ord),
    ("celsius", '\u{2103}', Ord),
    ("degree", '°', Ord),
    ("micro", 'µ', Ord),
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
fn every_symbol_works_in_math() {
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
fn every_symbol_works_in_text() {
    for &(name, symbol, _) in SYMBOLS {
        let tex = format!("\\text{{a \\{name}{{}} b}}");
        let out = convert(&tex, &strict()).unwrap_or_else(|e| panic!("{tex}: {e}"));
        assert!(out.text.contains(symbol), "{tex}: {}", out.text);
        assert!(!out.text.contains(name), "{tex}: {}", out.text);
    }
}

/// E-REC-1 used to cut an unknown `\textcelsius` into `\text` and `celsius`; a `\text…` name it does not
/// know is now drawn in red, as written.
#[test]
fn text_names_are_not_cut_into_text_and_a_word() {
    let out = convert(r"\textcelsius", &Options::default()).unwrap();
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    assert_eq!(out.text, "℃");
    let out = convert(r"\textfoo", &Options::default()).unwrap();
    assert!(out.warnings.iter().any(|w| w.rule == "E-REC-1" && w.description.contains("drawn in red")), "{:?}", out.warnings);
    assert!(out.svg.contains("#c0392b"));
    assert!(!out.text.starts_with("foo"), "{}", out.text);
    // nor any other name (#209): \alphax is drawn whole, not \alpha and x
    let out = convert(r"\alphax", &Options::default()).unwrap();
    assert!(out.warnings.iter().any(|w| w.rule == "E-REC-1" && w.description.contains("drawn in red")), "{:?}", out.warnings);
    assert_eq!(out.text, r"\alphax");
}

#[test]
fn the_issue_examples_read_without_warnings() {
    for (tex, text) in [(r"3\perthousand", "3‰"), (r"25\,\textcelsius", "25 ℃"), (r"R = 50\,\textohm", "R = 50 \u{2126}"), (r"\textonehalf + \textonequarter", "½ + ¼")] {
        let out = convert(tex, &Options::default()).unwrap();
        assert!(out.warnings.is_empty(), "{tex}: {:?}", out.warnings);
        assert_eq!(out.text, text, "{tex}");
    }
    // a binary operator and a relation keep their spacing in math
    assert_eq!(convert(r"a \texttimes b \textrightarrow c", &strict()).unwrap().speech, convert(r"a \times b \rightarrow c", &strict()).unwrap().speech);
}
