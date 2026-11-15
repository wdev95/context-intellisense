//! The symbol tables of input/symbols.rs, entry by entry: every name a LaTeX command, a MathML
//! entity or an AsciiMath word can reach, the character it stands for, and the properties the
//! readers rely on (the class of a character, which large operators take limits, which accents
//! stretch, which Greek letters stay upright).

use context_equation_core::composite;
use context_equation_core::input::symbols::{self, Symbol};
use context_equation_core::tree::Variant;
use context_equation_core::{convert, Class, Options};

fn latex(tex: &str) -> context_equation_core::Output {
    convert(tex, &Options { strict: true, ..Options::default() }).unwrap_or_else(|e| panic!("{tex}: {e}"))
}

#[test]
fn every_greek_letter_is_a_greek_character_of_the_case_its_name_says() {
    let letters = [
        ("alpha", 'α'),
        ("beta", 'β'),
        ("gamma", 'γ'),
        ("delta", 'δ'),
        ("epsilon", 'ϵ'),
        ("varepsilon", 'ε'),
        ("zeta", 'ζ'),
        ("eta", 'η'),
        ("theta", 'θ'),
        ("vartheta", 'ϑ'),
        ("iota", 'ι'),
        ("kappa", 'κ'),
        ("lambda", 'λ'),
        ("mu", 'μ'),
        ("nu", 'ν'),
        ("xi", 'ξ'),
        ("omicron", 'ο'),
        ("pi", 'π'),
        ("varpi", 'ϖ'),
        ("rho", 'ρ'),
        ("varrho", 'ϱ'),
        ("sigma", 'σ'),
        ("varsigma", 'ς'),
        ("tau", 'τ'),
        ("upsilon", 'υ'),
        ("phi", 'ϕ'),
        ("varphi", 'φ'),
        ("chi", 'χ'),
        ("psi", 'ψ'),
        ("omega", 'ω'),
        ("Gamma", 'Γ'),
        ("Delta", 'Δ'),
        ("Theta", 'Θ'),
        ("Lambda", 'Λ'),
        ("Xi", 'Ξ'),
        ("Pi", 'Π'),
        ("Sigma", 'Σ'),
        ("Upsilon", 'Υ'),
        ("Phi", 'Φ'),
        ("Psi", 'Ψ'),
        ("Omega", 'Ω'),
        ("upmu", 'μ'),
        ("uppi", 'π'),
        ("upalpha", 'α'),
        ("upbeta", 'β'),
        ("updelta", 'δ'),
        ("upepsilon", 'ε'),
        ("uptheta", 'θ'),
        ("uplambda", 'λ'),
        ("upsigma", 'σ'),
        ("upomega", 'ω'),
        ("upphi", 'φ'),
        ("uprho", 'ρ'),
        ("uptau", 'τ'),
        ("upupsilon", 'υ'),
        ("upkappa", 'κ'),
        ("upeta", 'η'),
        ("upnu", 'ν'),
        ("upxi", 'ξ'),
        ("upchi", 'χ'),
        ("uppsi", 'ψ'),
        ("upzeta", 'ζ'),
        ("upiota", 'ι'),
        ("Nu", 'Ν'),
        ("Alpha", 'Α'),
        ("Beta", 'Β'),
        ("Epsilon", 'Ε'),
        ("Zeta", 'Ζ'),
        ("Eta", 'Η'),
        ("Iota", 'Ι'),
        ("Kappa", 'Κ'),
        ("Mu", 'Μ'),
        ("Omicron", 'Ο'),
        ("Rho", 'Ρ'),
        ("Tau", 'Τ'),
        ("Chi", 'Χ'),
        ("Coppa", 'Ϙ'),
        ("coppa", 'ϙ'),
        ("Koppa", 'Ϟ'),
        ("koppa", 'ϟ'),
        ("Stigma", 'Ϛ'),
        ("stigma", 'ϛ'),
        ("Sampi", 'Ϡ'),
        ("sampi", 'ϡ'),
        ("upgamma", 'γ'),
        ("digamma", 'ϝ'),
        ("Digamma", 'Ϝ'),
        ("varkappa", 'ϰ'),
        ("Upgamma", 'Γ'),
        ("Updelta", 'Δ'),
        ("Uptheta", 'Θ'),
        ("Uplambda", 'Λ'),
        ("Upxi", 'Ξ'),
        ("Uppi", 'Π'),
        ("Upsigma", 'Σ'),
        ("Upupsilon", 'Υ'),
        ("Upphi", 'Φ'),
        ("Uppsi", 'Ψ'),
        ("Upomega", 'Ω'),
        ("upvarepsilon", 'ε'),
        ("upvartheta", 'ϑ'),
        ("upvarpi", 'ϖ'),
        ("upvarrho", 'ϱ'),
        ("upvarsigma", 'ς'),
        ("upvarphi", 'φ'),
        ("upomicron", 'ο'),
    ];
    for (name, c) in letters {
        assert_eq!(symbols::greek(name), Some(c), "{name}");
        assert!(('\u{0370}'..='\u{03FF}').contains(&c), "{name}: {c} is Greek");
        // \Gamma and \Upgamma are capitals; \gamma, \upgamma and \varkappa are not
        let capital = name.strip_prefix("up").unwrap_or(name).starts_with(|c: char| c.is_ascii_uppercase());
        assert_eq!(c.is_uppercase(), capital, "{name}: {c}");
        // TeX sets lowercase Greek in italic; upgreek's letters and the capitals stay upright
        assert_eq!(symbols::greek_upright(name), name.starts_with("up") || capital, "{name}");
    }
    assert_eq!(symbols::greek("alef"), None);
    assert!(!symbols::greek_upright("alpha"));
}

