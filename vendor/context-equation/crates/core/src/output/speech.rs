//! Phrase for screen readers, in the language of the locale.

use crate::layout::classes;
use crate::locale::Locale;
use crate::tree::*;

pub fn speak(node: &Node, loc: Locale) -> String {
    let mut f = Speaker { loc, words: Vec::new() };
    f.node(node);
    // #176: a symbol drawn by composition that no phrase read is its closest text
    let phrase = crate::composite::spell(&f.words.join(" "));
    phrase.split_whitespace().collect::<Vec<_>>().join(" ").replace(" ,", ",").replace(" ;", ";").replace(" :", ":")
}

struct Speaker {
    loc: Locale,
    words: Vec<String>,
}

impl Speaker {
    /// A phrase of the locale, filled with `args`.
    fn t(&self, key: &str, args: &[&str]) -> String {
        self.loc.fill(key, args)
    }

    fn say(&mut self, s: impl Into<String>) {
        let s = s.into();
        if !s.is_empty() {
            self.words.push(s);
        }
    }

    /// A phrase that may join the word before it: "pertence a" + "os reais" → "pertence aos reais".
    fn say_contracted(&mut self, phrase: String) {
        if let Some(last) = self.words.last_mut() {
            if let Some((joined, rest)) = self.loc.contract(last, &phrase) {
                *last = joined;
                self.say(rest);
                return;
            }
        }
        self.say(phrase);
    }

    fn spoken(&self, node: &Node) -> String {
        speak(node, self.loc)
    }

