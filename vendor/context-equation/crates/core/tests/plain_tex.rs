//! Plain TeX and amsmath constructs of older material (issue #170): `\root…\of`, `\leftroot` and
//! `\uproot`, the generalized fractions with delimiters, `\buildrel`, `\skew`, `\overunderset`,
//! `\vline`, `xalignat`, `xxalignat`, `darray` and the corner delimiters.

use context_equation_core::tree::{Class, Node, TableKind};
use context_equation_core::{convert, Input, Options};

fn en(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options::default()).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

fn strict(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options { strict: true, ..Default::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

/// The tree read back from the MathML is the tree.
fn round_trips(tex: &str) {
    let first = en(tex);
    assert!(!first.mathml.contains("<semantics>"), "{tex}: a fallback: {}", first.mathml);
    let back = convert(&first.mathml, &Options { input: Input::MathMl, ..Default::default() }).unwrap();
    assert_eq!(back.tree, first.tree, "{tex}: {}", first.mathml);
}

#[test]
fn root_of() {
    // \root index \of radicand is \sqrt[index]{radicand}
    assert_eq!(strict(r"\root 3 \of {x}").tree, strict(r"\sqrt[3]{x}").tree);
    assert_eq!(strict(r"\root n+1 \of {\frac{a}{b}} + y").tree, strict(r"\sqrt[n+1]{\frac{a}{b}} + y").tree);
    assert_eq!(strict(r"\root {[n]} \of x").tree, strict(r"\sqrt[{[n]}]{x}").tree);
    // without \of, as before: the index in brackets
    assert_eq!(strict(r"\root[3]{y}").tree, strict(r"\sqrt[3]{y}").tree);
    // \of alone is not a command
    assert!(en(r"\root 3 x").warnings.is_empty() || en(r"\root 3 x").warnings.iter().all(|w| !w.description.contains("\\of")));
    round_trips(r"\root 3 \of {x}");
}

#[test]
fn left_root_and_up_root() {
    let shifted = strict(r"\sqrt[\leftroot{-2}\uproot{2}n]{x}");
    let Node::Root { index: Some(index), .. } = shifted.tree.single() else { panic!("{:?}", shifted.tree) };
    // up: the index raised; left (here, -2: to the right): a phantom space after it
    let children = index.children();
    assert!(matches!(children[0], Node::Raised { em, .. } if em > 0.0), "{index:?}");
    assert!(matches!(&children[1], Node::Phantom { base, .. } if matches!(base.single(), Node::Space { em } if *em < 0.0)), "{index:?}");
    // speech and text read the index as written
    assert_eq!(shifted.speech, strict(r"\sqrt[n]{x}").speech);
    assert_eq!(shifted.text, strict(r"\sqrt[n]{x}").text);
    assert_ne!(shifted.svg, strict(r"\sqrt[n]{x}").svg);
    // in MathML, the raised index is an mpadded with a voffset (it reads back as math inside text,
    // as \raisebox does)
    assert!(shifted.mathml.contains("<mroot>") && shifted.mathml.contains("voffset"), "{}", shifted.mathml);
    // only one of them
    let Node::Root { index: Some(index), .. } = strict(r"\sqrt[\uproot{3}\beta]{k}").tree.single().clone() else { panic!() };
    assert!(matches!(index.single(), Node::Raised { .. }));
    let Node::Root { index: Some(index), .. } = strict(r"\sqrt[\leftroot 2 \beta]{k}").tree.single().clone() else { panic!() };
    assert!(matches!(index.children()[1], Node::Phantom { .. }), "{index:?}");
    // outside a root: ignored with a warning, refused in strict mode
    assert!(en(r"\leftroot{2} x").warnings.iter().any(|w| w.rule == "E-REC-9" && w.description.contains("\\leftroot")));
    assert!(convert(r"\uproot{2} x", &Options { strict: true, ..Default::default() }).is_err());
}

#[test]
fn generalized_fractions_with_delimiters() {
    let delimited = |tex: &str| match strict(tex).tree.single().clone() {
        Node::Delimited { open, close, parts, .. } => (open, close, parts.into_iter().next().unwrap()),
        other => panic!("{tex}: {other:?}"),
    };
    // \atopwithdelims: no rule, as \choose
    let (open, close, fraction) = delimited(r"{n \atopwithdelims() k}");
    assert_eq!((open, close), (Some('('), Some(')')));
    assert!(matches!(fraction, Node::Fraction { rule: false, .. }));
    assert_eq!(strict(r"{n \atopwithdelims() k}").tree, strict(r"{n \choose k}").tree);
    // \overwithdelims: a rule; \abovewithdelims: a rule as thick as asked
    let (open, close, fraction) = delimited(r"{a \overwithdelims[] b}");
    assert_eq!((open, close), (Some('['), Some(']')));
    assert!(matches!(fraction, Node::Fraction { rule: true, thickness: None, .. }));
    let (open, close, fraction) = delimited(r"{a \abovewithdelims\langle\rangle 1pt b}");
    assert_eq!((open, close), (Some('⟨'), Some('⟩')));
    assert!(matches!(fraction, Node::Fraction { rule: true, thickness: Some(t), .. } if (t - 0.1).abs() < 1e-6));
    // the null delimiter on one side; on both, the plain fraction
    assert_eq!(delimited(r"{a \atopwithdelims.| b}").0, None);
    assert!(matches!(strict(r"{a \overwithdelims.. b}").tree.single(), Node::Fraction { rule: true, .. }));
    // the old ones are as before
    assert_eq!(strict(r"{a \above 2pt b}").text, "a/b");
    round_trips(r"{n \atopwithdelims() k}");
}

#[test]
fn buildrel_skew_and_overunderset() {
    // \buildrel a \over b is \stackrel{a}{b}: a relation
    assert_eq!(strict(r"x \buildrel \rm def \over = y").tree, strict(r"x \stackrel{\rm def}{=} y").tree);
    assert!(en(r"\buildrel a").warnings.iter().any(|w| w.rule == "E-REC-7"));
    // \skew{n}\hat{x}: the accent over the base and n mu, taken back after it
    let r = strict(r"\skew{6}\hat{A}");
    let Node::Accent { base, accent: '\u{0302}', .. } = &r.tree.children()[0] else { panic!("{:?}", r.tree) };
    assert_eq!(base.children()[1], Node::Space { em: 6.0 / 18.0 });
    assert_eq!(r.tree.children()[1], Node::Space { em: -6.0 / 18.0 });
    assert_eq!(strict(r"\skew3\tilde M").speech, strict(r"\tilde{M}").speech);
    // \overunderset{a}{b}{=}: above and below the base, which keeps its class
    let r = strict(r"x \overunderset{a}{b}{=} y");
    let Node::ForcedClass { class: Class::Rel, base } = &r.tree.children()[1] else { panic!("{:?}", r.tree) };
    assert!(matches!(base.as_ref(), Node::Stacked { above: Some(_), below: Some(_), base } if **base == Node::Operator { symbol: '=', class: Class::Rel }), "{base:?}");
    assert!(matches!(strict(r"\overunderset{a}{b}{x}").tree.single(), Node::ForcedClass { class: Class::Ord, .. }));
}

#[test]
fn vline_and_the_obsolete_alignments() {
    // \vline: a rule as tall as the row, where it is written
    let r = strict(r"\begin{array}{cc} a & \vline\, b \\ c & \vline\, d \end{array}");
    let Node::Table { rows, .. } = r.tree.single() else { panic!() };
    assert!(
        matches!(rows[0][1].children()[0], Node::Rule { width, height, raise } if width == 0.04 && (height - 1.2).abs() < 1e-6 && (raise + 0.36).abs() < 1e-6),
        "{:?}",
        rows[0][1]
    );
    let r = strict(r"\def\arraystretch{2} \begin{array}{c} \vline a \end{array}");
    let Node::Table { rows, .. } = r.tree.single() else { panic!() };
    assert!(matches!(rows[0][0].children()[0], Node::Rule { height, .. } if (height - 2.4).abs() < 1e-6), "{:?}", rows[0][0]);
    // xalignat and xxalignat are alignat, with no line width to spread over
    let alignat = strict(r"\begin{alignat}{2} a &= b & c &= d \\ e &= f & g &= h \end{alignat}").tree;
    assert_eq!(strict(r"\begin{xalignat}{2} a &= b & c &= d \\ e &= f & g &= h \end{xalignat}").tree, alignat);
    assert_eq!(strict(r"\begin{xxalignat}{2} a &= b & c &= d \\ e &= f & g &= h \end{xxalignat}").tree, alignat);
    assert_eq!(strict(r"\begin{xalignat*}{2} a &= b & c &= d \end{xalignat*}").text, "a = b c = d");
    // darray: an array whose cells are in display style
    let r = strict(r"\begin{darray}{cc} \frac{a}{b} & \sum_{i=1}^n i \\[2pt] x & {\begin{matrix} 1 & 2 \end{matrix}} \end{darray}");
    let Node::Table { shape: TableKind::Array, rows, columns, .. } = r.tree.single() else { panic!("{:?}", r.tree) };
    assert_eq!(columns, &vec!['c', 'c']);
    assert!(rows.iter().flatten().all(|cell| matches!(cell, Node::Style { level: 0, .. })), "{rows:?}");
    assert_eq!(rows.len(), 2);
    round_trips(r"\begin{xalignat}{2} a &= b & c &= d \end{xalignat}");
}

#[test]
fn corner_delimiters() {
    for (left, right, open, close) in [("ulcorner", "urcorner", '⌜', '⌝'), ("llcorner", "lrcorner", '⌞', '⌟')] {
        let r = strict(&format!(r"\left\{left} \frac{{a}}{{b}} \right\{right}"));
        assert!(matches!(r.tree.single(), Node::Delimited { open: Some(o), close: Some(c), .. } if *o == open && *c == close), "{:?}", r.tree);
        assert!(r.warnings.is_empty());
    }
    // \bigl too; and still symbols of their own outside delimiters
    assert!(matches!(strict(r"\bigl\ulcorner x \bigr\urcorner").tree.children()[0], Node::FixedDelimiter { symbol: '⌜', .. }));
    assert_eq!(strict(r"\ulcorner x \urcorner").tree.children()[0], Node::Operator { symbol: '⌜', class: Class::Open });
    round_trips(r"\left\ulcorner \frac{a}{b} \right\urcorner");
}