#[test]
fn function_names_and_their_brazilian_and_texvc_spellings() {
    let functions = [
        ("sin", ("sin", false)),
        ("sen", ("sin", false)),
        ("cos", ("cos", false)),
        ("tan", ("tan", false)),
        ("tg", ("tan", false)),
        ("cot", ("cot", false)),
        ("cotg", ("cot", false)),
        ("sec", ("sec", false)),
        ("csc", ("csc", false)),
        ("cossec", ("csc", false)),
        ("cosec", ("csc", false)),
        ("arcsin", ("arcsin", false)),
        ("arcsen", ("arcsin", false)),
        ("arccos", ("arccos", false)),
        ("arctan", ("arctan", false)),
        ("arctg", ("arctan", false)),
        ("sinh", ("sinh", false)),
        ("senh", ("sinh", false)),
        ("cosh", ("cosh", false)),
        ("tanh", ("tanh", false)),
        ("tgh", ("tanh", false)),
        ("coth", ("coth", false)),
        ("csch", ("csch", false)),
        ("sech", ("sech", false)),
        ("arccot", ("arccot", false)),
        ("arcsec", ("arcsec", false)),
        ("arccsc", ("arccsc", false)),
        ("log", ("log", false)),
        ("lg", ("lg", false)),
        ("ln", ("ln", false)),
        ("exp", ("exp", false)),
        ("arg", ("arg", false)),
        ("deg", ("deg", false)),
        ("dim", ("dim", false)),
        ("hom", ("hom", false)),
        ("ker", ("ker", false)),
        ("det", ("det", true)),
        ("gcd", ("gcd", true)),
        ("mdc", ("gcd", true)),
        ("lcm", ("lcm", true)),
        ("mmc", ("lcm", true)),
        ("max", ("max", true)),
        ("min", ("min", true)),
        ("sup", ("sup", true)),
        ("inf", ("inf", true)),
        ("lim", ("lim", true)),
        ("limsup", ("lim sup", true)),
        ("liminf", ("lim inf", true)),
        ("Pr", ("Pr", true)),
        ("argmax", ("arg max", true)),
        ("argmin", ("arg min", true)),
        ("ctg", ("cot", false)),
        ("arcctg", ("arccot", false)),
        ("sh", ("sinh", false)),
        ("ch", ("cosh", false)),
        ("th", ("tanh", false)),
        ("cth", ("coth", false)),
        ("plim", ("plim", true)),
    ];
    for (name, expected) in functions {
        assert_eq!(symbols::function(name), Some(expected), "{name}");
    }
    assert_eq!(symbols::function("foo"), None);
    // the spelling an author types is the function the reader draws: \sen x is \sin x
    assert_eq!(latex(r"\sen x").tree, latex(r"\sin x").tree);
    assert_eq!(latex(r"\tg x").tree, latex(r"\tan x").tree);
}