    fn node(&mut self, node: &Node) {
        match node {
            Node::Number { value, thousands, .. } => {
                let n = super::written_number(value, *thousands, self.loc);
                self.say(n);
            }
            Node::Identifier { name, variant } => match double_struck(name, *variant).and_then(|l| self.loc.double_struck(&l)) {
                Some(phrase) => self.say_contracted(phrase),
                None => {
                    let l = letter(name, self.loc);
                    self.say(l);
                }
            },
            Node::Function { name, .. } => {
                let f = self.loc.speak_function(name);
                self.say(f);
            }
            Node::Operator { symbol, class } | Node::FixedDelimiter { symbol, class, .. } => {
                let o = operator(*symbol, *class, self.loc);
                self.say(o);
            }
            Node::LargeOperator { symbol, .. } => {
                let o = large_operator(*symbol, self.loc);
                self.say(o);
            }
            Node::Row { children } => {
                let children = &pair_bars(children);
                let cl = classes(children, self.loc, None, false);
                for (i, (child, class)) in children.iter().zip(cl).enumerate() {
                    match (child, class) {
                        // L-OPR-1: between two vectors, the dot is the dot product: "v dot w"
                        (Node::Operator { symbol: '·', .. }, Some(Class::Bin))
                            if i > 0 && children.get(i + 1).is_some_and(is_vector) && is_vector(&children[i - 1]) && self.loc.own_phrase("dot_product").is_some() =>
                        {
                            self.say(self.loc.own_phrase("dot_product").unwrap_or_default());
                        }
                        (Node::Operator { symbol, .. }, Some(c)) => {
                            let o = operator(*symbol, c, self.loc);
                            self.say(o);
                        }
                        (other, _) => self.node(other),
                    }
                }
            }
            Node::Fraction { num, den, rule, .. } => {
                if !rule {
                    let (n, d) = (self.spoken(num), self.spoken(den));
                    self.say(self.t("binomial", &[&n, &d]));
                } else if let Some(name) = ordinary_fraction(num, den, self.loc) {
                    self.say(name);
                } else if is_simple(num) && is_simple(den) {
                    let (n, d) = (self.spoken(num), self.spoken(den));
                    self.say(self.t("fraction_simple", &[&n, &d]));
                } else {
                    let (n, d) = (self.spoken(num), self.spoken(den));
                    self.say(self.t("fraction", &[&n, &d]));
                }
            }
            Node::Root { radicand, index } => {
                let r = self.spoken(radicand);
                let phrase = match index.as_deref() {
                    None => self.t("sqrt", &[&r]),
                    Some(Node::Number { value, .. }) if value == "3" => self.t("cbrt", &[&r]),
                    Some(Node::Number { value, .. }) if value == "2" => self.t("sqrt", &[&r]),
                    Some(i) => {
                        let i = self.spoken(i);
                        self.t("root_index", &[&i, &r])
                    }
                };
                if is_simple(radicand) {
                    self.say(phrase);
                } else {
                    self.say(format!("{phrase}, {}", self.t("root_end", &[])));
                }
            }
            Node::Scripts { base, sup, sub } => self.scripts(base, sup.as_deref(), sub.as_deref()),
            Node::Delimited { open, close, parts, middles } => {
                // |x| and ‖v‖; a longer content gets an end marker, like roots: "absolute value of, a plus b, end absolute value"
                let unit = match (open, close) {
                    (Some('|'), Some('|')) => Some(("abs", "abs_long")),
                    (Some('‖'), Some('‖')) => Some(("norm", "norm_long")),
                    _ => None,
                };
                if let (Some((short, long)), [inner]) = (unit, parts.as_slice()) {
                    let x = self.spoken(inner);
                    let phrase = self.t(if is_simple(inner) { short } else { long }, &[&x]);
                    self.say(phrase);
                    return;
                }
                // \binom: the parentheses are part of "n choose k"
                if let ((Some('('), Some(')')), [p]) = ((open, close), parts.as_slice()) {
                    if let binomial @ Node::Fraction { rule: false, .. } = p.single() {
                        self.node(binomial);
                        return;
                    }
                }
                if let Some(a) = open {
                    let o = operator(*a, Class::Open, self.loc);
                    self.say(o);
                }
                for (i, p) in parts.iter().enumerate() {
                    if i > 0 {
                        let o = operator(middles.get(i - 1).copied().unwrap_or('|'), Class::Rel, self.loc);
                        self.say(o);
                    }
                    self.node(p);
                }
                if let Some(f) = close {
                    let o = operator(*f, Class::Close, self.loc);
                    self.say(o);
                }
            }
            Node::Table { shape: kind, rows, .. } => self.table(*kind, rows),
            Node::Accent { base, accent, .. } => {
                let b = self.spoken(base);
                let key = match accent {
                    '\u{20D7}' | '→' => "accent.vector",
                    '\u{0302}' => "accent.hat",
                    '\u{0304}' => "accent.bar",
                    '\u{0307}' => "accent.dot",
                    '\u{0308}' => "accent.ddot",
                    '\u{0303}' => "accent.tilde",
                    '⏜' | '⏠' => "accent.arc",
                    // KaTeX's \overlinesegment and \underlinesegment
                    '\u{20E9}' => "accent.segment",
                    _ => {
                        self.say(b);
                        return;
                    }
                };
                self.say(self.t(key, &[&b]));
            }
            Node::Bar { base, above } => {
                let b = self.spoken(base);
                if *above {
                    self.say(self.t("accent.bar", &[&b]));
                } else {
                    self.say(b);
                }
            }
            Node::Brace { base, label, .. } => {
                self.node(base);
                if let Some(r) = label {
                    let r = self.spoken(r);
                    self.say(format!("({r})"));
                }
            }
            Node::Text { parts } => {
                for p in parts {
                    match p {
                        TextPart::Text(t) => self.say(t.replace(crate::input::latex::MARKERS, "").trim().to_string()),
                        TextPart::Math(m) => self.node(m),
                    }
                }
            }
            Node::Space { .. } | Node::Phantom { .. } => {}
            Node::Canceled { base, shape } => {
                let b = self.spoken(base);
                // the Feynman slash is notation, not a cancellation: "D slash"
                self.say(self.t(if *shape == Cancel::Slash { "slashed" } else { "canceled" }, &[&b]));
            }
            Node::Framed { base, shape, .. } => {
                let b = self.spoken(base);
                match shape {
                    Frame::Circle => self.say(self.t("circled", &[&b])),
                    Frame::Rectangle => self.say(self.t("boxed", &[&b])),
                    Frame::Phase | Frame::ActuarialAngle => self.say(self.t("angle", &[&b])),
                    // a background is emphasis, like color: it is not read
                    Frame::Fill => self.say(b),
                }
            }
            Node::Colored { base, .. } | Node::Attributed { base, .. } | Node::ForcedClass { base, .. } | Node::Style { base, .. } | Node::Unit { base, .. } => self.node(base),
            Node::Quantity { value, unit } => {
                let named = units_by_name(unit, value.as_deref(), self.loc);
                self.node(&named);
            }
            Node::Arrow { symbol, above, below } => {
                // a labeled arrow is a function or a reaction, not a limit: it is not read as "tends to"
                let o = match symbol {
                    '→' => self.t("arrow.right", &[]),
                    '←' => self.t("arrow.left", &[]),
                    '↔' => self.t("arrow.both", &[]),
                    '⇒' => self.t("arrow.implies", &[]),
                    '=' => self.t("arrow.equals", &[]),
                    other => operator(*other, Class::Rel, self.loc),
                };
                self.say(o);
                for label in [above, below].into_iter().flatten() {
                    let l = self.spoken(label);
                    self.say(format!("({l})"));
                }
            }
            Node::Stacked { base, above, below } => {
                self.node(base);
                if let Some(a) = above {
                    let a = self.spoken(a);
                    self.say(self.t("with_above", &[&a]));
                }
                if let Some(b) = below {
                    let b = self.spoken(b);
                    self.say(self.t("with_below", &[&b]));
                }
            }
            Node::Multicolumn { base, .. }
            | Node::Raised { base, .. }
            | Node::Scaled { base, .. }
            | Node::Smashed { base, .. }
            | Node::Overlapped { base, .. }
            | Node::VCenter { base }
            | Node::Resized { base, .. }
            | Node::Rotated { base, .. }
            | Node::Reflected { base } => self.node(base),
            Node::VdotsWithin { .. } => self.node(&Node::VDOTS),
            // the speech does not follow the style: \mathchoice reads as its text style's branch
            Node::Choice { branches } => {
                if let Some(b) = branches.get(1) {
                    self.node(b);
                }
            }
            Node::Rule { .. } => {}
            Node::VerticalArrow { symbol, left, right } => {
                let arrow = match symbol {
                    '↓' => self.t("arrow.down", &[]),
                    '↑' => self.t("arrow.up", &[]),
                    _ => self.t("arrow.equals", &[]),
                };
                self.say(arrow);
                for r in [left, right].into_iter().flatten() {
                    let r = self.spoken(r);
                    self.say(format!("({r})"));
                }
            }
            #[cfg(feature = "chem")]
            Node::Chemistry { scheme } => self.say(crate::chem::speech::speak(scheme, self.loc)),
            Node::ArrowDiagram { cells, arrows } => {
                self.table(TableKind::Diagram, cells);
                let name = |this: &mut Self, (r, c): (usize, usize)| -> String {
                    let spoken = cells.get(r).and_then(|row| row.get(c)).map(|n| this.spoken(n)).unwrap_or_default();
                    if spoken.is_empty() {
                        format!("{} {}", this.t("row", &[&(r + 1).to_string()]), c + 1)
                    } else {
                        spoken
                    }
                };
                for a in arrows {
                    let (from, to) = (name(self, a.from), name(self, a.to));
                    let mut phrase = format!("; {}", self.t("diagram_arrow", &[&from, &to]));
                    for l in &a.labels {
                        // a label with nothing to say (xymatrix's \hole) is not read
                        let l = self.spoken(&l.node);
                        if !l.is_empty() {
                            phrase.push_str(&format!(" ({l})"));
                        }
                    }
                    self.say(phrase);
                }
            }
            Node::LongDivision { dividend, divisor } => {
                let (q, r) = divide(dividend, divisor);
                self.say(self.t("long_division", &[dividend, divisor, &q, &r]));
            }
            Node::ColumnArithmetic { operation, terms } => {
                let a: i128 = terms[0].parse().unwrap_or(0);
                let b: i128 = terms[1].parse().unwrap_or(0);
                let (op, r) = match operation {
                    '+' => (operator('+', Class::Bin, self.loc), a + b),
                    '−' => (operator('−', Class::Bin, self.loc), a - b),
                    _ => (operator('×', Class::Bin, self.loc), a.saturating_mul(b)),
                };
                self.say(self.t("column_arithmetic", &[&a.to_string(), &op, &b.to_string(), &r.to_string()]));
            }
        }
    }

