//! Content MathML (MathML 3, chapter 4): meaning without a drawing, as Maple, Mathematica's export,
//! OpenMath converters and assessment tools write it. Each expression is spelled as the LaTeX an author
//! would type for it (`<apply><plus/><apply><power/><ci>x</ci><cn>2</cn></apply><cn>1</cn></apply>` is
//! `x^{2} + 1`) and read by the LaTeX reader, so its tree is that LaTeX's tree: the same drawing, speech,
//! text and braille, with the locale's rules. Parentheses are added where precedence needs them
//! (`(a + b) \cdot c`). The subset read is in spec/input.md; any other content element is E-REC-10.

use roxmltree::Node as Xml;

use crate::tree::Node;
use crate::Error;

use super::{text_of, Reader, R};
use crate::input::latex::{self, NESTING_LIMIT};

/// The elements this module reads: the ones that start a content expression.
pub(super) fn is_content(name: &str) -> bool {
    matches!(name, "apply" | "ci" | "cn" | "csymbol" | "set" | "interval" | "bind" | "bvar" | "lambda" | "piecewise" | "list" | "vector" | "matrix") || constant(name).is_some()
}

/// The constants written as empty elements (`<pi/>`), in LaTeX.
fn constant(name: &str) -> Option<&'static str> {
    Some(match name {
        "pi" => r"\pi",
        "exponentiale" => "e",
        "imaginaryi" => "i",
        "infinity" => r"\infty",
        "emptyset" => r"\emptyset",
        "eulergamma" => r"\gamma",
        "naturalnumbers" => r"\mathbb{N}",
        "integers" => r"\mathbb{Z}",
        "rationals" => r"\mathbb{Q}",
        "reals" => r"\mathbb{R}",
        "complexes" => r"\mathbb{C}",
        "primes" => r"\mathbb{P}",
        "true" => r"\mathrm{true}",
        "false" => r"\mathrm{false}",
        "notanumber" => r"\mathrm{NaN}",
        _ => return None,
    })
}

/// The elementary functions, in LaTeX: written before their argument (`\sin x`).
fn function(name: &str) -> Option<&'static str> {
    Some(match name {
        "sin" => r"\sin",
        "cos" => r"\cos",
        "tan" => r"\tan",
        "sec" => r"\sec",
        "csc" => r"\csc",
        "cot" => r"\cot",
        "arcsin" => r"\arcsin",
        "arccos" => r"\arccos",
        "arctan" => r"\arctan",
        "sinh" => r"\sinh",
        "cosh" => r"\cosh",
        "tanh" => r"\tanh",
        "ln" => r"\ln",
        "log" => r"\log",
        "max" => r"\max",
        "min" => r"\min",
        "gcd" => r"\gcd",
        _ => return None,
    })
}

/// Relations between two or more operands, in LaTeX.
fn relation(name: &str) -> Option<&'static str> {
    Some(match name {
        "eq" => "=",
        "neq" => r"\neq",
        "lt" => "<",
        "gt" => ">",
        "leq" => r"\leq",
        "geq" => r"\geq",
        "approx" => r"\approx",
        "equivalent" => r"\equiv",
        "in" => r"\in",
        "notin" => r"\notin",
        "subset" => r"\subseteq",
        "prsubset" => r"\subset",
        "tendsto" => r"\to",
        _ => return None,
    })
}

/// How tightly an expression holds together, loosest first: it decides the parentheses around it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Prec {
    Rel,
    Add,
    Mul,
    Pow,
    Atom,
}

/// What an expression is, where it matters for the spelling around it.
#[derive(Debug, Clone, PartialEq)]
enum Kind {
    /// A variable or a constant (`x`, `\pi`), or a power of one: may follow a number without a dot (`2x`).
    Letter,
    /// A number without a sign.
    Number,
    /// An elementary function applied to its argument: its power goes on the name (`\sin^{2} x`).
    Function {
        head: String,
        argument: String,
    },
    Other,
}

/// An expression spelled in LaTeX.
#[derive(Debug, Clone)]
struct Tex {
    s: String,
    prec: Prec,
    /// It starts with a minus sign (`-x`, `-3`): parenthesized after another operator.
    negative: bool,
    /// It holds a fraction: its parentheses stretch (`\left( … \right)`).
    tall: bool,
    kind: Kind,
}

impl Tex {
    fn new(s: impl Into<String>, prec: Prec) -> Tex {
        Tex { s: s.into(), prec, negative: false, tall: false, kind: Kind::Other }
    }