#[test]
fn large_operators_take_limits_below_as_their_character_says() {
    let operators = [
        ("sum", ('∑', true)),
        ("prod", ('∏', true)),
        ("coprod", ('∐', true)),
        ("bigcup", ('⋃', true)),
        ("bigcap", ('⋂', true)),
        ("bigoplus", ('⨁', true)),
        ("bigotimes", ('⨂', true)),
        ("bigodot", ('⨀', true)),
        ("bigvee", ('⋁', true)),
        ("bigwedge", ('⋀', true)),
        ("bigsqcup", ('⨆', true)),
        ("biguplus", ('⨄', true)),
        ("bigtimes", ('⨉', true)),
        ("int", ('∫', false)),
        ("intop", ('∫', true)),
        ("ointop", ('∮', true)),
        ("iint", ('∬', false)),
        ("iiint", ('∭', false)),
        ("oint", ('∮', false)),
        ("iiiint", ('⨌', false)),
        ("oiint", ('∯', false)),
        ("varoiint", ('∯', false)),
        ("oiiint", ('∰', false)),
        ("intclockwise", ('∱', false)),
        ("ointclockwise", ('∲', false)),
        ("varointclockwise", ('∲', false)),
        ("ointctrclockwise", ('∳', false)),
        ("varointctrclockwise", ('∳', false)),
        ("fint", ('⨏', false)),
        ("sqint", ('⨖', false)),
        ("bigsqcap", ('⨅', true)),
        ("biginterleave", ('⫼', true)),
        ("bigbox", (composite::BIGBOX, true)),
        ("bigparallel", (composite::BIGPARALLEL, true)),
        ("bignplus", (composite::BIGNPLUS, true)),
        ("bigcurlyvee", (composite::BIGCURLYVEE, true)),
        ("bigcurlywedge", (composite::BIGCURLYWEDGE, true)),
        ("bigboxplus", (composite::BIGBOXPLUS, true)),
        ("bigominus", (composite::BIGOMINUS, true)),
        ("bigoslash", (composite::BIGOSLASH, true)),
    ];
    for (name, (c, limits)) in operators {
        assert_eq!(symbols::large_operator(name), Some((c, limits)), "{name}");
        // plain TeX's \intop and \ointop are integrals that take limits: the character alone does not
        if !matches!(name, "intop" | "ointop") {
            assert_eq!(symbols::limits_by_default(c), limits, "{name}");
        }
    }
    assert!(!symbols::limits_by_default('∫'));
    assert!(!symbols::limits_by_default('x'));
    assert_eq!(symbols::large_operator("bigfoo"), None);
}

#[test]
fn delimiters_by_command_and_by_character() {
    let delimiters = [
        ("{", '{'),
        ("lbrace", '{'),
        ("}", '}'),
        ("rbrace", '}'),
        ("langle", '⟨'),
        ("rangle", '⟩'),
        ("lfloor", '⌊'),
        ("rfloor", '⌋'),
        ("lceil", '⌈'),
        ("rceil", '⌉'),
        ("vert", '|'),
        ("lvert", '|'),
        ("rvert", '|'),
        ("mid", '|'),
        ("|", '‖'),
        ("Vert", '‖'),
        ("lVert", '‖'),
        ("rVert", '‖'),
        ("lbrack", '['),
        ("rbrack", ']'),
        ("llbracket", '⟦'),
        ("lBrack", '⟦'),
        ("rrbracket", '⟧'),
        ("rBrack", '⟧'),
        ("uparrow", '↑'),
        ("downarrow", '↓'),
        ("updownarrow", '↕'),
        ("Uparrow", '⇑'),
        ("Downarrow", '⇓'),
        ("Updownarrow", '⇕'),
        ("lgroup", '⟮'),
        ("lparen", '('),
        ("rparen", ')'),
        ("lBrace", '⦃'),
        ("rBrace", '⦄'),
        ("Lbag", '⟅'),
        ("Rbag", '⟆'),
        ("llparenthesis", '⦇'),
        ("rrparenthesis", '⦈'),
        ("llceil", composite::LLCEIL),
        ("rrceil", composite::RRCEIL),
        ("llfloor", composite::LLFLOOR),
        ("rrfloor", composite::RRFLOOR),
        ("lAngle", '⟪'),
        ("rAngle", '⟫'),
        ("rgroup", '⟯'),
        ("lmoustache", '⎰'),
        ("rmoustache", '⎱'),
        ("arrowvert", '⏐'),
        ("Arrowvert", '‖'),
        ("bracevert", '⎪'),
        ("backslash", '\\'),
        ("lang", '⟨'),
        ("rang", '⟩'),
        ("uarr", '↑'),
        ("darr", '↓'),
        ("uArr", '⇑'),
        ("Uarr", '⇑'),
        ("dArr", '⇓'),
        ("Darr", '⇓'),
        ("ulcorner", '⌜'),
        ("urcorner", '⌝'),
        ("llcorner", '⌞'),
        ("lrcorner", '⌟'),
    ];
    for (name, c) in delimiters {
        assert_eq!(symbols::delimiter_command(name), Some(Some(c)), "{name}");
    }
    assert_eq!(symbols::delimiter_command("lfoo"), None);
    for c in ['(', ')', '[', ']', '|', '/', '⟨', '⟩', '‖'] {
        assert_eq!(symbols::delimiter_char(c), Some(Some(c)), "{c}");
    }
    // < and > are the angle brackets; . is the null delimiter
    assert_eq!(symbols::delimiter_char('<'), Some(Some('⟨')));
    assert_eq!(symbols::delimiter_char('>'), Some(Some('⟩')));
    assert_eq!(symbols::delimiter_char('.'), Some(None));
    assert_eq!(symbols::delimiter_char('x'), None);
}

