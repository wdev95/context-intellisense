//! Smaller missing commands (#177): braket's suggested `\Ketbra`, bm's `\hm`, revtex's `\openone`, the
//! archaic Greek letters, `\smiley`, `\lAngle`/`\rAngle`, `\wedgeq`/`\corresponds`, `\sout`, `\textcircled`
//! and `\textsl` in text, MathJax's `\data` and `\Newextarrow`, mhchem's `\cf`; and colortbl and the array
//! package's column declarations, read and ignored with a warning.

use context_equation_core::tree::{Class, Node, Variant};
use context_equation_core::{convert, font, Options};

fn strict() -> Options {
    Options { strict: true, ..Options::default() }
}

fn tree(tex: &str) -> String {
    convert(tex, &strict()).unwrap_or_else(|e| panic!("{tex}: {e}")).tree.to_json()
}

fn only(node: &Node) -> &Node {
    match node {
        Node::Row { children } if children.len() == 1 => only(&children[0]),
        other => other,
    }
}

fn clean(tex: &str) -> context_equation_core::Output {
    let out = convert(tex, &Options::default()).unwrap();
    assert!(out.warnings.is_empty(), "{tex}: {:?}", out.warnings);
    convert(tex, &strict()).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

#[test]
fn the_symbols_read_in_their_class_and_are_in_the_font() {
    for (name, symbol, class) in [
        ("smiley", '☺', Class::Ord),
        ("openone", '𝟙', Class::Ord),
        ("lAngle", '⟪', Class::Open),
        ("rAngle", '⟫', Class::Close),
        ("wedgeq", '≙', Class::Rel),
        ("corresponds", '≙', Class::Rel),
    ] {
        let tex = format!("\\{name}");
        assert_eq!(only(&clean(&tex).tree), &Node::Operator { symbol, class }, "{tex}");
        assert!(font::font().glyph(symbol).is_some(), "{tex}: STIX Two Math has no {symbol}");
    }
    // \smiley is no longer \smile and a y
    assert_eq!(clean(r"\smiley").text, "☺");
    // and they stretch after \left and \right
    match only(&clean(r"\left\lAngle x \right\rAngle").tree) {
        Node::Delimited { open, close, .. } => assert_eq!((*open, *close), (Some('⟪'), Some('⟫'))),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_archaic_greek_letters() {
    for (name, letter) in [("Coppa", 'Ϙ'), ("coppa", 'ϙ'), ("Koppa", 'Ϟ'), ("koppa", 'ϟ'), ("Stigma", 'Ϛ'), ("stigma", 'ϛ'), ("Sampi", 'Ϡ'), ("sampi", 'ϡ')] {
        let tex = format!("\\{name}");
        match only(&clean(&tex).tree) {
            Node::Identifier { name: drawn, .. } => assert_eq!(drawn.chars().collect::<Vec<_>>(), vec![letter], "{tex}"),
            other => panic!("{tex}: {other:?}"),
        }
        assert!(font::font().glyph(letter).is_some(), "{tex}: STIX Two Math has no {letter}");
    }
}

#[test]
fn ketbra_hm_data_and_cf() {
    assert_eq!(tree(r"\Ketbra{\psi}{\phi}"), tree(r"\left|{\psi}\vphantom{\phi}\right\rangle\left\langle{\phi}\vphantom{\psi}\right|"));
    match only(&clean(r"\hm{x}").tree) {
        Node::Identifier { variant, .. } => assert_eq!(*variant, Variant::BoldItalic),
        other => panic!("\\hm: {other:?}"),
    }
    assert_eq!(tree(r"\data{foo=bar}{x+1}"), tree(r"\htmlData{foo=bar}{x+1}"));
    assert_eq!(tree(r"\cf{H2O}"), tree(r"\ce{H2O}"));
    assert_eq!(tree(r"\text{water, \cf{H2O}}"), tree(r"\text{water, \ce{H2O}}"));
}

#[test]
fn sout_textcircled_and_textsl_inside_text() {
    assert_eq!(tree(r"\text{a \sout{bc} d}"), tree(r"\text{a $\sout{\text{bc}}$ d}"));
    assert_eq!(tree(r"\text{a \textcircled{b}}"), tree(r"\text{a $\textcircled{\text{b}}$}"));
    assert_eq!(clean(r"\text{a \textsl{bc}}").text, clean(r"\text{a bc}").text);
}

#[test]
fn newextarrow_defines_an_arrow() {
    // an arrow contextEquation stretches: the same as its \x… command, labels and all
    assert_eq!(tree(r"\Newextarrow{\xfoo}{5,10}{0x21C1} A \xfoo[b]{a} B"), tree(r"A \xrightharpoondown[b]{a} B"));
    assert_eq!(tree(r"\Newextarrow{\xfoo}{5,10}{8658} A \xfoo{a} B"), tree(r"A \xRightarrow{a} B"));
    // any other: its character with the labels over and under it, with a warning
    let out = convert(r"\Newextarrow{\xbar}{5,10}{0x2933} A \xbar{c} B", &Options::default()).unwrap();
    assert!(out.warnings.iter().any(|w| w.rule == "E-REC-9" && w.description.contains("U+2933")), "{:?}", out.warnings);
    assert!(out.text.contains('⤳'), "{}", out.text);
    // an empty optional label is no label, as in \xrightarrow[]{a}
    assert_eq!(tree(r"\xrightarrow[]{a}"), tree(r"\xrightarrow{a}"));
}

/// colortbl and the array package's `>{…}` do not fit contextEquation's tables (no cell colors, nothing applied per
/// cell): read with their arguments and ignored, with E-REC-9; refused in strict mode.
#[test]
fn colortbl_and_column_declarations_are_ignored_with_a_warning() {
    for (tex, plain) in [
        (r"\begin{array}{cc} \cellcolor{yellow} a & b \end{array}", r"\begin{array}{cc} a & b \end{array}"),
        (r"\begin{array}{cc} \rowcolor[gray]{0.9}[1pt][2pt] a & b \end{array}", r"\begin{array}{cc} a & b \end{array}"),
        (r"\begin{array}{>{\columncolor{red}}cc} a & b \end{array}", r"\begin{array}{cc} a & b \end{array}"),
        (r"\begin{array}{>{\displaystyle}c<{\,}c} a & b \end{array}", r"\begin{array}{cc} a & b \end{array}"),
        (r"\arrayrulecolor{blue} \begin{array}{|c|} a \end{array}", r"\begin{array}{|c|} a \end{array}"),
        (r"\newcolumntype{Y}{>{\centering}c} x", "x"),
    ] {
        let out = convert(tex, &Options::default()).unwrap();
        assert!(!out.warnings.is_empty() && out.warnings.iter().all(|w| w.rule == "E-REC-9"), "{tex}: {:?}", out.warnings);
        assert_eq!(out.tree.to_json(), tree(plain), "{tex}");
        assert!(convert(tex, &strict()).is_err(), "{tex}: accepted in strict mode");
    }
}

#[test]
fn the_issue_examples() {
    clean(r"\Ketbra{\psi}{\phi}");
    clean(r"\Koppa \koppa \Stigma \stigma \Sampi \sampi");
    clean(r"\smiley");
    let out = convert(r"\begin{array}{cc}\cellcolor{yellow} a & b\end{array}", &Options::default()).unwrap();
    assert_eq!(out.warnings.len(), 1, "{:?}", out.warnings);
}
