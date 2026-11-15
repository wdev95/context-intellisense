//! MathML output (spec/mathml.md): the tree of meaning as presentation MathML, in the elements of
//! MathML Core (what Chrome, Firefox and Safari draw), with an `intent` (MathML 4) only where the
//! tree knows something the drawing does not say: `sen` is the sine, `]a, b[` an open interval.
//! What MathML cannot draw (chemfig structures, commutative diagrams, the school long division,
//! crossed-out or framed content) makes the whole equation a `<semantics>` with the closest drawing
//! and the source in an `<annotation>` (S-MML-1).
//!
//! The reader in `input/mathml.rs` reads this output back into the same tree: the element choices
//! here mirror its mapping.

use crate::input::symbols;
use crate::layout::{alphabet, classes};
use crate::locale::Locale;
use crate::tree::*;
use crate::Input;

const NAMESPACE: &str = "http://www.w3.org/1998/Math/MathML";

/// The MathML of an equation, and what MathML could not carry, if anything (S-MML-1).
pub struct MathMl {
    pub text: String,
    /// The first construction that has no MathML drawing: the output is then a `<semantics>` fallback.
    pub fallback: Option<&'static str>,
}

/// The tree as a `<math>` element. `source` and `input` go in the fallback's annotation.
pub fn generate(tree: &Node, locale: Locale, display: bool, source: &str, input: Input) -> MathMl {
    let fallback = needs_fallback(tree);
    let mut w = Writer { out: String::with_capacity(64 + source.len() * 4), locale, display, arg: None };
    w.out.push_str("<math xmlns=\"");
    w.out.push_str(NAMESPACE);
    w.out.push_str(if display { "\" display=\"block\">" } else { "\" display=\"inline\">" });
    if fallback.is_some() {
        w.out.push_str("<semantics><mrow>");
    }
    match tree {
        // a group alone in the equation keeps its <mrow>: the reader reads <math> as a row of its own
        Node::Row { children } if matches!(children.as_slice(), [Node::Row { .. }]) => w.single(tree),
        Node::Row { children } => w.row(children),
        other => w.node(other),
    }
    if fallback.is_some() {
        let encoding = match input {
            Input::Latex => "application/x-tex",
            Input::MathMl => "application/mathml-presentation+xml",
            Input::AsciiMath => "text/x-asciimath",
        };
        w.out.push_str("</mrow><annotation encoding=\"");
        w.out.push_str(encoding);
        w.out.push_str("\">");
        escape_into(&mut w.out, source);
        w.out.push_str("</annotation></semantics>");
    }
    w.out.push_str("</math>");
    MathMl { text: w.out, fallback }
}

/// Why a stmaryrd or mathabx symbol drawn by composition (#176) makes the MathML a fallback.
const COMPOSITE: &str = "a stmaryrd or mathabx symbol drawn by composition";

/// The first node MathML Core has no drawing for. Walks the tree with its own stack, so a deep tree
/// costs heap, not call stack.
fn needs_fallback(tree: &Node) -> Option<&'static str> {
    let mut stack = vec![tree];
    while let Some(n) = stack.pop() {
        let reason = match n {
            #[cfg(feature = "chem")]
            Node::Chemistry { .. } => Some("chemical structure (chemfig)"),
            Node::ArrowDiagram { .. } | Node::VerticalArrow { .. } | Node::Table { shape: TableKind::Diagram, .. } => Some("commutative diagram"),
            Node::LongDivision { .. } => Some("long division"),
            Node::ColumnArithmetic { .. } => Some("column arithmetic"),
            // the Feynman slash of a letter or a symbol is a token with the combining long solidus
            Node::Canceled { shape: Cancel::Slash, base } if slashed_token(base).is_some() => None,
            Node::Canceled { shape: Cancel::Slash, .. } => Some("\\slashed on more than one symbol"),
            Node::Canceled { .. } => Some("crossed-out content (cancel)"),
            Node::Framed { shape: Frame::Fill, border: None, .. } => None,
            Node::Framed { .. } => Some("framed content"),
            Node::Rotated { .. } | Node::Reflected { .. } | Node::Scaled { .. } | Node::Resized { .. } => Some("graphicx box"),
            Node::Choice { .. } => Some("\\mathchoice"),
            // #176: drawn by composition, with no character of its own
            Node::Operator { symbol, .. } | Node::FixedDelimiter { symbol, .. } | Node::LargeOperator { symbol, .. } if crate::composite::is(*symbol) => Some(COMPOSITE),
            Node::Delimited { open, close, .. } if open.is_some_and(crate::composite::is) || close.is_some_and(crate::composite::is) => Some(COMPOSITE),
            _ => None,
        };
        if reason.is_some() {
            return reason;
        }
        each_child(n, &mut stack);
    }
    None
}

/// Pushes the direct children of a node. The nodes `needs_fallback` stops at (diagrams, graphicx
/// boxes) are never walked into, so their children are left out.
fn each_child<'a>(n: &'a Node, out: &mut Vec<&'a Node>) {
    match n {
        Node::Row { children } | Node::Choice { branches: children } | Node::Delimited { parts: children, .. } => out.extend(children.iter()),
        Node::Fraction { num, den, .. } => out.extend([num.as_ref(), den.as_ref()]),
        Node::Root { radicand, index } => {
            out.push(radicand);
            out.extend(index.as_deref());
        }
        Node::Scripts { base, sup, sub } | Node::Stacked { base, above: sup, below: sub } => {
            out.push(base);
            out.extend(sup.as_deref());
            out.extend(sub.as_deref());
        }
        Node::Table { rows, between, .. } => {
            out.extend(rows.iter().flatten());
            out.extend(between.iter().map(|(_, n)| n));
        }
        Node::Brace { base, label, .. } => {
            out.push(base);
            out.extend(label.as_deref());
        }
        Node::Arrow { above: a, below: b, .. } | Node::VerticalArrow { left: a, right: b, .. } => {
            out.extend(a.as_deref());
            out.extend(b.as_deref());
        }
        Node::Text { parts } => out.extend(parts.iter().filter_map(|p| if let TextPart::Math(m) = p { Some(m) } else { None })),
        Node::Quantity { unit: base, .. }
        | Node::Accent { base, .. }
        | Node::Bar { base, .. }
        | Node::Canceled { base, .. }
        | Node::Framed { base, .. }
        | Node::Phantom { base, .. }
        | Node::ForcedClass { base, .. }
        | Node::Style { base, .. }
        | Node::Colored { base, .. }
        | Node::Unit { base, .. }
        | Node::Multicolumn { base, .. }
        | Node::Raised { base, .. }
        | Node::VCenter { base }
        | Node::Smashed { base, .. }
        | Node::Attributed { base, .. }
        | Node::VdotsWithin { symbol: base, .. }
        | Node::Overlapped { base, .. } => out.push(base),
        _ => {}
    }
}