#[test]
fn accents_stretch_when_their_name_says_wide_over_or_under() {
    let accents = [
        ("hat", ('\u{0302}', false)),
        ("widehat", ('\u{0302}', true)),
        ("check", ('\u{030C}', false)),
        ("widecheck", ('\u{030C}', true)),
        ("tilde", ('\u{0303}', false)),
        ("widetilde", ('\u{0303}', true)),
        ("bar", ('\u{0304}', false)),
        ("acute", ('\u{0301}', false)),
        ("grave", ('\u{0300}', false)),
        ("breve", ('\u{0306}', false)),
        ("dot", ('\u{0307}', false)),
        ("ddot", ('\u{0308}', false)),
        ("dddot", ('\u{20DB}', false)),
        ("ddddot", ('\u{20DC}', false)),
        ("mathring", ('\u{030A}', false)),
        ("Tilde", ('\u{0303}', false)),
        ("Hat", ('\u{0302}', false)),
        ("Bar", ('\u{0304}', false)),
        ("Vec", ('\u{20D7}', false)),
        ("Dot", ('\u{0307}', false)),
        ("Ddot", ('\u{0308}', false)),
        ("vec", ('\u{20D7}', false)),
        ("overrightarrow", ('→', true)),
        ("overleftarrow", ('←', true)),
        ("overleftrightarrow", ('↔', true)),
        ("overleftharpoon", ('↼', true)),
        ("overrightharpoon", ('⇀', true)),
        ("overarc", ('⏜', true)),
        ("wideparen", ('⏜', true)),
        ("overparen", ('⏜', true)),
        ("arc", ('⏜', true)),
        ("underparen", ('⏝', true)),
        ("overgroup", ('⏠', true)),
        ("undergroup", ('⏡', true)),
        ("vv", ('→', true)),
        ("Overrightarrow", ('⇒', true)),
        ("overlinesegment", ('\u{20E9}', true)),
        ("widebar", ('\u{0304}', true)),
    ];
    for (name, (c, stretchy)) in accents {
        assert_eq!(symbols::accent(name), Some((c, stretchy)), "{name}");
        let wide = ["wide", "over", "under", "Over"].iter().any(|p| name.starts_with(p)) || matches!(name, "arc" | "vv");
        assert_eq!(stretchy, wide, "{name}");
    }
    assert_eq!(symbols::accent("foo"), None);
}