    fn kind(mut self, kind: Kind) -> Tex {
        self.kind = kind;
        self
    }

    fn tall(mut self, tall: bool) -> Tex {
        self.tall = tall;
        self
    }

    /// In parentheses when `needed`.
    fn wrapped(&self, needed: bool) -> String {
        match (needed, self.tall) {
            (false, _) => self.s.clone(),
            (true, false) => format!("({})", self.s),
            (true, true) => format!(r"\left( {} \right)", self.s),
        }
    }
}

/// The children of `<apply>` (and of `<set>`) that qualify the operator instead of being operands.
#[derive(Default)]
struct Qualifiers<'a, 'input> {
    bvars: Vec<Xml<'a, 'input>>,
    lowlimit: Option<Xml<'a, 'input>>,
    uplimit: Option<Xml<'a, 'input>>,
    degree: Option<Xml<'a, 'input>>,
    logbase: Option<Xml<'a, 'input>>,
    condition: Option<Xml<'a, 'input>>,
    domain: Option<Xml<'a, 'input>>,
    interval: Option<Xml<'a, 'input>>,
}

impl Reader<'_> {
    /// A content expression, read as the tree of its LaTeX spelling.
    pub(super) fn content(&mut self, node: Xml) -> R<Node> {
        let tex = self.expression(node)?;
        latex::read_with(&tex.s, self.locale, self.registry, self.strict)
    }

    fn expression(&mut self, node: Xml) -> R<Tex> {
        if self.depth >= NESTING_LIMIT {
            return Err(Error::Syntax { column: self.column(node), message: format!("nesting deeper than {NESTING_LIMIT} levels") });
        }
        self.depth += 1;
        let r = self.expression_unguarded(node);
        self.depth -= 1;
        r
    }

    fn expression_unguarded(&mut self, node: Xml) -> R<Tex> {
        let name = node.tag_name().name();
        if let Some(c) = constant(name) {
            return Ok(Tex::new(c, Prec::Atom).kind(Kind::Letter));
        }
        match name {
            "ci" => self.ci(node),
            "csymbol" => Ok(identifier(&text_of(node))),
            "cn" => self.cn(node),
            "apply" => self.apply(node),
            "set" => self.set(node),
            "interval" => self.interval(node),
            // semantics inside content: its first child, which is content here
            "semantics" => match Self::elements(node).into_iter().find(|c| !c.tag_name().name().starts_with("annotation")) {
                Some(c) => self.expression(c),
                None => Ok(Tex::new("{}", Prec::Atom)),
            },
            other => self.unsupported(node, other, &Self::elements(node)),
        }
    }

    /// E-REC-10: a content element outside the subset; its operands are read side by side.
    fn unsupported(&mut self, node: Xml, name: &str, operands: &[Xml]) -> R<Tex> {
        self.tolerate(node, format!("content MathML <{name}> is not supported"), "its operands read as a row")?;
        let mut parts = Vec::new();
        for o in operands {
            parts.push(self.expression(*o)?.wrapped(false));
        }
        Ok(Tex::new(format!("{{{}}}", parts.join(" ")), Prec::Atom))
    }

    /// `<ci>`: a variable, or presentation markup inside (`<ci><msub>…</msub></ci>`).
    fn ci(&mut self, node: Xml) -> R<Tex> {
        let children = Self::elements(node);
        if children.is_empty() {
            return Ok(identifier(&text_of(node)));
        }
        let mut parts = Vec::new();
        for c in children {
            parts.push(self.presentation(c)?);
        }
        Ok(Tex::new(parts.join(" "), Prec::Atom).kind(Kind::Letter))
    }

    /// The presentation markup a `<ci>` may hold, in LaTeX: tokens, rows, scripts and accents.
    fn presentation(&mut self, node: Xml) -> R<String> {
        let args = |this: &mut Self| -> R<Vec<String>> { Self::elements(node).into_iter().map(|c| this.presentation(c)).collect() };
        let name = node.tag_name().name();
        Ok(match name {
            "mi" | "mn" | "mo" => {
                let t = text_of(node);
                if t.chars().count() > 1 && name == "mi" {
                    format!(r"\mathrm{{{}}}", escape(&t))
                } else {
                    escape(&t)
                }
            }
            "mtext" => format!(r"\text{{{}}}", escape(&text_of(node))),
            "mrow" => format!("{{{}}}", args(self)?.join(" ")),
            "msub" | "msup" | "msubsup" => match args(self)?.as_slice() {
                [base, script] if name == "msub" => format!("{}_{{{script}}}", group(base)),
                [base, script] => format!("{}^{{{script}}}", group(base)),
                [base, sub, sup] => format!("{}_{{{sub}}}^{{{sup}}}", group(base)),
                _ => return self.presentation_unsupported(node),
            },
            "mover" => match (args(self)?.as_slice(), Self::elements(node).get(1).map(|m| text_of(*m))) {
                ([base, _], Some(mark)) => match mark.as_str() {
                    "^" | "ˆ" => format!(r"\hat{{{base}}}"),
                    "¯" | "‾" | "_" | "−" => format!(r"\bar{{{base}}}"),
                    "→" | "\u{20D7}" => format!(r"\vec{{{base}}}"),
                    "˙" | "." => format!(r"\dot{{{base}}}"),
                    "~" | "˜" => format!(r"\tilde{{{base}}}"),
                    _ => return self.presentation_unsupported(node),
                },
                _ => return self.presentation_unsupported(node),
            },
            _ => return self.presentation_unsupported(node),
        })
    }

    fn presentation_unsupported(&mut self, node: Xml) -> R<String> {
        let name = node.tag_name().name();
        self.tolerate(node, format!("<{name}> inside <ci> is not supported"), "its text read as is")?;
        Ok(format!(r"\mathrm{{{}}}", escape(&text_of(node))))
    }

    /// `<cn>`: a number; `type="rational"`, `"e-notation"` and `"complex-cartesian"` have two parts
    /// separated by `<sep/>`.
    fn cn(&mut self, node: Xml) -> R<Tex> {
        let mut parts = vec![String::new()];
        for child in node.children() {
            if child.is_element() && child.tag_name().name() == "sep" {
                parts.push(String::new());
            } else if let Some(t) = child.text().filter(|_| child.is_text()) {
                parts.last_mut().expect("one part").push_str(t);
            } else if child.is_element() {
                parts.last_mut().expect("one part").push_str(&text_of(child));
            }
        }
        let parts: Vec<String> = parts.iter().map(|p| p.chars().filter(|c| !c.is_whitespace()).collect()).collect();
        match (node.attribute("type").unwrap_or("real"), parts.as_slice()) {
            ("rational", [n, d]) => {
                let (n, d) = (self.cn_number(node, n)?, self.cn_number(node, d)?);
                let negative = n.negative;
                let s = format!(r"\frac{{{}}}{{{}}}", n.s.trim_start_matches('-'), d.s);
                Ok(Tex { s: if negative { format!("-{s}") } else { s }, prec: if negative { Prec::Mul } else { Prec::Atom }, negative, tall: true, kind: Kind::Other })
            }
            ("e-notation", [m, e]) => {
                let (m, e) = (self.cn_number(node, m)?, self.cn_number(node, e)?);
                Ok(Tex { s: format!(r"{} \times 10^{{{}}}", m.s, e.s), prec: Prec::Mul, negative: m.negative, tall: false, kind: Kind::Other })
            }
            ("complex-cartesian", [a, b]) => {
                let (a, b) = (self.cn_number(node, a)?, self.cn_number(node, b)?);
                let (sign, b) = if b.negative { ("-", b.s.trim_start_matches('-').to_string()) } else { ("+", b.s) };
                Ok(Tex { s: format!("{} {sign} {b} i", a.s), prec: Prec::Add, negative: a.negative, tall: false, kind: Kind::Other })
            }
            ("constant", _) => Ok(identifier(&parts.concat())),
            _ => {
                if parts.len() > 1 {
                    self.tolerate(node, "<sep/> in a <cn> that is not rational, e-notation or complex-cartesian".into(), "the parts read together")?;
                }
                self.cn_number(node, &parts.concat())
            }
        }
    }

    /// A number as written in content MathML (a dot for decimals, maybe a sign), in the locale's
    /// spelling for the LaTeX reader: `3.14` is `3,14` in pt-BR.
    fn cn_number(&mut self, node: Xml, t: &str) -> R<Tex> {
        let (negative, digits) = match t.strip_prefix(['-', '−']) {
            Some(rest) => (true, rest),
            None => (false, t.strip_prefix('+').unwrap_or(t)),
        };
        let valid = !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit() || c == '.') && digits.matches('.').count() <= 1 && digits != ".";
        if !valid {
            self.tolerate(node, format!("<cn> is not a number: '{t}'"), "written as text")?;
            return Ok(Tex::new(format!(r"\mathrm{{{}}}", escape(t)), Prec::Atom));
        }
        let digits = if self.locale.comma_is_decimal() { digits.replace('.', ",") } else { digits.to_string() };
        Ok(if negative {
            Tex { s: format!("-{digits}"), prec: Prec::Mul, negative: true, tall: false, kind: Kind::Other }
        } else {
            Tex::new(digits, Prec::Atom).kind(Kind::Number)
        })
    }

    /// The qualifiers of an `<apply>` or `<set>` and its operands, in order.
    fn split<'a, 'input>(children: &[Xml<'a, 'input>]) -> (Qualifiers<'a, 'input>, Vec<Xml<'a, 'input>>) {
        let mut q = Qualifiers::default();
        let mut operands = Vec::new();
        for c in children {
            match c.tag_name().name() {
                "bvar" => q.bvars.push(*c),
                "lowlimit" => q.lowlimit = Some(*c),
                "uplimit" => q.uplimit = Some(*c),
                "degree" => q.degree = Some(*c),
                "logbase" => q.logbase = Some(*c),
                "condition" => q.condition = Some(*c),
                "domainofapplication" => q.domain = Some(*c),
                _ => operands.push(*c),
            }
        }
        (q, operands)
    }

    /// The content inside a qualifier (`<lowlimit><cn>0</cn></lowlimit>`).
    fn inside(&mut self, qualifier: Option<Xml>) -> R<Option<Tex>> {
        match qualifier.and_then(|q| Self::elements(q).into_iter().next()) {
            Some(c) => Ok(Some(self.expression(c)?)),
            None => Ok(None),
        }
    }

    /// A bound variable and its degree: `<bvar><ci>x</ci><degree><cn>2</cn></degree></bvar>`.
    fn bvar(&mut self, bvar: Xml) -> R<(Tex, Option<Tex>)> {
        let (q, operands) = Self::split(&Self::elements(bvar));
        let variable = match operands.first() {
            Some(v) => self.expression(*v)?,
            None => Tex::new("{}", Prec::Atom),
        };
        Ok((variable, self.inside(q.degree)?))
    }

    /// Exactly `n` operands; a missing one is left empty (E-REC-10).
    fn operands(&mut self, node: Xml, name: &str, operands: &[Xml], n: usize) -> R<Vec<Tex>> {
        if operands.len() != n {
            self.tolerate(node, format!("content MathML <{name}> takes {n} operands and has {}", operands.len()), "the missing ones left empty, the extra ones ignored")?;
        }
        let mut out = Vec::new();
        for i in 0..n {
            out.push(match operands.get(i) {
                Some(o) => self.expression(*o)?,
                None => Tex::new("{}", Prec::Atom),
            });
        }
        Ok(out)
    }

    fn all(&mut self, operands: &[Xml]) -> R<Vec<Tex>> {
        operands.iter().map(|o| self.expression(*o)).collect()
    }

    fn apply(&mut self, node: Xml) -> R<Tex> {
        let children = Self::elements(node);
        let Some((&op, rest)) = children.split_first() else {
            self.tolerate(node, "content MathML <apply> without an operator".into(), "left empty")?;
            return Ok(Tex::new("{}", Prec::Atom));
        };
        let (mut q, mut operands) = Self::split(rest);
        let tag = op.tag_name().name();
        // the operator by name: an empty element (<plus/>), a csymbol (OpenMath's arith1 plus), or a ci
        // that names an elementary function
        let name = match tag {
            "csymbol" => text_of(op),
            "ci" if Self::elements(op).is_empty() && function(&text_of(op)).is_some() => text_of(op),
            "ci" | "apply" => return self.application(op, &operands),
            other => other.to_string(),
        };
        let name = match name.as_str() {
            "unary_minus" => "minus".to_string(),
            "unary_plus" => "plus".to_string(),
            _ => name,
        };
        // the limits of an integral or a sum may come as an interval: <int/><bvar/><interval>…
        if matches!(name.as_str(), "int" | "sum" | "product") && operands.len() > 1 && operands[0].tag_name().name() == "interval" {
            q.interval = Some(operands.remove(0));
        }
        if let Some(r) = relation(&name) {
            let parts = self.all(&operands)?;
            if parts.len() < 2 {
                self.tolerate(node, format!("content MathML <{name}> takes two operands or more and has {}", parts.len()), "written as is")?;
            }
            let s: Vec<String> = parts.iter().map(|p| p.wrapped(p.prec <= Prec::Rel)).collect();
            return Ok(Tex::new(s.join(&format!(" {r} ")), Prec::Rel));
        }
        if let Some(f) = function(&name) {
            let head = match (name.as_str(), self.inside(q.logbase)?) {
                ("log", Some(base)) => format!("{f}_{{{}}}", base.s),
                _ => f.to_string(),
            };
            let parts = self.all(&operands)?;
            return Ok(applied(&head, &parts));
        }
        match name.as_str() {
            "plus" => {
                let parts = self.all(&operands)?;
                if parts.len() == 1 {
                    let p = &parts[0];
                    return Ok(Tex::new(format!("+{}", p.wrapped(p.prec <= Prec::Add || p.negative)), Prec::Mul));
                }
                let s: Vec<String> = parts.iter().enumerate().map(|(i, p)| p.wrapped(if i == 0 { p.prec < Prec::Add } else { p.prec <= Prec::Add || p.negative })).collect();
                let negative = parts.first().is_some_and(|p| p.negative);
                Ok(Tex { s: s.join(" + "), prec: Prec::Add, negative, tall: parts.iter().any(|p| p.tall), kind: Kind::Other })
            }
            "minus" if operands.len() == 1 => {
                let p = self.expression(operands[0])?;
                Ok(Tex { s: format!("-{}", p.wrapped(p.prec <= Prec::Add || p.negative)), prec: Prec::Mul, negative: true, tall: p.tall, kind: Kind::Other })
            }
            "minus" => {
                let [a, b]: [Tex; 2] = self.operands(node, "minus", &operands, 2)?.try_into().expect("two operands");
                let s = format!("{} - {}", a.wrapped(a.prec < Prec::Add), b.wrapped(b.prec <= Prec::Add || b.negative));
                Ok(Tex { s, prec: Prec::Add, negative: a.negative, tall: a.tall || b.tall, kind: Kind::Other })
            }
            "times" => {
                let parts = self.all(&operands)?;
                Ok(product(&parts))
            }
            "divide" => {
                let [a, b]: [Tex; 2] = self.operands(node, "divide", &operands, 2)?.try_into().expect("two operands");
                Ok(Tex::new(format!(r"\frac{{{}}}{{{}}}", a.s, b.s), Prec::Atom).tall(true))
            }
            "power" => {
                let [base, exponent]: [Tex; 2] = self.operands(node, "power", &operands, 2)?.try_into().expect("two operands");
                Ok(power(&base, &exponent.s))
            }
            "exp" => {
                let [x]: [Tex; 1] = self.operands(node, "exp", &operands, 1)?.try_into().expect("one operand");
                Ok(Tex::new(format!("e^{{{}}}", x.s), Prec::Pow).kind(Kind::Letter))
            }
            "root" => {
                let [x]: [Tex; 1] = self.operands(node, "root", &operands, 1)?.try_into().expect("one operand");
                let s = match self.inside(q.degree)? {
                    Some(n) if n.s != "2" => format!(r"\sqrt[{}]{{{}}}", n.s, x.s),
                    _ => format!(r"\sqrt{{{}}}", x.s),
                };
                Ok(Tex::new(s, Prec::Atom).tall(x.tall))
            }
            "abs" => {
                let [x]: [Tex; 1] = self.operands(node, "abs", &operands, 1)?.try_into().expect("one operand");
                let s = if x.tall { format!(r"\left| {} \right|", x.s) } else { format!("|{}|", x.s) };
                Ok(Tex::new(s, Prec::Atom).tall(x.tall))
            }
            "floor" | "ceiling" => {
                let [x]: [Tex; 1] = self.operands(node, &name, &operands, 1)?.try_into().expect("one operand");
                let (l, r) = if name == "floor" { (r"\lfloor", r"\rfloor") } else { (r"\lceil", r"\rceil") };
                Ok(Tex::new(format!("{l} {} {r}", x.s), Prec::Atom).tall(x.tall))
            }
            "factorial" => {
                let [x]: [Tex; 1] = self.operands(node, "factorial", &operands, 1)?.try_into().expect("one operand");
                Ok(Tex::new(format!("{}!", x.wrapped(x.prec < Prec::Atom || x.negative)), Prec::Pow).tall(x.tall))
            }
            "union" | "intersect" | "setdiff" => {
                let symbol = match name.as_str() {
                    "union" => r"\cup",
                    "intersect" => r"\cap",
                    _ => r"\setminus",
                };
                let parts = self.all(&operands)?;
                let s: Vec<String> = parts.iter().map(|p| p.wrapped(p.prec <= Prec::Add)).collect();
                Ok(Tex::new(s.join(&format!(" {symbol} ")), Prec::Add))
            }
            "diff" | "partialdiff" => self.derivative(node, &name, &q, &operands),
            "int" => self.integral(node, &q, &operands),
            "sum" | "product" => self.big_operator(node, &name, &q, &operands),
            "limit" => self.limit(node, &q, &operands),
            _ => self.unsupported(op, &name, &operands),
        }
    }

    /// A function that is not elementary applied to its operands: `f(x)`, `g(x, y)`.
    fn application(&mut self, op: Xml, operands: &[Xml]) -> R<Tex> {
        let head = match op.tag_name().name() {
            "ci" if Self::elements(op).is_empty() => {
                let name = text_of(op);
                if name.chars().count() > 1 && name.chars().all(char::is_alphanumeric) {
                    format!(r"\operatorname{{{name}}}")
                } else {
                    identifier(&name).s
                }
            }
            _ => self.expression(op)?.wrapped(false),
        };
        let parts = self.all(operands)?;
        let inside: Vec<String> = parts.iter().map(|p| p.s.clone()).collect();
        let tall = parts.iter().any(|p| p.tall);
        let s = if tall { format!(r"{head}\left( {} \right)", inside.join(", ")) } else { format!("{head}({})", inside.join(", ")) };
        Ok(Tex::new(s, Prec::Atom).tall(tall))
    }

    /// `diff` and `partialdiff`: `\frac{dy}{dx}`, `\frac{d^{2}}{dx^{2}} (…)`; without a bound
    /// variable, the prime of a function (`f'`).
    fn derivative(&mut self, node: Xml, name: &str, q: &Qualifiers, operands: &[Xml]) -> R<Tex> {
        let [f]: [Tex; 1] = self.operands(node, name, operands, 1)?.try_into().expect("one operand");
        if q.bvars.is_empty() {
            return Ok(Tex::new(format!("{}'", f.wrapped(f.prec < Prec::Atom || f.negative)), Prec::Pow));
        }
        let d = if name == "partialdiff" { r"\partial" } else { "d" };
        let mut denominator = Vec::new();
        let mut orders = Vec::new();
        for b in &q.bvars {
            let (x, k) = self.bvar(*b)?;
            match k {
                Some(k) => {
                    denominator.push(format!("{d} {}^{{{}}}", x.s, k.s));
                    orders.push(k.s);
                }
                None => {
                    denominator.push(format!("{d} {}", x.s));
                    orders.push("1".into());
                }
            }
        }
        // the order on the d of the numerator: the <degree> given, or the sum of the orders when they are numbers
        let order = match self.inside(q.degree)? {
            Some(n) => Some(n.s),
            None if orders.len() == 1 => Some(orders[0].clone()),
            None => orders.iter().map(|o| o.parse::<u32>().ok()).sum::<Option<u32>>().map(|n| n.to_string()),
        };
        let top = match order.as_deref() {
            None | Some("1") => d.to_string(),
            Some(n) => format!("{d}^{{{n}}}"),
        };
        let denominator = denominator.join(" ");
        // a variable goes in the numerator (dy/dx); anything else after the fraction
        Ok(if f.kind == Kind::Letter && f.prec == Prec::Atom {
            Tex::new(format!(r"\frac{{{top} {}}}{{{denominator}}}", f.s), Prec::Atom).tall(true)
        } else {
            Tex::new(format!(r"\frac{{{top}}}{{{denominator}}} {}", f.wrapped(f.prec < Prec::Pow || f.negative)), Prec::Mul).tall(true)
        })
    }

    /// The subscript and superscript of an integral or a sum: `lowlimit`/`uplimit`, an `interval`, a
    /// `condition` or a `domainofapplication`. With `variable`, the lower limit is `i = 1`.
    fn limits(&mut self, q: &Qualifiers, variable: Option<&Tex>) -> R<(Option<String>, Option<String>)> {
        let (mut low, mut up) = (self.inside(q.lowlimit)?.map(|t| t.s), self.inside(q.uplimit)?.map(|t| t.s));
        if let Some(interval) = q.interval {
            let ends = Self::elements(interval);
            if let [a, b] = ends.as_slice() {
                low = Some(self.expression(*a)?.s);
                up = Some(self.expression(*b)?.s);
            }
        }
        if let (Some(l), Some(x)) = (&low, variable) {
            low = Some(format!("{} = {l}", x.s));
        }
        if low.is_none() {
            low = match (self.inside(q.condition)?, self.inside(q.domain)?) {
                (Some(c), _) => Some(c.s),
                (None, Some(d)) => Some(d.s),
                (None, None) => None,
            };
        }
        Ok((low, up))
    }

    fn integral(&mut self, node: Xml, q: &Qualifiers, operands: &[Xml]) -> R<Tex> {
        let [f]: [Tex; 1] = self.operands(node, "int", operands, 1)?.try_into().expect("one operand");
        let (low, up) = self.limits(q, None)?;
        let mut s = String::from(r"\int");
        if let Some(l) = low {
            s.push_str(&format!("_{{{l}}}"));
        }
        if let Some(u) = up {
            s.push_str(&format!("^{{{u}}}"));
        }
        s.push(' ');
        s.push_str(&f.wrapped(f.prec <= Prec::Add));
        for b in &q.bvars {
            let (x, _) = self.bvar(*b)?;
            s.push_str(&format!(r" \, d{}{}", if x.s.starts_with('\\') { " " } else { "" }, x.s));
        }
        Ok(Tex::new(s, Prec::Mul).tall(f.tall))
    }

    fn big_operator(&mut self, node: Xml, name: &str, q: &Qualifiers, operands: &[Xml]) -> R<Tex> {
        let [f]: [Tex; 1] = self.operands(node, name, operands, 1)?.try_into().expect("one operand");
        let variable = match q.bvars.first() {
            Some(b) => Some(self.bvar(*b)?.0),
            None => None,
        };
        let (low, up) = self.limits(q, variable.as_ref())?;
        let mut s = String::from(if name == "sum" { r"\sum" } else { r"\prod" });
        if let Some(l) = low {
            s.push_str(&format!("_{{{l}}}"));
        }
        if let Some(u) = up {
            s.push_str(&format!("^{{{u}}}"));
        }
        s.push(' ');
        s.push_str(&f.wrapped(f.prec <= Prec::Add));
        Ok(Tex::new(s, Prec::Mul).tall(f.tall))
    }

    /// `\lim_{x \to a}`: a bound variable and a `lowlimit`, or a `condition` (`<tendsto/>`).
    fn limit(&mut self, node: Xml, q: &Qualifiers, operands: &[Xml]) -> R<Tex> {
        let [f]: [Tex; 1] = self.operands(node, "limit", operands, 1)?.try_into().expect("one operand");
        let variable = match q.bvars.first() {
            Some(b) => Some(self.bvar(*b)?.0),
            None => None,
        };
        let below = match (variable, self.inside(q.lowlimit)?, self.inside(q.condition)?) {
            (Some(x), Some(a), _) => Some(format!(r"{} \to {}", x.s, a.s)),
            (_, _, Some(c)) => Some(c.s),
            _ => None,
        };
        let s = match below {
            Some(b) => format!(r"\lim_{{{b}}} {}", f.wrapped(f.prec <= Prec::Add)),
            None => format!(r"\lim {}", f.wrapped(f.prec <= Prec::Add)),
        };
        Ok(Tex::new(s, Prec::Mul).tall(f.tall))
    }

    /// `<set>`: its elements (`\{1, 2, 3\}`), or a bound variable and a condition (`\{x \mid x > 0\}`).
    fn set(&mut self, node: Xml) -> R<Tex> {
        let (q, operands) = Self::split(&Self::elements(node));
        let parts = self.all(&operands)?;
        let inside: Vec<String> = parts.iter().map(|p| p.s.clone()).collect();
        let s = match (q.bvars.first(), self.inside(q.condition)?) {
            (Some(b), Some(c)) => {
                let (x, _) = self.bvar(*b)?;
                let element = if inside.is_empty() { x.s } else { inside.join(", ") };
                format!(r"\{{{element} \mid {}\}}", c.s)
            }
            _ => format!(r"\{{{}\}}", inside.join(", ")),
        };
        Ok(Tex::new(s, Prec::Atom))
    }

    /// `<interval closure="…">`: closed `[a, b]` (the default), open `(a, b)`, `open-closed`, `closed-open`.
    fn interval(&mut self, node: Xml) -> R<Tex> {
        let [a, b]: [Tex; 2] = self.operands(node, "interval", &Self::elements(node), 2)?.try_into().expect("two operands");
        let (l, r) = match node.attribute("closure").unwrap_or("closed") {
            "open" => ("(", ")"),
            "open-closed" => ("(", "]"),
            "closed-open" => ("[", ")"),
            _ => ("[", "]"),
        };
        Ok(Tex::new(format!("{l}{}, {}{r}", a.s, b.s), Prec::Atom))
    }
}