/// The MathML 4 core concept (W3C, "Intent Core Concepts") of a function, by its canonical name.
pub fn function_concept(canonical: &str) -> Option<&'static str> {
    Some(match canonical {
        "sin" => "sine",
        "cos" => "cosine",
        "tan" => "tangent",
        "cot" => "cotangent",
        "sec" => "secant",
        "csc" => "cosecant",
        "arcsin" => "arcsine",
        "arccos" => "arccosine",
        "arctan" => "arctangent",
        "arccot" => "arccotangent",
        "arcsec" => "arcsecant",
        "arccsc" => "arccosecant",
        "sinh" => "hyperbolic-sine",
        "cosh" => "hyperbolic-cosine",
        "tanh" => "hyperbolic-tangent",
        "coth" => "hyperbolic-cotangent",
        "sech" => "hyperbolic-secant",
        "csch" => "hyperbolic-cosecant",
        "log" => "logarithm",
        "ln" => "natural-logarithm",
        "exp" => "exponential",
        "det" => "determinant",
        "gcd" => "greatest-common-divisor",
        "lcm" => "least-common-multiple",
        "max" => "max",
        "min" => "min",
        "lim" => "limit",
        "arg" => "complex-arg",
        _ => return None,
    })
}

/// The spacing MathML gives an operator of each TeX class: `lspace`, `rspace` and `form`, written
/// only when the class is not the one of the character (`{,}` is ordinary, `\mathrel{+}` a relation).
pub fn class_spacing(class: Class) -> (&'static str, &'static str, Option<&'static str>) {
    match class {
        Class::Ord => ("0em", "0em", None),
        Class::Op => ("0.1667em", "0.1667em", None),
        Class::Bin => ("0.2222em", "0.2222em", None),
        Class::Rel => ("0.2778em", "0.2778em", None),
        Class::Open => ("0em", "0em", Some("prefix")),
        Class::Close => ("0em", "0em", Some("postfix")),
        Class::Punct => ("0em", "0.1667em", None),
        Class::Inner => ("0.1667em", "0.1667em", Some("infix")),
    }
}

/// The class the MathML reader gives a lone `<mo>` with this character.
fn default_class(c: char) -> Class {
    symbols::class_of_char(c).unwrap_or(Class::Ord)
}

/// slashed.sty's `\slashed{X}` as one token: the element and its text, X then U+0338 (the combining long
/// solidus overlay, as Unicode writes ≠ as = with it). The text has two characters, so MathML Core does
/// not italicize it: an italic letter is written as its mathematical italic (𝐷̸). `None` when the base
/// is not a single letter or ordinary symbol (S-MML-1).
fn slashed_token(base: &Node) -> Option<(&'static str, String)> {
    let (tag, c) = match base {
        Node::Identifier { name, variant } => {
            let mut chars = name.chars();
            let (Some(c), None) = (chars.next(), chars.next()) else { return None };
            let shown = if *variant == Variant::Upright { c } else { alphabet::character(c, *variant) };
            if shown == c && *variant != Variant::Upright {
                return None;
            }
            ("mi", shown)
        }
        Node::Operator { symbol, class: Class::Ord } if default_class(*symbol) == Class::Ord && !stretches(*symbol) => ("mo", *symbol),
        _ => return None,
    };
    Some((tag, format!("{c}\u{0338}")))
}

/// Characters MathML stretches on its own as fences, in an `<mrow>`: outside `\left…\right` they are
/// written `stretchy="false"`, as TeX keeps a plain parenthesis its size.
fn stretches(c: char) -> bool {
    matches!(c, '(' | ')' | '[' | ']' | '{' | '}' | '|' | '‖' | '⟨' | '⟩' | '⌈' | '⌉' | '⌊' | '⌋' | '⟦' | '⟧' | '/' | '\\')
}

/// The MathML name of a variant, for the letters Unicode has no character for.
fn variant_name(v: Variant) -> &'static str {
    match v {
        Variant::Italic => "italic",
        Variant::Upright => "normal",
        Variant::Bold => "bold",
        Variant::BoldItalic => "bold-italic",
        Variant::DoubleStruck => "double-struck",
        Variant::Calligraphic => "script",
        Variant::Fraktur => "fraktur",
        Variant::SansSerif => "sans-serif",
        Variant::Monospace => "monospace",
        Variant::BoldScript => "bold-script",
        Variant::BoldFraktur => "bold-fraktur",
        Variant::BoldSansSerif => "bold-sans-serif",
        Variant::SansSerifItalic => "sans-serif-italic",
        Variant::SansSerifBoldItalic => "sans-serif-bold-italic",
    }
}

fn escape_into(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' | '\n' | '\r' => out.push(c),
            // XML 1.0 cannot represent these, not even escaped
            '\u{0}'..='\u{1F}' | '\u{FFFE}' | '\u{FFFF}' => out.push('\u{FFFD}'),
            other => out.push(other),
        }
    }
}

/// A length in em, as the reader measures it back: the shortest form that gives the same `f32`.
fn em(v: f32) -> String {
    format!("{v}em")
}