#[test]
fn text_accents_compose_into_one_character() {
    let marks = [
        ('\'', '\u{0301}'),
        ('`', '\u{0300}'),
        ('^', '\u{0302}'),
        ('~', '\u{0303}'),
        ('"', '\u{0308}'),
        ('c', '\u{0327}'),
        ('=', '\u{0304}'),
        ('.', '\u{0307}'),
        ('u', '\u{0306}'),
        ('v', '\u{030C}'),
        ('H', '\u{030B}'),
        ('r', '\u{030A}'),
        ('k', '\u{0328}'),
        ('b', '\u{0331}'),
        ('d', '\u{0323}'),
        ('t', '\u{0361}'),
    ];
    for (mark, combining) in marks {
        assert_eq!(symbols::text_combining(mark), Some(combining), "{mark}");
    }
    assert_eq!(symbols::text_combining('x'), None);
    // \'i is í, composed on the dotless ı; \c c is ç; \v S is Š
    assert_eq!(symbols::text_accent('\'', 'i'), Some('í'));
    assert_eq!(symbols::text_accent('c', 'c'), Some('ç'));
    assert_eq!(symbols::text_accent('v', 'S'), Some('Š'));
    // a known mark on a letter it does not compose with
    assert_eq!(symbols::text_accent('k', 'x'), None);
}

#[test]
fn text_accents_alone_are_their_spacing_characters() {
    // L-TXT-6: `\^{}` and the other accents with an empty argument
    for (mark, alone) in
        [('\'', '´'), ('`', '`'), ('^', 'ˆ'), ('~', '˜'), ('"', '¨'), ('c', '¸'), ('=', '¯'), ('.', '˙'), ('u', '˘'), ('v', 'ˇ'), ('H', '˝'), ('r', '˚'), ('k', '˛')]
    {
        assert_eq!(symbols::text_spacing(mark), Some(alone), "{mark}");
    }
    for mark in ['b', 'd', 't', 'x'] {
        assert_eq!(symbols::text_spacing(mark), None, "{mark}");
    }
}

#[test]
fn siunitx_units_and_prefixes() {
    let units = [
        ("meter", "m"),
        ("metre", "m"),
        ("second", "s"),
        ("gram", "g"),
        ("kilogram", "kg"),
        ("ampere", "A"),
        ("kelvin", "K"),
        ("mole", "mol"),
        ("candela", "cd"),
        ("newton", "N"),
        ("joule", "J"),
        ("watt", "W"),
        ("pascal", "Pa"),
        ("hertz", "Hz"),
        ("coulomb", "C"),
        ("volt", "V"),
        ("ohm", "Ω"),
        ("farad", "F"),
        ("tesla", "T"),
        ("weber", "Wb"),
        ("henry", "H"),
        ("siemens", "S"),
        ("liter", "L"),
        ("litre", "L"),
        ("hour", "h"),
        ("minute", "min"),
        ("day", "d"),
        ("degreeCelsius", "°C"),
        ("celsius", "°C"),
        ("degree", "°"),
        ("electronvolt", "eV"),
        ("calorie", "cal"),
        ("bar", "bar"),
        ("atm", "atm"),
        ("atmosphere", "atm"),
        ("percent", "%"),
        ("mmHg", "mmHg"),
        ("hectare", "ha"),
        ("tonne", "t"),
        ("angstrom", "Å"),
        ("radian", "rad"),
        ("steradian", "sr"),
        ("lumen", "lm"),
        ("lux", "lx"),
        ("becquerel", "Bq"),
        ("gray", "Gy"),
        ("sievert", "Sv"),
        ("katal", "kat"),
        ("dalton", "Da"),
        ("atomicmassunit", "u"),
        ("astronomicalunit", "au"),
        ("arcminute", "′"),
        ("arcsecond", "″"),
        ("bel", "B"),
        ("neper", "Np"),
        ("barn", "b"),
        ("knot", "kn"),
        ("nauticalmile", "M"),
        ("bit", "bit"),
        ("byte", "B"),
        ("molar", "M"),
        ("torr", "Torr"),
    ];
    for (name, symbol) in units {
        assert_eq!(symbols::unit(name), Some(symbol), "{name}");
    }
    let prefixes = [
        ("yotta", "Y"),
        ("zetta", "Z"),
        ("exa", "E"),
        ("peta", "P"),
        ("tera", "T"),
        ("giga", "G"),
        ("mega", "M"),
        ("kilo", "k"),
        ("hecto", "h"),
        ("deca", "da"),
        ("deka", "da"),
        ("deci", "d"),
        ("centi", "c"),
        ("milli", "m"),
        ("micro", "µ"),
        ("nano", "n"),
        ("pico", "p"),
        ("femto", "f"),
        ("atto", "a"),
        ("zepto", "z"),
        ("yocto", "y"),
    ];
    for (name, symbol) in prefixes {
        assert_eq!(symbols::prefix(name), Some(symbol), "{name}");
    }
    assert_eq!(symbols::unit("parsec"), None);
    assert_eq!(symbols::prefix("ronna"), None);
    // the second spelling is spoken by the first
    for (alias, name) in [("metre", "meter"), ("litre", "liter"), ("celsius", "degreeCelsius"), ("atmosphere", "atm"), ("second", "second")] {
        assert_eq!(symbols::canonical_unit(alias), name);
    }
    assert_eq!(symbols::canonical_prefix("deka"), "deca");
    assert_eq!(symbols::canonical_prefix("kilo"), "kilo");
}