/// A variable or a symbol by its text: one letter as is (`x`, `θ`), a longer name in italic as one
/// identifier (`\mathit{speed}`), anything else escaped.
fn identifier(t: &str) -> Tex {
    let mut chars = t.chars();
    let s = match (chars.next(), chars.next()) {
        (None, _) => "{}".to_string(),
        (Some(c), None) if c.is_alphabetic() => t.to_string(),
        _ if t.chars().all(char::is_alphanumeric) => format!(r"\mathit{{{t}}}"),
        _ => format!(r"\mathrm{{{}}}", escape(t)),
    };
    Tex::new(s, Prec::Atom).kind(Kind::Letter)
}

/// A token for LaTeX: its special characters escaped.
fn escape(t: &str) -> String {
    let mut s = String::new();
    for c in t.chars() {
        match c {
            '#' | '$' | '%' | '&' | '_' | '{' | '}' => {
                s.push('\\');
                s.push(c);
            }
            '\\' => s.push_str(r"\backslash "),
            '^' => s.push_str(r"\wedge "),
            '~' => s.push_str(r"\sim "),
            other => s.push(other),
        }
    }
    s
}

/// A base in braces when it is more than one token, so the script takes all of it.
fn group(base: &str) -> String {
    if base.chars().count() == 1 || (base.starts_with('\\') && base[1..].chars().all(|c| c.is_ascii_alphabetic())) {
        base.to_string()
    } else {
        format!("{{{base}}}")
    }
}