    fn scripts(&mut self, base: &Node, sup: Option<&Node>, sub: Option<&Node>) {
        let base = base.operator_core();
        if let Node::LargeOperator { symbol, .. } = base {
            let name = large_operator(*symbol, self.loc);
            let from = sub.map(|s| self.spoken(s));
            let to = sup.map(|s| self.spoken(s));
            let phrase = match (from, to) {
                (Some(d), Some(a)) => self.t("big_from_to", &[&name, &d, &a]),
                (Some(d), None) => self.t("big_over", &[&name, &d]),
                (None, Some(a)) => self.t("big_to", &[&name, &a]),
                (None, None) => name,
            };
            self.say(phrase);
            return;
        }
        if let (Node::Function { name, .. }, Some(s)) = (base, sub) {
            if name == "lim" {
                let s = self.spoken(s).replace(self.loc.phrase("op.→"), self.loc.phrase("tending_to"));
                self.say(self.t("lim_as", &[&s]));
                if let Some(p) = sup {
                    let p = self.spoken(p);
                    self.say(p);
                }
                return;
            }
        }
        // sen² x, log₂ x: the scripts belong to the function name, before "de"/"of"
        if let Node::Function { name, .. } = base {
            let spoken = self.loc.function_spoken(name);
            let bare = spoken.map_or_else(|| self.loc.function_name(name), str::to_string);
            // sen⁻¹ x is the inverse function, not 1/sen x: "arco seno de x", "inverse sine of x"
            let minus_one = format!("{} 1", self.loc.phrase("negative"));
            let inverse = sup.is_some_and(|s| self.spoken(s) == minus_one);
            let trig = matches!(name.as_str(), "sin" | "cos" | "tan" | "cot" | "sec" | "csc");
            let mut words = vec![match (inverse, trig) {
                (false, _) => bare,
                (true, true) => self.t("inverse_trig", &[&bare]),
                (true, false) => self.t("inverse", &[&bare]),
            }];
            if let Some(s) = sub {
                let s = self.spoken(s);
                words.push(self.t(if name == "log" { "log_base" } else { "sub" }, &[&s]));
            }
            if let Some(s) = sup.filter(|_| !inverse) {
                words.push(self.superscript(s));
            }
            let words = words.join(" ");
            self.say(if spoken.is_some() { self.t("function_of", &[&words]) } else { words });
            return;
        }
        self.node(base);
        if let Some(s) = sub {
            let s = self.spoken(s);
            self.say(self.t("sub", &[&s]));
        }
        if let Some(s) = sup {
            let phrase = self.superscript(s);
            self.say(phrase);
        }
    }

