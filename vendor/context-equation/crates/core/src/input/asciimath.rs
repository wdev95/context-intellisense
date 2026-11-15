//! Reader for AsciiMath (asciimath.org): `sum_(i=1)^n i^2 = (n(n+1)(2n+1))/6`. It builds the same tree of
//! meaning as the LaTeX reader. Symbols are given by their LaTeX spelling and read by the LaTeX reader,
//! so both notations draw and speak a symbol the same way.
//!
//! The grammar follows asciimath.org: an expression is a sequence of `I` or `I/I`; `I` is `S`, `S_S`,
//! `S^S` or `S_S^S`; `S` is a symbol, a bracketed expression, `unary S` or `binary S S`. Brackets around
//! a fraction's parts, a script or an argument are removed: `(a+b)/2` has no parentheses on top.

use crate::locale::Locale;
use crate::rules::{self, Registry};
use crate::tree::*;
use crate::Error;

use super::latex::{self, NESTING_LIMIT};
use super::symbols;

type R<T> = Result<T, Error>;

pub fn read_with(source: &str, locale: Locale, registry: &mut Registry, strict: bool) -> R<Node> {
    let tokens = tokenize(source, locale);
    let mut parser = Parser { tokens, pos: 0, locale, registry, strict, depth: 0 };
    let mut children = Vec::new();
    loop {
        children.extend(parser.expression(false)?);
        // an expression stops only at the end or at a closing bracket, here one nothing opened
        let Some(&Token { kind: Kind::Right(c), col }) = parser.peek() else { break };
        parser.pos += 1;
        parser.tolerate(rules::E_REC_5, col, format!("'{}' closes nothing", c.map_or(":)".into(), String::from)), "drawn as is")?;
        if let Some(c) = c {
            children.push(symbol_node(c));
        }
    }
    Ok(Node::row(children))
}

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    /// A symbol, with its LaTeX spelling.
    Symbol(&'static str),
    Number(String),
    Letter(char),
    Text(String),
    /// `color(red)`, with the raw color.
    Color(String),
    Unary(&'static str),
    Binary(&'static str),
    /// Opening bracket; `None` is the invisible `{:`.
    Left(Option<char>),
    /// Closing bracket; `None` is the invisible `:}`.
    Right(Option<char>),
    /// `|` and `||`: opening or closing, decided by what follows.
    Bar(char),
    Over,
    Sub,
    Sup,
}

#[derive(Debug, Clone)]
struct Token {
    kind: Kind,
    col: usize,
}

/// (AsciiMath, LaTeX) for every symbol. The tokenizer takes the longest match.
const SYMBOLS: &[(&str, &str)] = &[
    // operations
    ("+", "+"),
    ("-", "-"),
    ("*", "\\cdot"),
    ("**", "\\ast"),
    ("***", "\\star"),
    ("//", "/"),
    ("\\\\", "\\backslash"),
    ("setminus", "\\setminus"),
    ("xx", "\\times"),
    ("|><", "\\ltimes"),
    ("><|", "\\rtimes"),
    ("|><|", "\\bowtie"),
    ("-:", "\\div"),
    ("divide", "\\div"),
    ("@", "\\circ"),
    ("o+", "\\oplus"),
    ("ox", "\\otimes"),
    ("o.", "\\odot"),
    ("sum", "\\sum"),
    ("prod", "\\prod"),
    ("^^", "\\wedge"),
    ("^^^", "\\bigwedge"),
    ("vv", "\\vee"),
    ("vvv", "\\bigvee"),
    ("nn", "\\cap"),
    ("nnn", "\\bigcap"),
    ("uu", "\\cup"),
    ("uuu", "\\bigcup"),
    // relations
    ("=", "="),
    ("!=", "\\ne"),
    ("<", "<"),
    (">", ">"),
    ("<=", "\\le"),
    (">=", "\\ge"),
    ("lt", "<"),
    ("gt", ">"),
    ("le", "\\le"),
    ("ge", "\\ge"),
    ("mlt", "\\ll"),
    ("mgt", "\\gg"),
    ("-<", "\\prec"),
    ("-<=", "\\preceq"),
    (">-", "\\succ"),
    (">-=", "\\succeq"),
    ("in", "\\in"),
    ("!in", "\\notin"),
    ("sub", "\\subset"),
    ("sup", "\\supset"),
    ("sube", "\\subseteq"),
    ("supe", "\\supseteq"),
    ("-=", "\\equiv"),
    ("~=", "\\cong"),
    ("~~", "\\approx"),
    ("~", "\\sim"),
    ("prop", "\\propto"),
    // logic
    ("and", "\\text{ and }"),
    ("or", "\\text{ or }"),
    ("not", "\\neg"),
    // AsciiMath's => and iff are the short arrows ⇒ and ⇔, not amsmath's spaced long ones (#250)
    ("=>", "\\Rightarrow"),
    ("if", "\\text{ if }"),
    ("iff", "\\Leftrightarrow"),
    ("AA", "\\forall"),
    ("EE", "\\exists"),
    ("_|_", "\\bot"),
    ("TT", "\\top"),
    ("|--", "\\vdash"),
    ("|==", "\\models"),
    // miscellaneous
    ("int", "\\int"),
    ("oint", "\\oint"),
    ("del", "\\partial"),
    ("grad", "\\nabla"),
    ("+-", "\\pm"),
    ("-+", "\\mp"),
    ("O/", "\\emptyset"),
    ("oo", "\\infty"),
    ("aleph", "\\aleph"),
    (":.", "\\therefore"),
    (":'", "\\because"),
    ("/_", "\\angle"),
    ("/_\\", "\\triangle"),
    ("'", "'"),
    // the LaTeX for "\\ " is the same space, written so that trimming cannot cut it
    ("\\ ", "~"),
    ("frown", "\\frown"),
    ("quad", "\\quad"),
    ("qquad", "\\qquad"),
    ("cdots", "\\cdots"),
    ("vdots", "\\vdots"),
    ("ddots", "\\ddots"),
    ("ldots", "\\ldots"),
    ("...", "\\ldots"),
    ("diamond", "\\diamond"),
    ("square", "\\square"),
    ("|__", "\\lfloor"),
    ("__|", "\\rfloor"),
    ("|~", "\\lceil"),
    ("~|", "\\rceil"),
    ("CC", "\\mathbb{C}"),
    ("NN", "\\mathbb{N}"),
    ("QQ", "\\mathbb{Q}"),
    ("RR", "\\mathbb{R}"),
    ("ZZ", "\\mathbb{Z}"),
    ("dx", "d x"),
    ("dy", "d y"),
    ("dz", "d z"),
    ("dt", "d t"),
    ("hbar", "\\hbar"),
    ("!", "!"),
    (",", ","),
    (";", ";"),
    (":", ":"),
    (".", "."),
    // functions
    ("sin", "\\sin"),
    ("cos", "\\cos"),
    ("tan", "\\tan"),
    ("sec", "\\sec"),
    ("csc", "\\csc"),
    ("cot", "\\cot"),
    ("arcsin", "\\arcsin"),
    ("arccos", "\\arccos"),
    ("arctan", "\\arctan"),
    ("sinh", "\\sinh"),
    ("cosh", "\\cosh"),
    ("tanh", "\\tanh"),
    ("sech", "\\sech"),
    ("csch", "\\csch"),
    ("coth", "\\coth"),
    ("exp", "\\exp"),
    ("log", "\\log"),
    ("ln", "\\ln"),
    ("det", "\\det"),
    ("dim", "\\dim"),
    ("mod", "\\operatorname{mod}"),
    ("gcd", "\\gcd"),
    ("lcm", "\\operatorname{lcm}"),
    ("lub", "\\operatorname{lub}"),
    ("glb", "\\operatorname{glb}"),
    ("min", "\\min"),
    ("max", "\\max"),
    ("lim", "\\lim"),
    ("Lim", "\\operatorname*{Lim}"),
    // arrows
    ("uarr", "\\uparrow"),
    ("darr", "\\downarrow"),
    ("rarr", "\\rightarrow"),
    ("->", "\\to"),
    (">->", "\\rightarrowtail"),
    ("->>", "\\twoheadrightarrow"),
    (">->>", "\\twoheadrightarrow"),
    ("|->", "\\mapsto"),
    ("larr", "\\leftarrow"),
    ("harr", "\\leftrightarrow"),
    ("rArr", "\\Rightarrow"),
    ("lArr", "\\Leftarrow"),
    ("hArr", "\\Leftrightarrow"),
    // Greek
    ("alpha", "\\alpha"),
    ("beta", "\\beta"),
    ("gamma", "\\gamma"),
    ("Gamma", "\\Gamma"),
    ("delta", "\\delta"),
    ("Delta", "\\Delta"),
    ("epsilon", "\\epsilon"),
    ("epsi", "\\epsilon"),
    ("varepsilon", "\\varepsilon"),
    ("zeta", "\\zeta"),
    ("eta", "\\eta"),
    ("theta", "\\theta"),
    ("Theta", "\\Theta"),
    ("vartheta", "\\vartheta"),
    ("iota", "\\iota"),
    ("kappa", "\\kappa"),
    ("lambda", "\\lambda"),
    ("Lambda", "\\Lambda"),
    ("mu", "\\mu"),
    ("nu", "\\nu"),
    ("xi", "\\xi"),
    ("Xi", "\\Xi"),
    ("pi", "\\pi"),
    ("Pi", "\\Pi"),
    ("rho", "\\rho"),
    ("sigma", "\\sigma"),
    ("Sigma", "\\Sigma"),
    ("tau", "\\tau"),
    ("upsilon", "\\upsilon"),
    ("phi", "\\phi"),
    ("Phi", "\\Phi"),
    ("varphi", "\\varphi"),
    ("chi", "\\chi"),
    ("psi", "\\psi"),
    ("Psi", "\\Psi"),
    ("omega", "\\omega"),
    ("Omega", "\\Omega"),
    // the TeX names asciimath.org accepts too
    ("<=>", "\\iff"),
    ("cdot", "\\cdot"),
    ("ast", "\\ast"),
    ("star", "\\star"),
    ("backslash", "\\backslash"),
    ("times", "\\times"),
    ("div", "\\div"),
    ("ltimes", "\\ltimes"),
    ("rtimes", "\\rtimes"),
    ("bowtie", "\\bowtie"),
    ("circ", "\\circ"),
    ("oplus", "\\oplus"),
    ("otimes", "\\otimes"),
    ("odot", "\\odot"),
    ("wedge", "\\wedge"),
    ("bigwedge", "\\bigwedge"),
    ("vee", "\\vee"),
    ("bigvee", "\\bigvee"),
    ("cap", "\\cap"),
    ("bigcap", "\\bigcap"),
    ("cup", "\\cup"),
    ("bigcup", "\\bigcup"),
    ("partial", "\\partial"),
    ("nabla", "\\nabla"),
    ("pm", "\\pm"),
    ("mp", "\\mp"),
    ("emptyset", "\\emptyset"),
    ("infty", "\\infty"),
    ("therefore", "\\therefore"),
    ("because", "\\because"),
    ("angle", "\\angle"),
    ("triangle", "\\triangle"),
    ("lfloor", "\\lfloor"),
    ("rfloor", "\\rfloor"),
    ("lceiling", "\\lceil"),
    ("rceiling", "\\rceil"),
    ("ne", "\\ne"),
    ("ll", "\\ll"),
    ("gg", "\\gg"),
    ("prec", "\\prec"),
    ("preceq", "\\preceq"),
    ("succ", "\\succ"),
    ("succeq", "\\succeq"),
    ("notin", "\\notin"),
    ("subset", "\\subset"),
    ("supset", "\\supset"),
    ("subseteq", "\\subseteq"),
    ("supseteq", "\\supseteq"),
    ("equiv", "\\equiv"),
    ("cong", "\\cong"),
    ("approx", "\\approx"),
    ("propto", "\\propto"),
    ("neg", "\\neg"),
    ("implies", "\\implies"),
    ("forall", "\\forall"),
    ("exists", "\\exists"),
    ("bot", "\\bot"),
    ("top", "\\top"),
    ("vdash", "\\vdash"),
    ("models", "\\models"),
    ("uparrow", "\\uparrow"),
    ("downarrow", "\\downarrow"),
    ("rightarrow", "\\rightarrow"),
    ("to", "\\to"),
    ("rightarrowtail", "\\rightarrowtail"),
    ("twoheadrightarrow", "\\twoheadrightarrow"),
    ("twoheadrightarrowtail", "\\twoheadrightarrow"),
    ("mapsto", "\\mapsto"),
    ("leftarrow", "\\leftarrow"),
    ("leftrightarrow", "\\leftrightarrow"),
    ("Rightarrow", "\\Rightarrow"),
    ("Leftarrow", "\\Leftarrow"),
    ("Leftrightarrow", "\\Leftrightarrow"),
];

/// Commands with one argument.
const UNARY: &[&str] = &[
    "sqrt",
    "abs",
    "floor",
    "ceil",
    "norm",
    "hat",
    "bar",
    "overline",
    "ul",
    "underline",
    "vec",
    "tilde",
    "dot",
    "ddot",
    "overarc",
    "overparen",
    "underparen",
    "ubrace",
    "underbrace",
    "obrace",
    "overbrace",
    "cancel",
    "bb",
    "mathbf",
    "bbb",
    "mathbb",
    "cc",
    "mathcal",
    "tt",
    "mathtt",
    "fr",
    "mathfrak",
    "sf",
    "mathsf",
    "rm",
    "mathrm",
];

/// Commands with two arguments.
const BINARY: &[&str] = &["frac", "root", "stackrel", "overset", "underset"];

/// Brackets: (text, opening?, drawn delimiter).
const BRACKETS: &[(&str, bool, Option<char>)] = &[
    ("(:", true, Some('⟨')),
    (":)", false, Some('⟩')),
    ("<<", true, Some('⟨')),
    (">>", false, Some('⟩')),
    ("langle", true, Some('⟨')),
    ("rangle", false, Some('⟩')),
    ("{:", true, None),
    (":}", false, None),
    ("(", true, Some('(')),
    (")", false, Some(')')),
    ("[", true, Some('[')),
    ("]", false, Some(']')),
    ("{", true, Some('{')),
    ("}", false, Some('}')),
];

fn tokenize(source: &str, locale: Locale) -> Vec<Token> {
    let c: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    let starts = |i: usize, s: &str| c[i..].iter().take(s.chars().count()).copied().eq(s.chars());
    while i < c.len() {
        let col = i + 1;
        if c[i].is_whitespace() {
            i += 1;
            continue;
        }
        // "quoted text"
        if c[i] == '"' {
            let end = c[i + 1..].iter().position(|&x| x == '"').map_or(c.len(), |p| p + i + 1);
            tokens.push(Token { kind: Kind::Text(c[i + 1..end].iter().collect()), col });
            i = (end + 1).min(c.len());
            continue;
        }
        // numbers: digits with a decimal point; a decimal comma in the languages that write it (L-NUM-1)
        if c[i].is_ascii_digit() || (c[i] == '.' && c.get(i + 1).is_some_and(char::is_ascii_digit)) {
            let mut value = String::new();
            while i < c.len() {
                let decimal = (c[i] == '.' || (c[i] == ',' && locale.comma_is_decimal())) && c.get(i + 1).is_some_and(char::is_ascii_digit) && !value.contains('.');
                if c[i].is_ascii_digit() {
                    value.push(c[i]);
                } else if decimal {
                    value.push('.');
                } else {
                    break;
                }
                i += 1;
            }
            tokens.push(Token { kind: Kind::Number(value), col });
            continue;
        }
        // text(…), mbox(…) and color(…): the argument is raw
        let raw_commands = ["text", "mbox", "color"];
        if let Some(name) = raw_commands.iter().find(|n| starts(i, n)) {
            let mut k = i + name.len();
            while c.get(k).is_some_and(|x| x.is_whitespace()) {
                k += 1;
            }
            if let Some(&open) = c.get(k).filter(|x| matches!(x, '(' | '[' | '{')) {
                let close = match open {
                    '(' => ')',
                    '[' => ']',
                    _ => '}',
                };
                let end = c[k + 1..].iter().position(|&x| x == close).map_or(c.len(), |p| p + k + 1);
                let raw: String = c[k + 1..end].iter().collect();
                tokens.push(Token { kind: if *name == "color" { Kind::Color(raw) } else { Kind::Text(raw) }, col });
                i = (end + 1).min(c.len());
                continue;
            }
        }
        // the longest symbol, command or bracket that starts here
        let mut best: Option<(usize, Kind)> = None;
        let mut consider = |len: usize, kind: Kind| {
            if best.as_ref().is_none_or(|(l, _)| len > *l) {
                best = Some((len, kind));
            }
        };
        for (text, latex) in SYMBOLS {
            if starts(i, text) {
                consider(text.chars().count(), Kind::Symbol(latex));
            }
        }
        for name in UNARY {
            if starts(i, name) {
                consider(name.len(), Kind::Unary(name));
            }
        }
        for name in BINARY {
            if starts(i, name) {
                consider(name.len(), Kind::Binary(name));
            }
        }
        for (text, open, d) in BRACKETS {
            if starts(i, text) {
                consider(text.chars().count(), if *open { Kind::Left(*d) } else { Kind::Right(*d) });
            }
        }
        for (text, kind) in [("|", Kind::Bar('|')), ("||", Kind::Bar('‖')), ("/", Kind::Over), ("_", Kind::Sub), ("^", Kind::Sup)] {
            if starts(i, text) {
                consider(text.len(), kind);
            }
        }
        // nothing matched: one character, a letter or a symbol ("speed" is s p e e d)
        match best {
            Some((len, kind)) => {
                tokens.push(Token { kind, col });
                i += len;
            }
            None => {
                tokens.push(Token { kind: Kind::Letter(c[i]), col });
                i += 1;
            }
        }
    }
    tokens
}

struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    locale: Locale,
    registry: &'a mut Registry,
    strict: bool,
    depth: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn tolerate(&mut self, rule: &'static str, column: usize, problem: impl Into<String>, recovery: &str) -> R<()> {
        let problem = problem.into();
        if self.strict {
            return Err(Error::Syntax { column, message: problem });
        }
        self.registry.warn(rule, format!("column {column}: {problem}; {recovery}"));
        Ok(())
    }

    fn nested<T>(&mut self, col: usize, f: impl FnOnce(&mut Self) -> R<T>) -> R<T> {
        if self.depth >= NESTING_LIMIT {
            return Err(Error::Syntax { column: col, message: format!("nesting deeper than {NESTING_LIMIT} levels") });
        }
        self.depth += 1;
        let r = f(self);
        self.depth -= 1;
        r
    }

    /// `E ::= I E | I/I E`, up to the end, a closing bracket or (inside `|…|`) the closing bar.
    fn expression(&mut self, inside_bars: bool) -> R<Vec<Node>> {
        let mut out = Vec::new();
        loop {
            match self.peek().map(|t| &t.kind) {
                None | Some(Kind::Right(_)) => return Ok(out),
                Some(Kind::Bar(_)) if inside_bars => return Ok(out),
                _ => {}
            }
            let left = self.intermediate()?;
            if matches!(self.peek().map(|t| &t.kind), Some(Kind::Over)) {
                self.pos += 1;
                let right = if self.peek().is_some_and(|t| !matches!(t.kind, Kind::Right(_))) { self.intermediate()? } else { Node::row(vec![]) };
                out.push(Node::Fraction {
                    numerator_align: None,
                    num: Box::new(unbracket(left)),
                    den: Box::new(unbracket(right)),
                    style: FractionStyle::Auto,
                    rule: true,
                    thickness: None,
                });
            } else {
                out.push(left);
            }
        }
    }

    /// `I ::= S | S_S | S^S | S_S^S`.
    fn intermediate(&mut self) -> R<Node> {
        let base = self.simple()?;
        let mut sub = None;
        let mut sup = None;
        // f'(x): primes belong to what precedes them, as in LaTeX
        let mut primes = 0;
        while matches!(self.peek().map(|t| &t.kind), Some(Kind::Symbol("'"))) {
            self.pos += 1;
            primes += 1;
        }
        if primes > 0 {
            sup = Some(Node::Operator { symbol: ['′', '″', '‴'][primes.min(3) - 1], class: Class::Ord });
        }
        loop {
            match self.peek().map(|t| &t.kind) {
                Some(Kind::Sub) if sub.is_none() => {
                    self.pos += 1;
                    sub = Some(unbracket(self.simple()?));
                }
                Some(Kind::Sup) if sup.is_none() || primes > 0 && sup.as_ref().is_some_and(|s| matches!(s, Node::Operator { .. })) => {
                    self.pos += 1;
                    let arg = unbracket(self.simple()?);
                    // f'^2: the prime and then the exponent, as LaTeX reads it
                    sup = Some(match sup {
                        Some(prime) => Node::row(vec![prime, arg]),
                        None => arg,
                    });
                }
                _ => break,
            }
        }
        if sub.is_none() && sup.is_none() {
            return Ok(base);
        }
        // ubrace(a+b)_n: the script is the brace's label
        if let Node::Brace { base: b, above, label: None, bracket } = &base {
            let label = if *above { sup.clone() } else { sub.clone() };
            if let Some(label) = label {
                return Ok(Node::Brace { base: b.clone(), above: *above, label: Some(Box::new(label)), bracket: *bracket });
            }
        }
        Ok(Node::Scripts { base: Box::new(base), sup: sup.map(Box::new), sub: sub.map(Box::new) })
    }

    /// `S`: one symbol, a bracketed expression, `unary S` or `binary S S`.
    fn simple(&mut self) -> R<Node> {
        let Some(token) = self.peek().cloned() else {
            return Ok(Node::row(vec![]));
        };
        self.nested(token.col, |this| this.simple_unguarded(token))
    }

    fn simple_unguarded(&mut self, token: Token) -> R<Node> {
        let col = token.col;
        self.pos += 1;
        Ok(match token.kind {
            Kind::Symbol(tex) => latex_symbol(tex, self.locale),
            Kind::Number(value) => {
                if value.contains('.') && self.locale.comma_is_decimal() {
                    self.registry.note(rules::L_NUM_1, format!("'{value}': read as a decimal number"));
                }
                Node::Number { value, group: false, thousands: None, variant: Variant::Upright }
            }
            Kind::Letter(c) if c.is_alphabetic() => {
                let variant = if c.is_ascii_alphabetic() || ('α'..='ω').contains(&c) { Variant::Italic } else { Variant::Upright };
                Node::Identifier { name: c.to_string(), variant }
            }
            Kind::Letter(c) => symbol_node(c),
            Kind::Text(t) => Node::Text { parts: vec![TextPart::Text(t)] },
            Kind::Color(color) => {
                let base = self.simple()?;
                let valid = !color.is_empty() && color.len() < 40 && color.chars().all(|x| x.is_ascii_alphanumeric() || "#(),. ".contains(x));
                if valid {
                    Node::Colored { color: color.trim().to_string(), base: Box::new(unbracket(base)) }
                } else {
                    self.tolerate(rules::E_REC_9, col, format!("invalid color: {color}"), "drawn without color")?;
                    unbracket(base)
                }
            }
            Kind::Unary(name) => {
                let arg = self.simple()?;
                unary(name, arg)
            }
            Kind::Binary(name) => {
                let a = unbracket(self.simple()?);
                let b = unbracket(self.simple()?);
                match name {
                    "frac" => Node::Fraction { numerator_align: None, num: Box::new(a), den: Box::new(b), style: FractionStyle::Auto, rule: true, thickness: None },
                    "root" => Node::Root { radicand: Box::new(b), index: Some(Box::new(a)) },
                    "underset" => {
                        let class = class_of(&b);
                        Node::ForcedClass { class, base: Box::new(Node::Stacked { base: Box::new(b), above: None, below: Some(Box::new(a)) }) }
                    }
                    _ => {
                        let class = if name == "stackrel" { Class::Rel } else { class_of(&b) };
                        Node::ForcedClass { class, base: Box::new(Node::Stacked { base: Box::new(b), above: Some(Box::new(a)), below: None }) }
                    }
                }
            }
            Kind::Left(open) => {
                let inner = self.expression(false)?;
                match self.peek().map(|t| t.kind.clone()) {
                    Some(Kind::Right(close)) => {
                        self.pos += 1;
                        bracketed(open, close, inner)
                    }
                    _ => {
                        self.tolerate(rules::E_REC_5, col, format!("'{}' without a closing bracket", open.map_or("{:".into(), String::from)), "drawn as is")?;
                        let mut children: Vec<Node> = open.map(symbol_node).into_iter().collect();
                        children.extend(inner);
                        Node::row(children)
                    }
                }
            }
            Kind::Bar(bar) => {
                // |x| when a matching bar follows; otherwise a lone bar
                let saved = self.pos;
                let inner = self.expression(true)?;
                if matches!(self.peek().map(|t| &t.kind), Some(Kind::Bar(b)) if *b == bar) && !inner.is_empty() {
                    self.pos += 1;
                    Node::Delimited { open: Some(bar), close: Some(bar), parts: vec![Node::row(inner)], middles: vec![] }
                } else {
                    self.pos = saved;
                    Node::Operator { symbol: bar, class: Class::Ord }
                }
            }
            Kind::Sub | Kind::Sup => {
                // a script with no base: on an empty base, like {}_a in LaTeX
                let script = unbracket(self.simple()?);
                let (sup, sub) = if token.kind == Kind::Sup { (Some(script), None) } else { (None, Some(script)) };
                Node::Scripts { base: Box::new(Node::row(vec![])), sup: sup.map(Box::new), sub: sub.map(Box::new) }
            }
            Kind::Over => Node::Operator { symbol: '/', class: Class::Ord },
            Kind::Right(close) => {
                // sqrt), x^): where an argument or a script is expected, the argument is empty and
                // the bracket is left to what it closes
                self.pos -= 1;
                self.tolerate(rules::E_REC_7, col, format!("missing argument before '{}'", close.map_or(":}".into(), String::from)), "left empty")?;
                Node::row(vec![])
            }
        })
    }
}

/// A symbol given by its LaTeX spelling, read by the LaTeX reader.
fn latex_symbol(tex: &str, locale: Locale) -> Node {
    let mut scratch = Registry::default();
    latex::read_with(tex, locale, &mut scratch, true).map(Node::simplify).expect("every AsciiMath symbol is valid LaTeX")
}

fn symbol_node(c: char) -> Node {
    match symbols::character(c) {
        Some(s) => Node::Operator { symbol: s.symbol, class: s.class },
        None => Node::Operator { symbol: c, class: symbols::class_of_char(c).unwrap_or(Class::Ord) },
    }
}

fn class_of(n: &Node) -> Class {
    match n {
        Node::Operator { class, .. } => *class,
        Node::LargeOperator { .. } | Node::Function { .. } => Class::Op,
        _ => Class::Ord,
    }
}

/// Without the outer brackets: `(a+b)/2` has no parentheses on top, `x^(2n)` neither.
fn unbracket(n: Node) -> Node {
    match n {
        Node::Delimited { open, close, mut parts, middles } if plain_brackets(open, close) && parts.len() == 1 && middles.is_empty() => parts.pop().expect("one part").simplify(),
        other => other,
    }
}

/// Brackets `unbracket` takes off: parentheses, square brackets, braces, or none.
fn plain_brackets(open: Option<char>, close: Option<char>) -> bool {
    matches!(open, Some('(' | '[' | '{') | None) && matches!(close, Some(')' | ']' | '}') | None)
}

fn unary(name: &str, arg: Node) -> Node {
    let delimited = |o: char, c: char, arg: Node| Node::Delimited { open: Some(o), close: Some(c), parts: vec![unbracket(arg)], middles: vec![] };
    let accent = |accent: char, stretchy: bool, arg: Node| Node::Accent { base: Box::new(unbracket(arg)), accent, stretchy, below: false };
    let wide = |arg: &Node| !matches!(unbracket(arg.clone()), Node::Identifier { .. } | Node::Number { .. });
    match name {
        "sqrt" => Node::Root { radicand: Box::new(unbracket(arg)), index: None },
        "abs" => delimited('|', '|', arg),
        "norm" => delimited('‖', '‖', arg),
        "floor" => delimited('⌊', '⌋', arg),
        "ceil" => delimited('⌈', '⌉', arg),
        "hat" => {
            let w = wide(&arg);
            accent('\u{0302}', w, arg)
        }
        "tilde" => {
            let w = wide(&arg);
            accent('\u{0303}', w, arg)
        }
        "vec" => {
            if wide(&arg) {
                accent('→', true, arg)
            } else {
                accent('\u{20D7}', false, arg)
            }
        }
        "dot" => accent('\u{0307}', false, arg),
        "ddot" => accent('\u{0308}', false, arg),
        "overarc" | "overparen" => accent('⏜', true, arg),
        "underparen" => Node::Accent { base: Box::new(unbracket(arg)), accent: '⏝', stretchy: true, below: true },
        "bar" | "overline" => Node::Bar { base: Box::new(unbracket(arg)), above: true },
        "ul" | "underline" => Node::Bar { base: Box::new(unbracket(arg)), above: false },
        "ubrace" | "underbrace" => Node::Brace { base: Box::new(unbracket(arg)), above: false, label: None, bracket: false },
        "obrace" | "overbrace" => Node::Brace { base: Box::new(unbracket(arg)), above: true, label: None, bracket: false },
        "cancel" => Node::Canceled { base: Box::new(unbracket(arg)), shape: Cancel::Bar },
        font => {
            let variant = match font {
                "bb" | "mathbf" => Variant::Bold,
                "bbb" | "mathbb" => Variant::DoubleStruck,
                "cc" | "mathcal" => Variant::Calligraphic,
                "tt" | "mathtt" => Variant::Monospace,
                "fr" | "mathfrak" => Variant::Fraktur,
                "sf" | "mathsf" => Variant::SansSerif,
                _ => Variant::Upright,
            };
            with_variant(unbracket(arg), variant)
        }
    }
}

/// The letters of `node` in another alphabet (`bb`, `bbb`, `cc`…); quoted text too (`bb "AaBb"`).
fn with_variant(node: Node, variant: Variant) -> Node {
    match node {
        Node::Identifier { name, .. } => Node::Identifier { name, variant },
        // #203: digits in bold, sans-serif and typewriter (`bb(1)`)
        Node::Number { value, group, thousands, .. } => Node::Number { value, group, thousands, variant: variant.of_digits() },
        // the one part's text, as the guard asks
        Node::Text { parts } if matches!(parts.as_slice(), [TextPart::Text(_)]) => {
            Node::Identifier { name: parts.into_iter().filter_map(|p| if let TextPart::Text(t) = p { Some(t) } else { None }).collect(), variant }
        }
        Node::Row { children } => Node::Row { children: children.into_iter().map(|c| with_variant(c, variant)).collect() },
        Node::Scripts { base, sup, sub } => Node::Scripts { base: Box::new(with_variant(*base, variant)), sup, sub },
        other => other,
    }
}

/// A bracketed expression: a matrix when it holds bracketed rows of equal length separated by commas
/// (`[[a,b],[c,d]]`), `cases` with `{` and the invisible `:}`, or delimiters that grow with the content.
fn bracketed(open: Option<char>, close: Option<char>, inner: Vec<Node>) -> Node {
    if let Some(rows) = matrix_rows(&inner) {
        let shape = match (open, close) {
            (Some('('), Some(')')) => Some(TableKind::PMatrix),
            (Some('['), Some(']')) => Some(TableKind::BMatrix),
            (Some('{'), Some('}')) => Some(TableKind::BraceMatrix),
            (Some('{'), None) => Some(TableKind::Cases),
            (None, Some('}')) => Some(TableKind::RCases),
            (None, None) => Some(TableKind::Matrix),
            _ => None,
        };
        if let Some(shape) = shape {
            let (rows, verticals) = augmented(rows);
            return Node::Table { shape, rows, columns: vec![], verticals, dashed: vec![], horizontals: vec![], between: vec![], intertexts: vec![], stretch: 1.0, colsep: 500 };
        }
    }
    if open.is_none() && close.is_none() {
        return Node::row(inner);
    }
    Node::Delimited { open, close, parts: vec![Node::row(inner)], middles: vec![] }
}

/// An augmented matrix (`[[a,b,|,c],[d,e,|,f]]`): a column of bars in every row becomes a vertical rule.
fn augmented(rows: Vec<Vec<Node>>) -> (Vec<Vec<Node>>, Vec<usize>) {
    let bar = |n: &Node| matches!(n, Node::Operator { symbol: '|', .. });
    let ncols = rows.first().map_or(0, Vec::len);
    let bars: Vec<usize> = (0..ncols).filter(|&j| rows.iter().all(|r| r.get(j).is_some_and(bar))).collect();
    if bars.is_empty() {
        return (rows, vec![]);
    }
    let rows = rows.into_iter().map(|r| r.into_iter().enumerate().filter(|(j, _)| !bars.contains(j)).map(|(_, c)| c).collect()).collect();
    // the rule goes before the column that followed the bar, counted without the bar columns
    let verticals = bars.iter().enumerate().map(|(k, j)| j - k).collect();
    (rows, verticals)
}

fn matrix_rows(inner: &[Node]) -> Option<Vec<Vec<Node>>> {
    let comma = |n: &Node| matches!(n, Node::Operator { symbol: ',', .. });
    let mut rows: Vec<Vec<Node>> = Vec::new();
    let mut expect_row = true;
    let mut bracket: Option<(Option<char>, Option<char>)> = None;
    for n in inner {
        if expect_row {
            let Node::Delimited { open, close, parts, middles } = n else { return None };
            if !middles.is_empty() || parts.len() != 1 || !matches!(open, Some('(' | '[')) {
                return None;
            }
            // every row with the same brackets
            if bracket.get_or_insert((*open, *close)) != &(*open, *close) {
                return None;
            }
            let cells: Vec<Node> = parts[0].children().split(comma).map(|c| Node::row(c.to_vec()).simplify()).collect();
            rows.push(cells);
            expect_row = false;
        } else if comma(n) {
            expect_row = true;
        } else {
            return None;
        }
    }
    let ncols = rows.first()?.len();
    (!expect_row && rows.len() >= 2 && rows.iter().all(|r| r.len() == ncols)).then_some(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every symbol is read by the LaTeX reader in strict mode: none may fail at conversion time.
    #[test]
    fn every_symbol_is_valid_latex() {
        for (am, tex) in SYMBOLS {
            let mut registry = Registry::default();
            let r = latex::read_with(tex, Locale::En, &mut registry, true);
            assert!(r.is_ok(), "{am} → {tex}: {r:?}");
        }
    }
}