struct Writer {
    out: String,
    locale: Locale,
    display: bool,
    /// An `arg` attribute for the next element written (the argument of an `intent`).
    arg: Option<&'static str>,
}

impl Writer {
    /// `<tag` with the pending `arg`; the caller adds attributes and closes it with `>` (`gt`).
    fn start(&mut self, tag: &str) {
        self.out.push('<');
        self.out.push_str(tag);
        if let Some(a) = self.arg.take() {
            self.attr("arg", a);
        }
    }

    fn attr(&mut self, name: &str, value: &str) {
        self.out.push(' ');
        self.out.push_str(name);
        self.out.push_str("=\"");
        escape_into(&mut self.out, value);
        self.out.push('"');
    }

    fn gt(&mut self) {
        self.out.push('>');
    }

    fn end(&mut self, tag: &str) {
        self.out.push_str("</");
        self.out.push_str(tag);
        self.out.push('>');
    }

    fn open(&mut self, tag: &str) {
        self.start(tag);
        self.gt();
    }

    fn text(&mut self, t: &str) {
        escape_into(&mut self.out, t);
    }

    fn leaf(&mut self, tag: &str, t: &str) {
        self.open(tag);
        self.text(t);
        self.end(tag);
    }

    fn char_leaf(&mut self, tag: &str, c: char) {
        let mut b = [0; 4];
        self.leaf(tag, c.encode_utf8(&mut b));
    }

    /// A node that must be a single element (an argument of `<mfrac>`, `<msup>`…): a row is an `<mrow>`.
    fn single(&mut self, n: &Node) {
        match n {
            Node::Row { children } => {
                self.open("mrow");
                self.row(children);
                self.end("mrow");
            }
            other => self.node(other),
        }
    }

    /// The content of an element with an inferred row (`<msqrt>`, `<mtd>`, `<mstyle>`…). A row of one
    /// node keeps its `<mrow>`: the reader takes a lone element for itself.
    fn inline(&mut self, n: &Node) {
        match n {
            Node::Row { children } if children.len() != 1 => self.row(children),
            other => self.single(other),
        }
    }

    fn node(&mut self, n: &Node) {
        // a chain of crossed-out content (`\not\not…`) draws its base, in a loop rather than a recursion
        if matches!(n, Node::Canceled { shape: Cancel::Slash, .. }) && self.slashed(n) {
            return;
        }
        let mut n = n;
        loop {
            match n {
                Node::Canceled { base, .. }
                | Node::Rotated { base, .. }
                | Node::Reflected { base }
                | Node::Scaled { base, .. }
                | Node::Resized { base, .. }
                | Node::VCenter { base } => {
                    n = base;
                    continue;
                }
                Node::Number { value, group, thousands, variant } => self.number(value, (*group, *thousands), *variant),
                Node::Identifier { name, variant } => self.identifier(name, *variant),
                Node::Function { name, limits } => self.function(name, *limits, false),
                Node::Operator { symbol, class } => self.operator(*symbol, *class),
                Node::FixedDelimiter { symbol, class, size } => self.fixed_delimiter(*symbol, *class, *size),
                Node::LargeOperator { symbol, limits } => self.large_operator(*symbol, *limits),
                Node::Row { children } => self.single_row(children),
                Node::Fraction { .. } => self.fraction(n),
                Node::Root { radicand, index } => self.root(radicand, index.as_deref()),
                Node::Scripts { base, sup, sub } => self.scripts(base, sup.as_deref(), sub.as_deref()),
                Node::Delimited { open, close, parts, middles } => self.delimited(*open, *close, parts, middles),
                Node::Table { .. } => self.table(n),
                Node::Accent { base, accent, stretchy, below } => self.accent(base, *accent, *stretchy, *below),
                Node::Bar { base, above } => self.under_over(base, if *above { '‾' } else { '_' }, *above, true),
                Node::Brace { base, above, label, bracket } => self.brace(base, *above, label.as_deref(), *bracket),
                Node::Text { parts } => self.text_node(parts),
                Node::Space { em: e } => self.space(*e),
                Node::Framed { base, background, padding, .. } => self.paint(base, None, background.as_deref(), *padding),
                Node::Colored { color, base } => self.paint(base, Some(color), None, None),
                Node::Arrow { symbol, above, below } => self.arrow(*symbol, above.as_deref(), below.as_deref()),
                Node::Stacked { base, above, below } => self.stacked(base, above.as_deref(), below.as_deref()),
                Node::Phantom { base, width, height } => self.phantom(base, *width, *height),
                Node::ForcedClass { base, .. } | Node::Quantity { unit: base, .. } | Node::Multicolumn { base, .. } => self.node(base),
                Node::Attributed { attributes, base } => self.attributed(attributes, base),
                Node::VdotsWithin { symbol, short } => self.vdots_within(symbol, *short),
                Node::Style { level, base } => self.style(*level, base),
                Node::Unit { base, .. } => self.unit(base),
                Node::Raised { em: e, base, .. } => self.padded(base, &[("voffset", em(*e))]),
                Node::Smashed { base, height, depth } => self.smashed(base, *height, *depth),
                Node::Overlapped { base, side } => self.overlapped(base, *side),
                Node::Rule { width, height, raise } => self.rule(*width, *height, *raise),
                Node::Choice { branches } => self.choice(branches),
                Node::VerticalArrow { symbol, left, right } => self.vertical_arrow(*symbol, left.as_deref(), right.as_deref()),
                Node::ArrowDiagram { cells, .. } => self.cells(cells),
                #[cfg(feature = "chem")]
                Node::Chemistry { scheme } => self.chemistry(scheme),
                Node::LongDivision { dividend, divisor } => self.long_division(dividend, divisor),
                Node::ColumnArithmetic { operation, terms } => self.column_arithmetic(*operation, terms),
            }
            break;
        }
    }