    fn superscript(&mut self, s: &Node) -> String {
        match s {
            Node::Number { value, .. } if value == "2" => self.t("squared", &[]),
            Node::Number { value, .. } if value == "3" => self.t("cubed", &[]),
            Node::Operator { symbol: '′', .. } => self.t("prime", &[]),
            Node::Operator { symbol: '″', .. } => self.t("double_prime", &[]),
            Node::Operator { symbol: '°' | '∘', .. } => self.t("degrees", &[]),
            Node::Operator { symbol: '+', .. } => self.t("from_right", &[]),
            Node::Operator { symbol: '−', .. } => self.t("from_left", &[]),
            other => {
                let x = self.spoken(other);
                self.t(if is_simple(other) { "power" } else { "power_long" }, &[&x])
            }
        }
    }

    fn table(&mut self, kind: TableKind, rows: &[Vec<Node>]) {
        use TableKind::*;
        let ncol = rows.iter().map(Vec::len).max().unwrap_or(0);
        let spoken_rows: Vec<String> =
            rows.iter().map(|l| l.iter().map(|c| self.spoken(c)).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(if kind == Aligned { " " } else { ", " })).collect();
        let body =
            |label: &str| -> String { spoken_rows.iter().enumerate().map(|(i, l)| format!("{}: {l}", self.t(label, &[&(i + 1).to_string()]))).collect::<Vec<_>>().join("; ") };
        let phrase = match kind {
            Matrix | PMatrix | BMatrix | BraceMatrix | VVMatrix | SmallMatrix | BorderMatrix => {
                format!("{}: {}", self.t("matrix", &[&rows.len().to_string(), &ncol.to_string()]), body("row"))
            }
            VMatrix => format!("{}: {}", self.t("determinant", &[]), body("row")),
            Cases | RCases => format!("{}: {}", self.t("cases", &[]), body("case")),
            Array | Substack => spoken_rows.join("; "),
            Diagram => format!("{}: {}", self.t("diagram", &[]), body("row")),
            Aligned | Centered => spoken_rows.join("; "),
        };
        self.say(phrase);
    }
}