/// An elementary function applied: `\sin x`, `\sin(x + 1)`, `\max(a, b)`.
fn applied(head: &str, parts: &[Tex]) -> Tex {
    match parts {
        [x] if matches!(x.kind, Kind::Letter | Kind::Number) && x.prec == Prec::Atom => {
            Tex::new(format!("{head} {}", x.s), Prec::Mul).kind(Kind::Function { head: head.to_string(), argument: x.s.clone() })
        }
        [x] => {
            let argument = x.wrapped(true);
            Tex::new(format!("{head}{argument}"), Prec::Mul).tall(x.tall).kind(Kind::Function { head: head.to_string(), argument })
        }
        _ => {
            let inside: Vec<String> = parts.iter().map(|p| p.s.clone()).collect();
            let tall = parts.iter().any(|p| p.tall);
            let s = if tall { format!(r"{head}\left( {} \right)", inside.join(", ")) } else { format!("{head}({})", inside.join(", ")) };
            Tex::new(s, Prec::Atom).tall(tall)
        }
    }
}

/// `base^{exponent}`: a function's power on its name (`\sin^{2} x`), a base that is not a single
/// thing in parentheses (`(a + b)^{2}`, `(-x)^{2}`, `\left( \frac{1}{2} \right)^{2}`).
fn power(base: &Tex, exponent: &str) -> Tex {
    if let Kind::Function { head, argument } = &base.kind {
        return Tex::new(format!("{head}^{{{exponent}}} {argument}"), Prec::Mul).tall(base.tall);
    }
    let wrap = base.prec < Prec::Atom || base.negative || base.tall;
    let kind = if base.kind == Kind::Letter && !wrap { Kind::Letter } else { Kind::Other };
    Tex::new(format!("{}^{{{exponent}}}", base.wrapped(wrap)), Prec::Pow).tall(base.tall).kind(kind)
}

/// `times`: side by side when it reads like algebra (`2x`, `\frac{1}{2} x`, `2 \pi r`, `a b`,
/// `3 \sin x`): letters, after one number or other single thing, with a function last. With a dot
/// otherwise (`(a + b) \cdot c`, `2 \cdot 3`, `x \cdot 2`).
fn product(parts: &[Tex]) -> Tex {
    let last = parts.len().saturating_sub(1);
    let side_by_side = parts.len() >= 2
        && parts.iter().enumerate().all(|(i, p)| {
            !p.negative
                && match p.kind {
                    Kind::Letter => true,
                    Kind::Function { .. } => i == last,
                    Kind::Number | Kind::Other => i == 0 && p.prec == Prec::Atom,
                }
        });
    let s: Vec<String> = parts.iter().enumerate().map(|(i, p)| p.wrapped(p.prec < Prec::Mul || (p.negative && i > 0))).collect();
    let negative = parts.first().is_some_and(|p| p.negative);
    Tex { s: s.join(if side_by_side { " " } else { r" \cdot " }), prec: Prec::Mul, negative, tall: parts.iter().any(|p| p.tall), kind: Kind::Other }
}