    /// `\slashed{X}` as one token (`slashed_token`); false when it has none, and the fallback draws X.
    #[inline(never)]
    fn slashed(&mut self, n: &Node) -> bool {
        let Node::Canceled { base, .. } = n else { return false };
        let Some((tag, text)) = slashed_token(base) else { return false };
        self.leaf(tag, &text);
        true
    }

    #[inline(never)]
    fn single_row(&mut self, children: &[Node]) {
        self.open("mrow");
        self.row(children);
        self.end("mrow");
    }

    /// The children of a row, with function application after a function and an `intent` on the
    /// intervals of L-INT-1.
    #[inline(never)]
    fn row(&mut self, children: &[Node]) {
        let intervals = if self.locale.open_interval() && children.iter().any(|c| matches!(c, Node::Operator { symbol: ']' | '[', .. })) {
            intervals(children, self.locale)
        } else {
            Vec::new()
        };
        let mut i = 0;
        while i < children.len() {
            if let Some(&(_, sep, j, concept)) = intervals.iter().find(|iv| iv.0 == i) {
                self.interval(&children[i..=j], sep - i, concept);
                i = j + 1;
                continue;
            }
            self.node(&children[i]);
            if applies(&children[i]) {
                if let Some(next) = children.get(i + 1).filter(|n| takes_argument(n)) {
                    // TeX's thin space between a function and its argument, unless a parenthesis follows
                    self.out.push_str(if opens(next) { "<mo>\u{2061}</mo>" } else { "<mo lspace=\"0em\" rspace=\"0.1667em\">\u{2061}</mo>" });
                }
            }
            i += 1;
        }
    }

    /// `]a, b[`: an `<mrow>` with the interval concept, its bounds as the arguments.
    #[inline(never)]
    fn interval(&mut self, span: &[Node], sep: usize, concept: &'static str) {
        self.start("mrow");
        self.attr("intent", &format!("{concept}($a,$b)"));
        self.gt();
        self.node(&span[0]);
        self.bound(&span[1..sep], "a");
        self.node(&span[sep]);
        self.bound(&span[sep + 1..span.len() - 1], "b");
        self.node(&span[span.len() - 1]);
        self.end("mrow");
    }

    fn bound(&mut self, nodes: &[Node], name: &'static str) {
        if let [one] = nodes {
            self.arg = Some(name);
            self.node(one);
        } else {
            self.start("mrow");
            self.attr("arg", name);
            self.gt();
            self.row(nodes);
            self.end("mrow");
        }
    }

    #[inline(never)]
    fn number(&mut self, value: &str, (group, thousands): (bool, Option<char>), variant: Variant) {
        // #203: a digit in another alphabet is written as its Unicode character (𝟏), as a letter is
        let written: String = super::written_number(value, thousands, self.locale).chars().map(|c| alphabet::character(c, variant)).collect();
        if group && thousands.is_none() {
            // siunitx groups (L-UNI-1): a thin space every three digits of the integer part, from five on
            let (int, rest) = written.split_at(written.find(|c: char| !c.is_numeric()).unwrap_or(written.len()));
            let n = int.chars().count();
            let mut s = String::with_capacity(written.len() + 8);
            for (i, c) in int.chars().enumerate() {
                if n >= 5 && i > 0 && (n - i) % 3 == 0 {
                    s.push('\u{2009}');
                }
                s.push(c);
            }
            s.push_str(rest);
            self.leaf("mn", &s);
        } else {
            self.leaf("mn", &written);
        }
    }

    #[inline(never)]
    fn identifier(&mut self, name: &str, variant: Variant) {
        let mut chars = name.chars();
        let (Some(c), rest) = (chars.next(), chars.next()) else {
            return self.leaf("mi", "");
        };
        if rest.is_none() {
            // a single letter: MathML Core sets it in italic on its own, as TeX does (L-LET-1)
            let natural = if c.is_ascii_alphabetic() || ('α'..='ω').contains(&c) || c == 'ı' || c == 'ȷ' { Variant::Italic } else { Variant::Upright };
            let italicized = alphabet::character(c, Variant::Italic) != c;
            if variant == natural && (variant == Variant::Italic || !italicized) {
                return self.char_leaf("mi", c);
            }
        }
        // the variant as Unicode's mathematical letters (𝐱, ℝ), which MathML Core draws; upright, and
        // a letter without one, keep the mathvariant attribute
        if variant != Variant::Upright {
            let mapped: String = name.chars().map(|c| alphabet::character(c, variant)).collect();
            if mapped.chars().zip(name.chars()).all(|(m, c)| m != c) {
                return self.leaf("mi", &mapped);
            }
        }
        self.start("mi");
        self.attr("mathvariant", variant_name(variant));
        self.gt();
        if variant == Variant::Upright && rest.is_none() {
            self.char_leaf_text(c);
        } else {
            self.text(name);
        }
        self.end("mi");
    }

    fn char_leaf_text(&mut self, c: char) {
        // #176: a symbol drawn by composition, as its closest text (the equation is a fallback)
        if let Some(k) = crate::composite::get(c) {
            self.text(k.text);
            return;
        }
        let mut b = [0; 4];
        self.text(c.encode_utf8(&mut b));
    }

    /// A function name as the language writes it (L-FUN-1). When that is not the canonical name
    /// (`sen` for the sine), an `intent` says which function it is.
    #[inline(never)]
    fn function(&mut self, name: &str, limits: bool, scripted: bool) {
        let written = self.locale.function_name(name);
        // with limits under it (lim, max), an operator, so the limits move aside in inline style
        let tag = if scripted && limits { "mo" } else { "mi" };
        self.start(tag);
        if written != name {
            self.attr("intent", function_concept(name).unwrap_or(name));
        }
        if tag == "mo" {
            self.attr("movablelimits", "true");
        }
        self.gt();
        self.text(&written);
        self.end(tag);
    }