/// `|x|` and `‖v‖` written with plain bars read as one unit, like `\left| x \right|`: each bar is
/// paired with the next equal one, when there is something between them.
fn pair_bars(children: &[Node]) -> Vec<Node> {
    let bar = |n: &Node| match n {
        Node::Operator { symbol: c @ ('|' | '‖'), .. } => Some(*c),
        _ => None,
    };
    let mut out = Vec::with_capacity(children.len());
    let mut i = 0;
    while i < children.len() {
        let close = bar(&children[i]).and_then(|c| children[i + 1..].iter().position(|n| bar(n) == Some(c)).map(|p| (c, i + 1 + p)));
        match close {
            Some((c, j)) if j > i + 1 => {
                out.push(Node::Delimited { open: Some(c), close: Some(c), parts: vec![Node::row(children[i + 1..j].to_vec()).simplify()], middles: Vec::new() });
                i = j + 1;
            }
            _ => {
                out.push(children[i].clone());
                i += 1;
            }
        }
    }
    out
}

/// The prefix and name of a unit, alone or with its power (`\metre\squared`); None for any other node.
fn unit_of(n: &Node) -> Option<(Option<&str>, &str)> {
    let n = if let Node::Scripts { base, .. } = n { base.as_ref() } else { n };
    if let Node::Unit { prefix, name, .. } = n {
        Some((prefix.as_deref(), name.as_str()))
    } else {
        None
    }
}

