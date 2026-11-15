//! KaTeX's texvc aliases (#172), the names MediaWiki equations use: each reads as the command KaTeX
//! expands it to (KaTeX's macros.ts and font.ts), and an author's own definition of the name wins.

use context_equation_core::input::symbols;
use context_equation_core::{convert, font, Options};

/// (alias, what KaTeX expands it to).
const ALIASES: &[(&str, &str)] = &[
    // symbols
    (r"\alef", r"\aleph"),
    (r"\alefsym", r"\aleph"),
    (r"\bull", r"\bullet"),
    (r"\clubs", r"\clubsuit"),
    (r"\diamonds", r"\diamondsuit"),
    (r"\hearts", r"\heartsuit"),
    (r"\spades", r"\spadesuit"),
    (r"\cnums", r"\mathbb{C}"),
    (r"\Complex", r"\mathbb{C}"),
    (r"\N", r"\mathbb{N}"),
    (r"\natnums", r"\mathbb{N}"),
    (r"\R", r"\mathbb{R}"),
    (r"\Reals", r"\mathbb{R}"),
    (r"\reals", r"\mathbb{R}"),
    (r"\Z", r"\mathbb{Z}"),
    (r"\Dagger", r"\ddagger"),
    (r"\empty", r"\emptyset"),
    (r"\exist", r"\exists"),
    (r"\image", r"\Im"),
    (r"\infin", r"\infty"),
    (r"\isin", r"\in"),
    (r"\plusmn", r"\pm"),
    (r"\sdot", r"\cdot"),
    (r"\sub", r"\subset"),
    (r"\sube", r"\subseteq"),
    (r"\supe", r"\supseteq"),
    (r"\thetasym", r"\vartheta"),
    (r"\weierp", r"\wp"),
    (r"\real", r"\Re"),
    (r"\lang", r"\langle"),
    (r"\rang", r"\rangle"),
    // arrows
    (r"\Darr", r"\Downarrow"),
    (r"\dArr", r"\Downarrow"),
    (r"\darr", r"\downarrow"),
    (r"\Harr", r"\Leftrightarrow"),
    (r"\hArr", r"\Leftrightarrow"),
    (r"\harr", r"\leftrightarrow"),
    (r"\Larr", r"\Leftarrow"),
    (r"\lArr", r"\Leftarrow"),
    (r"\larr", r"\leftarrow"),
    (r"\Lrarr", r"\Leftrightarrow"),
    (r"\lrArr", r"\Leftrightarrow"),
    (r"\lrarr", r"\leftrightarrow"),
    (r"\Rarr", r"\Rightarrow"),
    (r"\rArr", r"\Rightarrow"),
    (r"\rarr", r"\rightarrow"),
    (r"\Uarr", r"\Uparrow"),
    (r"\uArr", r"\Uparrow"),
    (r"\uarr", r"\uparrow"),
    // fonts
    (r"\bold{AaBb}", r"\mathbf{AaBb}"),
    (r"\bold x", r"\mathbf x"),
    (r"\frak{AaBb}", r"\mathfrak{AaBb}"),
    // operators, read as the functions they name (as \cotg and \senh are)
    (r"\arcctg x", r"\operatorname{arccot} x"),
    (r"\ctg x", r"\cot x"),
    (r"\cth x", r"\coth x"),
    (r"\sh x", r"\sinh x"),
    (r"\ch x", r"\cosh x"),
    (r"\th x", r"\tanh x"),
    (r"\plim_{n} x_n", r"\operatorname*{plim}_{n} x_n"),
    // text
    (r"\text{\sect 3}", r"\text{\S 3}"),
];

fn strict() -> Options {
    Options { strict: true, ..Options::default() }
}

fn tree(tex: &str) -> String {
    convert(tex, &strict()).unwrap_or_else(|e| panic!("{tex}: {e}")).tree.to_json()
}

#[test]
fn every_alias_reads_as_what_katex_expands_it_to() {
    for &(alias, target) in ALIASES {
        assert_eq!(tree(alias), tree(target), "{alias} is not {target}");
        let tolerant = convert(alias, &Options::default()).unwrap();
        assert!(tolerant.warnings.is_empty(), "{alias}: {:?}", tolerant.warnings);
    }
}

#[test]
fn every_symbol_alias_is_in_the_font() {
    for &(alias, _) in ALIASES {
        let name = alias.trim_start_matches('\\');
        if let Some(s) = symbols::command(name) {
            assert!(font::font().glyph(s.symbol).is_some(), "{alias}: STIX Two Math has no {}", s.symbol);
        }
    }
    for c in "ℕℝℤℂϑ".chars() {
        assert!(font::font().glyph(c).is_some(), "STIX Two Math has no {c}");
    }
}

/// The aliases that start like a JSON escape (`\r`, `\n`, `\t`) are known commands: they never reach the
/// E-REC-2 recovery, which would read them as a line break and the rest of the name.
#[test]
fn aliases_that_look_like_json_escapes_read_as_the_alias() {
    for (alias, target) in [
        (r"\real", r"\Re"),
        (r"\rarr", r"\rightarrow"),
        (r"\th x", r"\tanh x"),
        (r"\natnums", r"\mathbb{N}"),
        (r"\rArr", r"\Rightarrow"),
        (r"\reals", r"\mathbb{R}"),
        (r"\rang", r"\rangle"),
    ] {
        let tolerant = convert(alias, &Options::default()).unwrap();
        assert!(tolerant.warnings.is_empty(), "{alias}: {:?}", tolerant.warnings);
        assert_eq!(tolerant.tree.to_json(), tree(target), "{alias}");
    }
}

#[test]
fn an_authors_definition_of_the_name_wins() {
    assert_eq!(tree(r"\newcommand{\R}{\mathcal{R}} \R"), tree(r"\mathcal{R}"));
    assert_eq!(tree(r"\renewcommand{\N}{\mathbf{N}} \N"), tree(r"\mathbf{N}"));
    assert_eq!(tree(r"\def\Z{z} \Z"), tree("z"));
    assert_eq!(tree(r"\newcommand{\th}{\theta} \th"), tree(r"\theta"));
    assert_eq!(tree(r"\def\sub#1{_{#1}} x\sub{1}"), tree("x_{1}"));
}

#[test]
fn the_delimiter_aliases_work_after_left_and_right() {
    assert_eq!(tree(r"\left\lang x \right\rang"), tree(r"\left\langle x \right\rangle"));
    assert_eq!(tree(r"\left\uarr x \right\darr"), tree(r"\left\uparrow x \right\downarrow"));
    assert_eq!(tree(r"\left\Uarr x \right\dArr"), tree(r"\left\Uparrow x \right\Downarrow"));
    assert_eq!(tree(r"\bigl\lang x \bigr\rang"), tree(r"\bigl\langle x \bigr\rangle"));
}

#[test]
fn the_issue_examples_read_without_warnings() {
    for tex in [r"f\colon \R \to \R", r"\alef_0 < \cnums", r"A \sube B \Rarr x \isin A", r"\hearts \spades \clubs \diamonds"] {
        let out = convert(tex, &Options::default()).unwrap();
        assert!(out.warnings.is_empty(), "{tex}: {:?}", out.warnings);
    }
    // the number sets keep their reading
    assert_eq!(convert(r"x \isin \R", &Options::default()).unwrap().speech, convert(r"x \in \mathbb{R}", &Options::default()).unwrap().speech);
}