    #[inline(never)]
    fn operator(&mut self, symbol: char, class: Class) {
        self.start("mo");
        if class != default_class(symbol) {
            let (l, r, form) = class_spacing(class);
            if let Some(f) = form {
                self.attr("form", f);
            }
            self.attr("lspace", l);
            self.attr("rspace", r);
        }
        if stretches(symbol) {
            self.attr("stretchy", "false");
        }
        self.gt();
        self.char_leaf_text(symbol);
        self.end("mo");
    }

    #[inline(never)]
    fn fixed_delimiter(&mut self, symbol: char, class: Class, size: u8) {
        let total = [1.2f32, 1.8, 2.4, 3.0][(size as usize).clamp(1, 4) - 1];
        self.start("mo");
        if class != default_class(symbol) {
            let (l, r, form) = class_spacing(class);
            if let Some(f) = form {
                self.attr("form", f);
            }
            self.attr("lspace", l);
            self.attr("rspace", r);
        }
        self.attr("stretchy", "true");
        self.attr("symmetric", "true");
        self.attr("minsize", &em(total));
        self.attr("maxsize", &em(total));
        self.gt();
        self.char_leaf_text(symbol);
        self.end("mo");
    }

    #[inline(never)]
    fn large_operator(&mut self, symbol: char, limits: Option<bool>) {
        self.start("mo");
        if limits == Some(true) {
            self.attr("movablelimits", "false");
        }
        self.gt();
        self.char_leaf_text(symbol);
        self.end("mo");
    }

    #[inline(never)]
    fn fraction(&mut self, fraction: &Node) {
        let Node::Fraction { num, den, style, rule, thickness, numerator_align } = fraction else { unreachable!("a fraction") };
        let (style, rule, thickness) = (*style, *rule, *thickness);
        self.start("mfrac");
        // \cfrac[l], \cfrac[r]: MathML's numalign, as MathJax writes it
        match numerator_align {
            Some('l') => self.attr("numalign", "left"),
            Some('r') => self.attr("numalign", "right"),
            _ => {}
        }
        if !rule {
            self.attr("linethickness", "0");
        } else if let Some(t) = thickness {
            self.attr("linethickness", &em(t));
        }
        match style {
            FractionStyle::Display => self.attr("displaystyle", "true"),
            FractionStyle::Row => self.attr("displaystyle", "false"),
            FractionStyle::Auto => {}
        }
        self.gt();
        self.single(num);
        self.single(den);
        self.end("mfrac");
    }

    #[inline(never)]
    fn root(&mut self, radicand: &Node, index: Option<&Node>) {
        match index {
            None => {
                self.open("msqrt");
                self.inline(radicand);
                self.end("msqrt");
            }
            Some(index) => {
                self.open("mroot");
                self.single(radicand);
                self.single(index);
                self.end("mroot");
            }
        }
    }

    #[inline(never)]
    fn scripts(&mut self, base: &Node, sup: Option<&Node>, sub: Option<&Node>) {
        // limits under and over: a large operator that takes them (∑ by default, ∫ with \limits), lim
        let under = match base.operator_core() {
            Node::LargeOperator { symbol, limits } => limits.unwrap_or_else(|| symbols::limits_by_default(*symbol)),
            Node::Function { limits, .. } => *limits,
            _ => false,
        };
        let tag = match (under, sup.is_some(), sub.is_some()) {
            (true, true, true) => "munderover",
            (true, true, false) => "mover",
            (true, false, _) => "munder",
            (false, true, true) => "msubsup",
            (false, true, false) => "msup",
            (false, false, _) => "msub",
        };
        self.open(tag);
        match base {
            Node::Function { name, limits } => self.function(name, *limits, true),
            other => self.single(other),
        }
        if let Some(s) = sub {
            self.single(s);
        }
        if let Some(s) = sup {
            self.single(s);
        }
        if sub.is_none() && sup.is_none() {
            self.leaf("mrow", "");
        }
        self.end(tag);
    }

    /// A delimiter of `\left…\right`: one MathML stretches on its own, or one asked to; an empty
    /// `<mo>` is the null delimiter `.`.
    fn fence(&mut self, c: Option<char>, form: &'static str) {
        self.start("mo");
        match c {
            None => {
                self.attr("fence", "true");
                self.attr("form", form);
                self.gt();
            }
            Some(c) => {
                // a fence the reader takes as one by its class; a reversed bracket (\left] … \right[) says so
                let natural = default_class(c) == if form == "prefix" { Class::Open } else { Class::Close } || matches!(c, '|' | '‖');
                if !natural {
                    self.attr("fence", "true");
                    self.attr("form", form);
                    self.attr("stretchy", "true");
                }
                self.gt();
                self.char_leaf_text(c);
            }
        }
        self.end("mo");
    }