#[test]
fn texvc_macros_are_the_commands_they_expand_to() {
    let macros = [
        ("N", "\\mathbb{N}"),
        ("natnums", "\\mathbb{N}"),
        ("R", "\\mathbb{R}"),
        ("Reals", "\\mathbb{R}"),
        ("reals", "\\mathbb{R}"),
        ("Z", "\\mathbb{Z}"),
        ("cnums", "\\mathbb{C}"),
        ("Complex", "\\mathbb{C}"),
        ("thetasym", "\\vartheta "),
        ("bold", "\\mathbf "),
        ("frak", "\\mathfrak "),
    ];
    for (name, expansion) in macros {
        assert_eq!(symbols::texvc_macro(name), Some(expansion), "{name}");
        // \R x draws and reads as \mathbb{R} x
        assert_eq!(latex(&format!("\\{name} x")).tree, latex(&format!("{expansion} x")).tree, "{name}");
    }
    assert_eq!(symbols::texvc_macro("Q"), None);
}

#[test]
fn unicode_math_alphabets() {
    let alphabets = [
        ("mathbfup", Variant::Bold),
        ("mathsfup", Variant::SansSerif),
        ("symsf", Variant::SansSerif),
        ("mathbfsf", Variant::BoldSansSerif),
        ("mathbfsfup", Variant::BoldSansSerif),
        ("mathsfit", Variant::SansSerifItalic),
        ("mathbfsfit", Variant::SansSerifBoldItalic),
        ("mathbfcal", Variant::BoldScript),
        ("mathbfscr", Variant::BoldScript),
        ("mathbffrak", Variant::BoldFraktur),
        ("symup", Variant::Upright),
        ("symnormal", Variant::Italic),
        ("symcal", Variant::Calligraphic),
        ("symscr", Variant::Calligraphic),
        ("symfrak", Variant::Fraktur),
        ("symtt", Variant::Monospace),
        ("symbb", Variant::DoubleStruck),
        ("mathbbmss", Variant::DoubleStruck),
        ("mathbbmtt", Variant::DoubleStruck),
    ];
    for (name, variant) in alphabets {
        assert_eq!(symbols::unicode_math_alphabet(name), Some(variant), "{name}");
    }
    assert_eq!(symbols::unicode_math_alphabet("mathbf"), None);
}

#[test]
fn characters_typed_directly_take_their_tex_class() {
    use Class::*;
    for (c, symbol, class) in [
        ('+', '+', Bin),
        ('×', '×', Bin),
        ('-', '−', Bin),
        ('–', '−', Bin),
        ('*', '∗', Bin),
        ('/', '/', Ord),
        ('=', '=', Rel),
        ('≪', '≪', Rel),
        ('(', '(', Open),
        ('⟨', '⟨', Open),
        (')', ')', Close),
        ('!', '!', Close),
        (',', ',', Punct),
        (';', ';', Punct),
        ('∞', '∞', Ord),
        ('✗', '✗', Ord),
    ] {
        assert_eq!(symbols::character(c), Some(Symbol { symbol, class }), "{c}");
        assert_eq!(symbols::class_of_char(c), Some(class), "{c}");
    }
    assert_eq!(symbols::character('x'), None);
    // a character only a command gives still has the command's class: ≰ is \nleq, a relation
    assert_eq!(symbols::character('≰'), None);
    assert_eq!(symbols::class_of_char('≰'), Some(Rel));
    assert_eq!(symbols::command("nleq"), Some(Symbol { symbol: '≰', class: Rel }));
    assert_eq!(symbols::command("nosuchcommand"), None);
    assert_eq!(symbols::class_of_char('x'), None);
}