/// A single token: number, letter, or the same thing with a simple subscript.
/// L-UNI-3: a quantity's units by their names, in the languages that have them ("9.81 meters per
/// second squared"): each unit becomes the text of its name, `/` becomes "per", and the grouping of
/// the denominator goes. The last unit before "per" agrees in number with the value. A unit the
/// language does not name keeps its symbol; a language without unit names reads the symbols.
fn units_by_name(unit: &Node, value: Option<&str>, loc: Locale) -> Node {
    let Some(per) = loc.unit_per() else {
        return unit.clone();
    };
    let plural = value.is_none_or(|v| loc.unit_plural(v));
    // a unit (alone or with its power) as the text of its name, keeping its scripts
    let name = |node: &Node, (prefix, unit): (Option<&str>, &str), plural: bool| -> Node {
        let Some(t) = loc.unit_name(prefix, unit, plural) else {
            return node.clone();
        };
        let text = Node::Text { parts: vec![TextPart::Text(t)] };
        match node {
            Node::Scripts { sup, sub, .. } => Node::Scripts { base: Box::new(text), sup: sup.clone(), sub: sub.clone() },
            _ => text,
        }
    };
    let is_unit = |n: &Node| unit_of(n).is_some();
    let words = |children: &[Node]| -> Vec<Node> {
        let slash = children.iter().position(|c| matches!(c, Node::Operator { symbol: '/', .. }));
        let numerator = slash.unwrap_or(children.len());
        let last = children[..numerator].iter().rposition(is_unit);
        let mut out = Vec::new();
        for (i, c) in children.iter().enumerate() {
            match c {
                // 1/s: "per second"
                Node::Number { .. } if slash == Some(1) && i == 0 => {}
                Node::Operator { symbol: '/', .. } => out.push(Node::Text { parts: vec![TextPart::Text(per.to_string())] }),
                Node::Operator { symbol: '(' | ')' | '·', .. } if slash.is_some_and(|s| i > s) => {}
                c => out.push(match unit_of(c) {
                    Some(u) => name(c, u, plural && Some(i) == last),
                    None => c.clone(),
                }),
            }
        }
        out
    };
    match unit {
        Node::Row { children } => Node::row(
            children
                .iter()
                .map(|c| match c {
                    Node::Row { children: inner } if inner.iter().any(is_unit) => Node::row(words(inner)),
                    // siunitx's per-mode=fraction: the units over the others, read as with the slash
                    Node::Row { children: inner } if matches!(inner.as_slice(), [Node::Fraction { den, .. }] if den.children().iter().any(is_unit)) => {
                        let [Node::Fraction { num, den, .. }] = inner.as_slice() else { unreachable!("matched above") };
                        let mut flat = match num.as_ref() {
                            Node::Row { children } => children.clone(),
                            other => vec![other.clone()],
                        };
                        flat.push(Node::Operator { symbol: '/', class: Class::Ord });
                        flat.extend(den.children().iter().cloned());
                        Node::row(words(&flat))
                    }
                    other => other.clone(),
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

/// L-OPR-1: a vector by its notation: an arrow over it (`\vec`, `\overrightarrow`) or a bold letter
/// (`\mathbf`, `\boldsymbol`, `\vb`), also with an index (`\vec{v}_1`).
fn is_vector(node: &Node) -> bool {
    match node {
        Node::Accent { accent: '⃗' | '→', .. } => true,
        Node::Identifier { variant: Variant::Bold | Variant::BoldItalic, .. } => true,
        Node::Row { children } => children.len() == 1 && is_vector(&children[0]),
        Node::Scripts { base, sup: None, .. } => is_vector(base),
        _ => false,
    }
}

fn is_simple(node: &Node) -> bool {
    match node {
        Node::Number { .. } | Node::Identifier { .. } | Node::Operator { .. } => true,
        Node::Row { children } => children.len() == 1 && is_simple(&children[0]),
        Node::Scripts { base, sup, sub } => is_simple(base) && sup.as_deref().is_none_or(is_simple) && sub.as_deref().is_none_or(is_simple),
        _ => false,
    }
}

/// 1/2 → "um meio", 3/4 → "três quartos", the way it is read at school, in the languages that have the
/// names (`fractions` in the locale file).
fn ordinary_fraction(num: &Node, den: &Node, loc: Locale) -> Option<String> {
    let names = loc.fractions()?;
    let (Node::Number { value: n, .. }, Node::Number { value: d, .. }) = (num, den) else {
        return None;
    };
    let n: usize = n.parse().ok()?;
    let d: usize = d.parse().ok()?;
    if !(1..=10).contains(&n) || !(2..=10).contains(&d) {
        return None;
    }
    let denominator = if n > 1 { &names.plural[d] } else { &names.singular[d] };
    Some(format!("{} {denominator}", names.numerals[n]))
}

fn divide(a: &str, b: &str) -> (String, String) {
    match (a.parse::<u128>(), b.parse::<u128>()) {
        (Ok(a), Ok(b)) if b > 0 => ((a / b).to_string(), (a % b).to_string()),
        _ => ("?".into(), "?".into()),
    }
}

fn large_operator(c: char, loc: Locale) -> String {
    let key = match c {
        '∑' | '∏' | '∫' | '∬' | '∭' | '∮' | '⋃' | '⋂' => format!("big.{c}"),
        // the other integrals are read as an integral
        c if crate::layout::is_integral(c) => "big.∫".into(),
        _ => "big.other".into(),
    };
    loc.phrase(&key).to_string()
}

/// L-LET-2: the Latin capital of a double-struck letter, or the digit of a double-struck digit (𝟙, the
/// indicator function), written with `\mathbb` or as the Unicode character (ℝ, 𝔸, 𝟙).
fn double_struck(name: &str, variant: Variant) -> Option<String> {
    let mut chars = name.chars();
    let (c, None) = (chars.next()?, chars.next()) else { return None };
    let capital = match c {
        'A'..='Z' | '0'..='9' if variant == Variant::DoubleStruck => c,
        '\u{1D7D8}'..='\u{1D7E1}' => char::from_u32('0' as u32 + (c as u32 - 0x1D7D8))?,
        'ℂ' => 'C',
        'ℍ' => 'H',
        'ℕ' => 'N',
        'ℙ' => 'P',
        'ℚ' => 'Q',
        'ℝ' => 'R',
        'ℤ' => 'Z',
        '\u{1D538}'..='\u{1D551}' => char::from_u32('A' as u32 + (c as u32 - 0x1D538))?,
        _ => return None,
    };
    Some(capital.to_string())
}

fn letter(name: &str, loc: Locale) -> String {
    let lower = match name {
        "ϵ" => "ε",
        "ϑ" => "θ",
        "ϕ" => "φ",
        other => other,
    };
    if let Some(n) = loc.optional(&format!("greek.{lower}")) {
        return n.to_string();
    }
    // capitals with their own shape: "capital gamma", "gama maiúsculo"
    let capital = match name {
        "Γ" => "γ",
        "Δ" => "δ",
        "Θ" => "θ",
        "Λ" => "λ",
        "Π" => "π",
        "Σ" => "σ",
        "Φ" => "φ",
        "Ψ" => "ψ",
        "Ω" => "ω",
        _ => return name.to_string(),
    };
    loc.fill("capital", &[loc.phrase(&format!("greek.{capital}"))])
}

fn operator(symbol: char, class: Class, loc: Locale) -> String {
    // #176: a symbol drawn by composition is read as the character whose meaning it has (a short arrow
    // as the arrow), or as its closest text
    if let Some(key) = crate::composite::speech_key(symbol) {
        return loc.phrase(key).to_string();
    }
    if let Some(k) = crate::composite::get(symbol) {
        return k.meaning.map_or_else(|| k.text.to_string(), |m| operator(m, class, loc));
    }
    let key = match symbol {
        '−' if class == Class::Ord => "negative".to_string(),
        // L-INT-1: ]a, b[
        ']' if class == Class::Open => "open_interval".to_string(),
        '[' if class == Class::Close => "close_interval".to_string(),
        '·' | '∗' => "op.×".to_string(),
        '÷' => "op./".to_string(),
        '⟶' => "op.→".to_string(),
        '⟹' => "op.⇒".to_string(),
        '⟺' => "op.⇔".to_string(),
        // \Longleftarrow and \impliedby (#250) are read as ⇐
        '⟸' => return operator('⇐', class, loc),
        '∣' => "op.|".to_string(),
        // \vdots and \vdotswithin: the list goes on, down the column
        '⋯' | '⋮' => "op.…".to_string(),
        '″' => "double_prime".to_string(),
        // physics' bold nabla is read as the nabla
        '𝛁' => "op.∇".to_string(),
        other => format!("op.{other}"),
    };
    loc.optional(&key).map_or_else(|| symbol.to_string(), str::to_string)
}