    #[inline(never)]
    fn delimited(&mut self, open: Option<char>, close: Option<char>, parts: &[Node], middles: &[char]) {
        // \binom: (n over k) without a rule is the binomial coefficient
        let binomial = matches!((open, close, parts), (Some('('), Some(')'), [Node::Fraction { rule: false, thickness: None, .. }]));
        self.start("mrow");
        if binomial {
            self.attr("intent", "binomial-coefficient($n,$k)");
        }
        self.gt();
        self.fence(open, "prefix");
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                self.start("mo");
                self.attr("stretchy", "true");
                self.gt();
                self.char_leaf_text(middles.get(i - 1).copied().unwrap_or('|'));
                self.end("mo");
            }
            match part {
                Node::Fraction { num, den, style, .. } if binomial => {
                    self.start("mfrac");
                    self.attr("linethickness", "0");
                    match style {
                        FractionStyle::Display => self.attr("displaystyle", "true"),
                        FractionStyle::Row => self.attr("displaystyle", "false"),
                        FractionStyle::Auto => {}
                    }
                    self.gt();
                    self.arg = Some("n");
                    self.single(num);
                    self.arg = Some("k");
                    self.single(den);
                    self.end("mfrac");
                }
                // each part is the row between two delimiters
                Node::Row { children } => self.row(children),
                other => self.node(other),
            }
        }
        self.fence(close, "postfix");
        self.end("mrow");
    }

    #[inline(never)]
    fn table(&mut self, n: &Node) {
        let Node::Table { shape, rows, columns, verticals, horizontals, .. } = n else { return };
        let ncol = rows.iter().map(|r| r.iter().map(|c| if let Node::Multicolumn { columns, .. } = c { *columns } else { 1 }).sum::<usize>()).max().unwrap_or(0);
        let (open, close) = match shape {
            TableKind::PMatrix => (Some('('), Some(')')),
            TableKind::BMatrix => (Some('['), Some(']')),
            TableKind::BraceMatrix => (Some('{'), Some('}')),
            TableKind::VMatrix => (Some('|'), Some('|')),
            TableKind::VVMatrix => (Some('‖'), Some('‖')),
            TableKind::Cases => (Some('{'), None),
            TableKind::RCases => (None, Some('}')),
            _ => (None, None),
        };
        let fenced = open.is_some() || close.is_some();
        if fenced {
            self.open("mrow");
            if let Some(o) = open {
                self.char_leaf("mo", o);
            }
        }
        self.start("mtable");
        let align = |c: char| match c {
            'l' => "left",
            'r' => "right",
            _ => "center",
        };
        let column_list = |f: &dyn Fn(usize) -> &'static str| (0..ncol.max(1)).map(f).collect::<Vec<_>>().join(" ");
        match shape {
            TableKind::Cases | TableKind::RCases => {
                let spec = column_list(&|j| align(columns.get(j).copied().unwrap_or('l')));
                self.attr("columnalign", &spec);
            }
            TableKind::Aligned => {
                self.attr("displaystyle", "true");
                self.attr("columnalign", &column_list(&|j| if j.is_multiple_of(2) { "right" } else { "left" }));
                self.attr("columnspacing", &column_list(&|j| if j.is_multiple_of(2) { "0em" } else { "2em" }));
            }
            TableKind::Centered => self.attr("displaystyle", "true"),
            TableKind::SmallMatrix => self.attr("scriptlevel", "1"),
            TableKind::Substack => self.attr("displaystyle", "false"),
            TableKind::Array => {
                let spec = column_list(&|j| align(columns.get(j).copied().unwrap_or('c')));
                self.attr("columnalign", &spec);
            }
            _ if !columns.is_empty() => {
                let spec = column_list(&|j| align(columns.get(j).copied().unwrap_or('c')));
                self.attr("columnalign", &spec);
            }
            _ => {}
        }
        if ncol > 1 && verticals.iter().any(|&v| v > 0 && v < ncol) {
            let spec = (1..ncol).map(|j| if verticals.contains(&j) { "solid" } else { "none" }).collect::<Vec<_>>().join(" ");
            self.attr("columnlines", &spec);
        }
        if rows.len() > 1 && horizontals.iter().any(|h| h.row > 0 && h.row < rows.len()) {
            let spec = (1..rows.len()).map(|r| if horizontals.iter().any(|h| h.row == r) { "solid" } else { "none" }).collect::<Vec<_>>().join(" ");
            self.attr("rowlines", &spec);
        }
        self.gt();
        // the alignment of each column, also as CSS for the browsers that ignore columnalign
        let column = |j: usize| match shape {
            TableKind::Cases | TableKind::RCases => columns.get(j).copied().unwrap_or('l'),
            TableKind::Aligned => {
                if j.is_multiple_of(2) {
                    'r'
                } else {
                    'l'
                }
            }
            _ => columns.get(j).copied().unwrap_or('c'),
        };
        for row in rows {
            self.open("mtr");
            let mut j = 0;
            for cell in row {
                self.cell(cell, column(j));
                j += if let Node::Multicolumn { columns, .. } = cell { *columns } else { 1 };
            }
            self.end("mtr");
        }
        self.end("mtable");
        if fenced {
            if let Some(c) = close {
                self.char_leaf("mo", c);
            }
            self.end("mrow");
        }
    }

    fn cell(&mut self, cell: &Node, align: char) {
        self.start("mtd");
        let align = if let Node::Multicolumn { alignment, .. } = cell { *alignment } else { align };
        match align {
            'l' => self.attr("style", "text-align: left"),
            'r' => self.attr("style", "text-align: right"),
            _ => {}
        }
        let content = match cell {
            Node::Multicolumn { columns, alignment, base } => {
                self.attr("columnspan", &columns.to_string());
                self.attr(
                    "columnalign",
                    match alignment {
                        'l' => "left",
                        'r' => "right",
                        _ => "center",
                    },
                );
                base.as_ref()
            }
            other => other,
        };
        self.gt();
        self.inline(content);
        self.end("mtd");
    }

    #[inline(never)]
    fn accent(&mut self, base: &Node, accent: char, stretchy: bool, below: bool) {
        // the mark as the reader knows it: spacing characters, not the combining ones
        let (mark, is_accent) = match accent {
            // under the base, the reader takes the character as it is (\utilde: U+0303)
            _ if below && !matches!(accent, '←' | '→' | '↔' | '⏝') => (accent, true),
            '\u{20D7}' => ('→', true),
            '→' | '←' | '↔' => (accent, false),
            '\u{0302}' => ('ˆ', !stretchy),
            '\u{0303}' => ('˜', !stretchy),
            '\u{030C}' => ('ˇ', true),
            '\u{0307}' => ('˙', true),
            '\u{0308}' => ('¨', true),
            '\u{0301}' => ('´', true),
            '\u{0300}' => ('`', true),
            '\u{0306}' => ('˘', true),
            '\u{030A}' => ('˚', true),
            '\u{0304}' => ('¯', true),
            other => (other, true),
        };
        let tag = if below { "munder" } else { "mover" };
        self.start(tag);
        self.attr(if below { "accentunder" } else { "accent" }, if is_accent { "true" } else { "false" });
        self.gt();
        self.single(base);
        self.start("mo");
        self.attr("stretchy", if stretchy { "true" } else { "false" });
        self.gt();
        self.char_leaf_text(mark);
        self.end("mo");
        self.end(tag);
    }

    #[inline(never)]
    fn under_over(&mut self, base: &Node, mark: char, above: bool, accent: bool) {
        let tag = if above { "mover" } else { "munder" };
        self.start(tag);
        if accent {
            self.attr(if above { "accent" } else { "accentunder" }, "true");
        }
        self.gt();
        self.single(base);
        self.char_leaf("mo", mark);
        self.end(tag);
    }

    #[inline(never)]
    fn brace(&mut self, base: &Node, above: bool, label: Option<&Node>, bracket: bool) {
        let mark = match (above, bracket) {
            (true, false) => '⏞',
            (false, false) => '⏟',
            (true, true) => '⎴',
            (false, true) => '⎵',
        };
        let tag = if above { "mover" } else { "munder" };
        if label.is_some() {
            self.open(tag);
        }
        self.under_over(base, mark, above, false);
        if let Some(l) = label {
            self.single(l);
            self.end(tag);
        }
    }

    #[inline(never)]
    fn text_node(&mut self, parts: &[TextPart]) {
        // MathML ignores the white space at the ends of <mtext>: no-break spaces keep it
        let math = parts.iter().any(|p| matches!(p, TextPart::Math(_)));
        if math {
            self.open("mrow");
        }
        for p in parts {
            match p {
                TextPart::Text(t) => {
                    self.open("mtext");
                    let kept: String = t.chars().map(|c| if c == ' ' { '\u{A0}' } else { c }).collect();
                    self.text(&kept);
                    self.end("mtext");
                }
                TextPart::Math(m) => self.node(m),
            }
        }
        if math {
            self.end("mrow");
        }
        if parts.is_empty() {
            self.leaf("mtext", "");
        }
    }

    fn space(&mut self, e: f32) {
        self.start("mspace");
        self.attr("width", &em(e));
        self.out.push_str("/>");
    }

    #[inline(never)]
    fn paint(&mut self, base: &Node, color: Option<&str>, background: Option<&str>, padding: Option<f32>) {
        self.start("mrow");
        if let Some(c) = color {
            self.attr("mathcolor", c);
        }
        if let Some(b) = background {
            self.attr("mathbackground", b);
        }
        // \bbox[yellow,5px]: the space around the content, as CSS (MathML Core boxes take padding)
        if let Some(p) = padding {
            self.attr("style", &format!("padding: {}", em(p)));
        }
        self.gt();
        self.inline(base);
        self.end("mrow");
    }

    #[inline(never)]
    fn arrow(&mut self, symbol: char, above: Option<&Node>, below: Option<&Node>) {
        let tag = match (above.is_some(), below.is_some()) {
            (true, true) => "munderover",
            (true, false) => "mover",
            (false, true) => "munder",
            (false, false) => "mrow",
        };
        self.open(tag);
        self.start("mo");
        self.attr("stretchy", "true");
        self.gt();
        self.char_leaf_text(symbol);
        self.end("mo");
        if let Some(b) = below {
            self.single(b);
        }
        if let Some(a) = above {
            self.single(a);
        }
        self.end(tag);
    }

    #[inline(never)]
    fn stacked(&mut self, base: &Node, above: Option<&Node>, below: Option<&Node>) {
        let tag = match (above.is_some(), below.is_some()) {
            (true, true) => "munderover",
            (true, false) => "mover",
            (false, true) => "munder",
            (false, false) => "mrow",
        };
        self.open(tag);
        self.single(base);
        if let Some(b) = below {
            self.single(b);
        }
        if let Some(a) = above {
            self.single(a);
        }
        self.end(tag);
    }

    #[inline(never)]
    fn phantom(&mut self, base: &Node, width: bool, height: bool) {
        let padded = width != height;
        if padded {
            self.start("mpadded");
            if width {
                self.attr("height", "0em");
                self.attr("depth", "0em");
            } else {
                self.attr("width", "0em");
            }
            self.gt();
        }
        self.open("mphantom");
        self.inline(base);
        self.end("mphantom");
        if padded {
            self.end("mpadded");
        }
    }

    /// `\class`, `\cssId`, `\style`, `\htmlData`: MathML Core's global attributes on an `<mrow>`. The
    /// tree holds them already sanitized; a name that is not an HTML attribute name is left out.
    #[inline(never)]
    fn attributed(&mut self, attributes: &[(String, String)], base: &Node) {
        self.start("mrow");
        for (name, value) in attributes {
            let known = matches!(name.as_str(), "class" | "id" | "style") || name.strip_prefix("data-").is_some_and(|k| !k.is_empty());
            if known && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                self.attr(name, value);
            }
        }
        self.gt();
        self.inline(base);
        self.end("mrow");
    }

    /// `\vdotswithin{=}`: ⋮ centered in the width of the relation, as a one-column table over a phantom
    /// of it with no height; `\shortvdotswithin` without space between the rows.
    #[inline(never)]
    fn vdots_within(&mut self, symbol: &Node, short: bool) {
        self.start("mtable");
        if short {
            self.attr("rowspacing", "0em");
        }
        self.gt();
        self.out.push_str("<mtr><mtd><mo>\u{22EE}</mo></mtd></mtr><mtr><mtd>");
        self.phantom(symbol, true, false);
        self.out.push_str("</mtd></mtr></mtable>");
    }

    #[inline(never)]
    fn style(&mut self, level: u8, base: &Node) {
        self.start("mstyle");
        self.attr("displaystyle", if level == 0 { "true" } else { "false" });
        if level >= 2 {
            self.attr("scriptlevel", if level == 2 { "1" } else { "2" });
        }
        self.gt();
        self.inline(base);
        self.end("mstyle");
    }

    #[inline(never)]
    fn unit(&mut self, base: &Node) {
        // MathML 4's `unit` property: read as a unit, not as a variable
        match base {
            Node::Text { parts } if parts.len() == 1 => {
                if let TextPart::Text(t) = &parts[0] {
                    self.start("mi");
                    self.attr("mathvariant", "normal");
                    self.attr("intent", ":unit");
                    self.gt();
                    self.text(t);
                    self.end("mi");
                }
            }
            other => {
                self.start("mrow");
                self.attr("intent", ":unit");
                self.gt();
                self.inline(other);
                self.end("mrow");
            }
        }
    }

    #[inline(never)]
    fn padded(&mut self, base: &Node, attrs: &[(&str, String)]) {
        self.start("mpadded");
        for (k, v) in attrs {
            self.attr(k, v);
        }
        self.gt();
        self.inline(base);
        self.end("mpadded");
    }

    fn smashed(&mut self, base: &Node, height: bool, depth: bool) {
        let mut attrs = Vec::new();
        if height {
            attrs.push(("height", "0em".to_string()));
        }
        if depth {
            attrs.push(("depth", "0em".to_string()));
        }
        self.padded(base, &attrs);
    }

    fn overlapped(&mut self, base: &Node, side: char) {
        let mut attrs = vec![("width", "0em".to_string())];
        match side {
            'l' => attrs.push(("lspace", "-100%".to_string())),
            'c' => attrs.push(("lspace", "-50%".to_string())),
            _ => {}
        }
        self.padded(base, &attrs);
    }

    fn rule(&mut self, width: f32, height: f32, raise: f32) {
        self.start("mspace");
        self.attr("width", &em(width));
        self.attr("height", &em((height + raise).max(0.0)));
        self.attr("depth", &em((-raise).max(0.0)));
        self.attr("mathbackground", "currentColor");
        self.out.push_str("/>");
    }

    #[inline(never)]
    fn choice(&mut self, branches: &[Node]) {
        let pick = if self.display { 0 } else { 1 };
        if let Some(b) = branches.get(pick).or(branches.first()) {
            self.node(b);
        }
    }

    #[inline(never)]
    fn vertical_arrow(&mut self, symbol: char, left: Option<&Node>, right: Option<&Node>) {
        self.open("mrow");
        if let Some(l) = left {
            self.node(l);
        }
        self.char_leaf("mo", symbol);
        if let Some(r) = right {
            self.node(r);
        }
        self.end("mrow");
    }

    #[inline(never)]
    fn cells(&mut self, cells: &[Vec<Node>]) {
        self.open("mtable");
        for row in cells {
            self.open("mtr");
            for c in row {
                self.cell(c, 'c');
            }
            self.end("mtr");
        }
        self.end("mtable");
    }

    #[cfg(feature = "chem")]
    fn chemistry(&mut self, scheme: &crate::chem::Scheme) {
        let text = crate::chem::speech::linear(scheme, self.locale);
        self.leaf("mtext", &text);
    }

    fn long_division(&mut self, dividend: &str, divisor: &str) {
        self.open("mrow");
        self.leaf("mn", dividend);
        self.char_leaf("mo", '÷');
        self.leaf("mn", divisor);
        self.end("mrow");
    }

    fn column_arithmetic(&mut self, operation: char, terms: &[String]) {
        self.open("mrow");
        for (i, t) in terms.iter().enumerate() {
            if i > 0 {
                self.char_leaf("mo", operation);
            }
            self.leaf("mn", t);
        }
        self.end("mrow");
    }
}

