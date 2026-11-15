//! Issue #216: plain TeX's `\sevenrm` and its kin, the size commands written as environments in math
//! (`\begin{tiny}…\end{tiny}`) and youngtab's `\yng`, which contextEquation leaves out (spec/boundary.md).

use context_equation_core::{convert, Input, Options};

fn en(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options::default()).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

fn strict(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options { strict: true, ..Default::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

fn round_trips(tex: &str) {
    let first = en(tex);
    assert!(!first.mathml.contains("<semantics>"), "{tex}: a fallback: {}", first.mathml);
    let back = convert(&first.mathml, &Options { input: Input::MathMl, ..Default::default() }).unwrap();
    assert_eq!(back.tree, first.tree, "{tex}: {}", first.mathml);
}

#[test]
fn seven_point_roman_is_script_style_roman() {
    // plain TeX's cmr7 beside 10-point text: \scriptstyle\rm, for the rest of the group
    assert_eq!(strict(r"{\sevenrm a}").tree, strict(r"{\scriptstyle\rm a}").tree);
    assert_eq!(strict(r"{\sevenrm ab} c").tree, strict(r"{\scriptstyle\rm ab} c").tree);
    assert_eq!(strict(r"{\fiverm x}").tree, strict(r"{\scriptscriptstyle\rm x}").tree);
    assert_eq!(strict(r"{\sevenbf x}").tree, strict(r"{\scriptstyle\bf x}").tree);
    assert_eq!(strict(r"{\fivebf x}").tree, strict(r"{\scriptscriptstyle\bf x}").tree);
    // the font ends with the group
    assert_eq!(strict(r"{\sevenrm a} b").tree, strict(r"{\scriptstyle\rm a} b").tree);
    assert!(en(r"{\sevenrm a}").warnings.is_empty());
    round_trips(r"{\sevenrm a} + b");
}

#[test]
fn size_environments_in_math_are_the_size_commands() {
    for size in ["tiny", "scriptsize", "footnotesize", "small", "normalsize", "large", "Large", "LARGE", "huge", "Huge"] {
        let env = strict(&format!(r"\begin{{{size}}} x \end{{{size}}} + y"));
        assert_eq!(env.tree, strict(&format!(r"{{\{size} x}} + y")).tree, "{size}");
        assert!(env.warnings.is_empty(), "{size}: {:?}", env.warnings);
    }
    // nested in itself and around a fraction
    assert_eq!(strict(r"\begin{tiny}\frac{a}{\begin{tiny}b\end{tiny}}\end{tiny}").tree, strict(r"{\tiny\frac{a}{{\tiny b}}}").tree);
    // an environment the equation defines itself keeps its definition
    assert_eq!(strict(r"\newenvironment{small}{[}{]}\begin{small}x\end{small}").tree, strict(r"{[x]}").tree);
}

#[test]
fn young_diagrams_are_out_of_scope() {
    // tolerant: a warning, the row lengths read with the command, which is written in red
    let out = en(r"\yng(2,1) \otimes x");
    assert!(out.warnings.iter().any(|w| w.rule == "E-REC-1" && w.description.contains(r"\yng(2,1)")), "{:?}", out.warnings);
    assert!(!out.text.contains("2,1)⊗") && out.text.contains('x'), "{}", out.text);
    assert!(out.svg.contains("#c0392b"));
    // strict: refused
    assert!(convert(r"\yng(2,1)", &Options { strict: true, ..Default::default() }).is_err());
}