/// A function, or a function with scripts (`\sin^2`), that takes the next node as its argument.
fn applies(n: &Node) -> bool {
    match n {
        Node::Function { .. } => true,
        Node::Scripts { base, .. } => matches!(base.operator_core(), Node::Function { .. }),
        _ => false,
    }
}

/// What follows a function is its argument, unless it is a relation, an operation or punctuation.
fn takes_argument(n: &Node) -> bool {
    !matches!(n, Node::Operator { class: Class::Rel | Class::Bin | Class::Punct | Class::Close, .. } | Node::Space { .. } | Node::Phantom { .. })
}

/// A node that starts with an opening delimiter: `\sin(x)` takes no space before it.
fn opens(n: &Node) -> bool {
    matches!(n, Node::Operator { class: Class::Open, .. } | Node::FixedDelimiter { class: Class::Open, .. } | Node::Delimited { .. })
        || matches!(n, Node::Table { shape: TableKind::PMatrix | TableKind::BMatrix | TableKind::BraceMatrix | TableKind::VMatrix | TableKind::VVMatrix, .. })
}

/// The intervals of L-INT-1 in a row: (opening index, separator index, closing index, concept).
fn intervals(children: &[Node], locale: Locale) -> Vec<(usize, usize, usize, &'static str)> {
    let cl = classes(children, locale, None, false);
    let bracket = |i: usize| match &children[i] {
        Node::Operator { symbol: s @ (']' | '['), .. } => Some(*s),
        _ => None,
    };
    let mut found = Vec::new();
    let mut i = 0;
    while i < children.len() {
        let (Some(o), Some(Class::Open)) = (bracket(i), cl[i]) else {
            i += 1;
            continue;
        };
        let mut sep = None;
        let mut end = None;
        for k in i + 1..children.len() {
            match cl[k] {
                Some(Class::Open) => break,
                Some(Class::Close) => {
                    end = Some(k);
                    break;
                }
                _ => {}
            }
            if matches!(children[k], Node::Operator { symbol: ',' | ';', .. }) {
                if sep.is_some() {
                    break;
                }
                sep = Some(k);
            }
        }
        let concept = match (o, end.and_then(bracket)) {
            (']', Some('[')) => Some("open-interval"),
            (']', Some(']')) => Some("open-closed-interval"),
            ('[', Some('[')) => Some("closed-open-interval"),
            _ => None,
        };
        match (concept, sep, end) {
            (Some(c), Some(s), Some(e)) if s > i + 1 && e > s + 1 => {
                found.push((i, s, e, c));
                i = e + 1;
            }
            _ => i += 1,
        }
    }
    found
}
