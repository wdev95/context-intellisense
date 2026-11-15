//! Código Matemático Unificado para a Língua Portuguesa (CMU, MEC/SEESP, 2006). Section numbers (§) refer
//! to that edition, as transcribed by the Instituto Benjamin Constant; `spec/braille.md` lists the sources,
//! what is covered and what is not.
//!
//! The CMU writes on a single line, with no level indicators: an exponent or index is one term after its
//! sign (⠡, ⠌), and anything longer goes in the auxiliary parentheses ⠢…⠔, which exist only in braille
//! (§1.3). The same parentheses delimit numerators, denominators and radicands.

use super::{cells, letter, Braille};
use crate::tree::*;

pub fn transcribe(node: &Node) -> Braille {
    let mut w = Writer { out: String::new(), after_number: false, missing: Vec::new() };
    w.node(node);
    let text = w.out.trim_end_matches(['⠀', '\n']).to_string();
    Braille { text, missing: w.missing }
}

struct Writer {
    out: String,
    /// The last thing written was a number: a letter a–j after it takes ⠐ (§1.1 a).
    after_number: bool,
    missing: Vec<char>,
}

const AUX_OPEN: &str = "⠢";
const AUX_CLOSE: &str = "⠔";

impl Writer {
    fn push(&mut self, s: &str) {
        self.out.push_str(s);
        self.after_number = false;
    }

    fn dots(&mut self, d: &str) {
        self.push(&cells(d));
    }

    fn blank(&mut self) {
        if !self.out.is_empty() && !self.out.ends_with(['⠀', '\n']) {
            self.out.push('⠀');
        }
        self.after_number = false;
    }

    fn missing(&mut self, c: char) {
        if !self.missing.contains(&c) {
            self.missing.push(c);
        }
        self.push("⠿");
    }

    fn node(&mut self, node: &Node) {
        match node {
            // \mathchoice: the text style's branch
            Node::Choice { branches } => {
                if let Some(b) = branches.get(1) {
                    self.node(b);
                }
            }
            Node::Number { value, thousands, .. } => self.number(value, thousands.is_some()),
            Node::Identifier { name, variant } => self.identifier(name, *variant),
            Node::Function { name, .. } => self.function_name(name),
            Node::Operator { symbol, .. } | Node::FixedDelimiter { symbol, .. } => self.symbol(*symbol),
            Node::LargeOperator { symbol, .. } => self.large_operator(*symbol, None, None),
            Node::Row { children } => self.row(children),
            Node::Fraction { num, den, rule, .. } => self.fraction(num, den, *rule),
            Node::Root { radicand, index } => {
                // §5.3: ⠫ index ⠱ radicand; the square root omits the index
                self.dots("1246");
                if let Some(i) = index {
                    self.node(i);
                }
                self.dots("156");
                self.argument(radicand);
            }
            Node::Scripts { base, sup, sub } => self.scripts(base, sup.as_deref(), sub.as_deref()),
            Node::Delimited { open, close, parts, middles } => {
                if let Some(o) = open {
                    self.symbol(*o);
                }
                for (i, p) in parts.iter().enumerate() {
                    if i > 0 {
                        self.symbol(middles.get(i - 1).copied().unwrap_or('|'));
                    }
                    self.node(p);
                }
                if let Some(c) = close {
                    self.symbol(*c);
                }
            }
            Node::Table { shape, rows, .. } => self.table(*shape, rows),
            Node::Accent { base, accent, below, .. } => self.mark(base, *accent, *below),
            Node::Bar { base, above } => self.mark(base, '\u{0304}', !above),
            // no sign for a horizontal brace or bracket in the 2006 code
            Node::Brace { base, above, .. } => {
                self.node(base);
                self.missing(if *above { '⏞' } else { '⏟' });
            }
            Node::Text { parts } => {
                for p in parts {
                    match p {
                        TextPart::Text(t) => self.text(t),
                        TextPart::Math(m) => self.node(m),
                    }
                }
            }
            Node::Arrow { symbol, above, below } => {
                self.symbol(*symbol);
                if above.is_some() || below.is_some() {
                    self.missing('⟶');
                }
            }
            // "sobrescrito" ⠡⠡ and "subscrito" ⠌⠌: directly over and under (§2.2)
            Node::Stacked { base, above, below } => {
                self.node(base);
                let filled = |n: Option<&Node>| n.filter(|n| **n != Node::row(vec![])).cloned();
                if let Some(b) = filled(below.as_deref()) {
                    self.dots("34 34");
                    self.argument(&b);
                }
                if let Some(a) = filled(above.as_deref()) {
                    self.dots("16 16");
                    self.argument(&a);
                }
            }
            // the 2006 code has no sign for striking out or framing an expression
            Node::Canceled { base, shape } => {
                self.node(base);
                // the Feynman slash (\slashed) is the overlay of the slash, not a cancellation
                self.missing(if *shape == Cancel::Slash { '\u{0338}' } else { '╱' });
            }
            Node::Framed { base, shape: Frame::Rectangle, .. } => {
                self.node(base);
                self.missing('▭');
            }
            // the phasor A∠θ: the angle sign (§8.1), then the angle
            Node::Framed { base, shape: Frame::Phase, .. } => {
                self.dots("45 25");
                self.node(base);
            }
            // the code has no sign for the actuarial angle either
            Node::Framed { base, shape: Frame::ActuarialAngle, .. } => {
                self.node(base);
                self.missing('⌉');
            }
            Node::Framed { base, .. }
            | Node::Colored { base, .. }
            | Node::Attributed { base, .. }
            | Node::ForcedClass { base, .. }
            | Node::Style { base, .. }
            | Node::Multicolumn { base, .. }
            | Node::Raised { base, .. }
            | Node::Scaled { base, .. }
            | Node::Smashed { base, .. }
            | Node::VCenter { base }
            | Node::Resized { base, .. }
            | Node::Rotated { base, .. }
            | Node::Reflected { base }
            | Node::Overlapped { base, .. }
            | Node::Unit { base, .. }
            | Node::Quantity { unit: base, .. } => self.node(base),
            Node::VdotsWithin { .. } => self.node(&Node::VDOTS),
            Node::Space { .. } | Node::Phantom { .. } | Node::Rule { .. } => {}
            Node::VerticalArrow { symbol, .. } => self.missing(*symbol),
            #[cfg(feature = "chem")]
            Node::Chemistry { scheme } => self.node(&crate::chem::speech::as_nodes(scheme)),
            Node::ArrowDiagram { cells, arrows } => {
                // each arrow is one more line, after the rows of cells
                let lines: Vec<Vec<Node>> = cells.iter().cloned().chain(arrows.iter().map(|a| vec![a.as_row(cells)])).collect();
                self.table(TableKind::Diagram, &lines);
            }
            Node::LongDivision { dividend, divisor } => {
                self.number(dividend, false);
                self.dots("256");
                self.number(divisor, false);
            }
            Node::ColumnArithmetic { operation, terms } => {
                for (i, t) in terms.iter().enumerate() {
                    if i > 0 {
                        self.symbol(*operation);
                    }
                    self.number(t, false);
                }
            }
        }
    }

    fn row(&mut self, children: &[Node]) {
        let mut i = 0;
        while i < children.len() {
            // left indices (an isotope's ¹⁴₆C): the letter first, then ⠠⠌ lower and ⠈⠡ upper (§2.2, §2.4.2)
            if let (Node::Scripts { base, sup, sub }, Some(next)) = (&children[i], children.get(i + 1)) {
                if **base == Node::row(vec![]) {
                    let (letter, right_sup, right_sub) = match next {
                        Node::Scripts { base, sup, sub } => (base.as_ref(), sup.as_deref(), sub.as_deref()),
                        other => (other, None, None),
                    };
                    self.node(letter);
                    if let Some(s) = sub {
                        self.dots("6 34");
                        self.argument(s);
                    }
                    if let Some(s) = sup {
                        self.dots("4 16");
                        self.argument(s);
                    }
                    self.indices(right_sup, right_sub);
                    i += 2;
                    continue;
                }
            }
            // geometry: ∠ and a name of several points in auxiliary parentheses; △ and □ directly
            // followed by the points (§8.1)
            if let Node::Operator { symbol: s @ ('∠' | '△' | '□'), .. } = &children[i] {
                let points: Vec<&Node> = children[i + 1..].iter().take_while(|n| matches!(n, Node::Identifier { name, .. } if name.chars().count() == 1)).collect();
                if !points.is_empty() {
                    self.dots(match s {
                        '∠' => "45 25",
                        '△' => "6 23456",
                        _ => "456 13456",
                    });
                    let aux = *s == '∠' && points.len() > 1;
                    if aux {
                        self.push(AUX_OPEN);
                    }
                    for p in &points {
                        self.node(p);
                    }
                    if aux {
                        self.push(AUX_CLOSE);
                    }
                    i += 1 + points.len();
                    continue;
                }
            }
            // a mixed number: the whole part, then the fraction (§3.2.2: 2 3/4 = ⠼⠃⠼⠒⠙)
            if let (Node::Number { value, thousands, .. }, Some(f @ Node::Fraction { num, den, rule, .. })) = (&children[i], children.get(i + 1)) {
                if numeric_fraction(num, den, *rule).is_some() && !value.contains('.') {
                    self.number(value, thousands.is_some());
                    self.node(f);
                    i += 2;
                    continue;
                }
            }
            // ">" followed by the letter "o": the letter takes ⠐ (§4.2)
            if let (Node::Operator { symbol: '>', .. }, Some(Node::Identifier { name, .. })) = (&children[i], children.get(i + 1)) {
                if name == "o" {
                    self.symbol('>');
                    self.dots("5 135");
                    i += 2;
                    continue;
                }
            }
            self.node(&children[i]);
            i += 1;
        }
    }

    /// §3.1–3.2: ⠼ and the digits as the letters a–j; the decimal comma ⠂, the thousands point ⠄.
    fn number(&mut self, value: &str, thousands: bool) {
        let (integer, fraction) = value.split_once('.').map_or((value, None), |(a, b)| (a, Some(b)));
        let mut s = String::from("⠼");
        let n = integer.chars().count();
        for (i, d) in integer.chars().enumerate() {
            if thousands && i > 0 && (n - i) % 3 == 0 {
                s.push('⠄');
            }
            s.push_str(&cells(upper_digit(d)));
        }
        if let Some(f) = fraction {
            s.push('⠂');
            for d in f.chars() {
                s.push_str(&cells(upper_digit(d)));
            }
        }
        self.push(&s);
        self.after_number = true;
    }

    fn identifier(&mut self, name: &str, variant: Variant) {
        // the double-struck one (\mathbb{1}, 𝟙): the digit, as before it was a symbol; its typeform is an
        // open question for a transcriber (docs/braille-review.md)
        if name.len() == 1 && name.as_bytes()[0].is_ascii_digit() {
            return self.number(name, false);
        }
        let mut chars = name.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            if matches!(variant, Variant::Italic | Variant::Upright) {
                self.text(name);
            } else {
                // each letter of a word in a typographic variant takes its prefix
                for c in name.chars() {
                    self.letter_in_variant(c, variant);
                }
            }
            return;
        };
        // §3.5: the number sets ℕ ℤ ℚ ℝ ℂ
        if variant == Variant::DoubleStruck && matches!(c, 'N' | 'Z' | 'Q' | 'R' | 'C') {
            self.push(&format!("⠸{}", cells(letter(c).unwrap_or("123456"))));
            return;
        }
        self.letter_in_variant(c, variant);
    }

    fn letter_in_variant(&mut self, c: char, variant: Variant) {
        // §1.1: other typographic variants (Gothic, bold…): ⠠ before lowercase, ⠰ before capitals, and the
        // ⠰ stands for the capital sign ("$z" in the table: the letter itself, no ⠨)
        let variant_prefix = match variant {
            Variant::Italic | Variant::Upright => "",
            _ if c.is_uppercase() => "⠰",
            _ => "⠠",
        };
        let c = if variant_prefix == "⠰" && c.is_ascii_uppercase() { c.to_ascii_lowercase() } else { c };
        match self.letter_cells(c) {
            Some(l) => {
                // §1.1 a: a letter a–j right after a number takes ⠐, or it would read as a digit
                let dot5 = self.after_number && variant_prefix.is_empty() && ('a'..='j').contains(&c);
                let s = format!("{}{variant_prefix}{l}", if dot5 { "⠐" } else { "" });
                self.push(&s);
            }
            None => self.missing(c),
        }
    }

    /// Latin capitals with ⠨; Greek with ⠈, Greek capitals with ⠘ (§1.1, §1.2).
    fn letter_cells(&self, c: char) -> Option<String> {
        if c.is_ascii_alphabetic() {
            let l = cells(letter(c)?);
            return Some(if c.is_ascii_uppercase() { format!("⠨{l}") } else { l });
        }
        let lower = c.to_lowercase().next()?;
        let dots = match lower {
            'α' => "1",
            'β' => "12",
            'γ' => "1245",
            'δ' => "145",
            'ϵ' | 'ε' => "15",
            'ζ' => "1356",
            'η' => "156",
            'θ' | 'ϑ' => "1456",
            'ι' => "24",
            'κ' => "13",
            'λ' => "123",
            'μ' => "134",
            'ν' => "1345",
            'ξ' => "1346",
            'ο' => "135",
            'π' => "1234",
            'ρ' => "1235",
            'σ' | 'ς' => "234",
            'τ' => "2345",
            'υ' => "136",
            'ϕ' | 'φ' => "124",
            'χ' => "12346",
            'ψ' => "13456",
            'ω' => "2456",
            _ => return None,
        };
        Some(format!("{}{}", if c != lower { "⠘" } else { "⠈" }, cells(dots)))
    }

    /// §7.5: the name in letters (Brazilian names: sen, tg, cossec), ended by ⠄ (dot 3).
    fn function_name(&mut self, name: &str) {
        let written = match name {
            "sin" => "sen",
            "tan" => "tg",
            "cot" => "cotg",
            "csc" => "cossec",
            "arcsin" => "arc'sen",
            "arctan" => "arc'tg",
            "arccot" => "arc'cotg",
            "arccsc" => "arc'cossec",
            "sinh" => "sh",
            "cosh" => "ch",
            "tanh" => "th",
            "coth" => "cth",
            other => other,
        };
        let mut s = String::new();
        for c in written.chars() {
            match c {
                '\'' => s.push('⠄'),
                c => s.push_str(&letter(c).map(cells).unwrap_or_default()),
            }
        }
        s.push('⠄');
        self.push(&s);
    }

    fn text(&mut self, t: &str) {
        let mut digits = String::new();
        for c in t.chars().filter(|c| !crate::input::latex::MARKERS.contains(c)) {
            if c.is_ascii_digit() {
                digits.push(c);
                continue;
            }
            if !digits.is_empty() {
                self.number(&digits, false);
                digits.clear();
            }
            if c.is_whitespace() {
                // a space alone (\ , a MathML no-break space) is not a word break: the CMU writes no blanks
                // inside an expression
                if !t.trim().is_empty() {
                    self.blank();
                }
            } else if let Some(l) = self.letter_cells(c).or_else(|| portuguese_letter(c)) {
                self.push(&l);
            } else {
                self.symbol(c);
            }
        }
        if !digits.is_empty() {
            self.number(&digits, false);
        }
    }

    fn symbol(&mut self, c: char) {
        // #176: a symbol drawn by composition that is another one drawn another way (a short arrow)
        let c = crate::composite::get(c).and_then(|k| k.meaning).unwrap_or(c);
        if self.letter_cells(c).is_some() && !c.is_ascii() {
            let l = self.letter_cells(c).unwrap_or_default();
            self.push(&l);
            return;
        }
        match sign(c) {
            Some(d) => {
                // "portanto" and "posto que" have a blank on each side (§6.3); the bars and the comma are
                // followed by one (§1.3, §6.1)
                if matches!(c, '∴' | '∵') {
                    self.blank();
                }
                self.dots(d);
                if matches!(c, '|' | ',' | '∴' | '∵') {
                    self.blank();
                }
            }
            None => self.missing(c),
        }
    }

    /// A numerator, denominator, exponent, index or radicand: one term as is, more in ⠢…⠔ (§1.3).
    fn argument(&mut self, n: &Node) {
        if simple(n) {
            self.node(n);
        } else {
            self.push(AUX_OPEN);
            self.node(n);
            self.push(AUX_CLOSE);
        }
    }

    fn fraction(&mut self, num: &Node, den: &Node, rule: bool) {
        if !rule {
            // binomial coefficient (n over r): ⠒ between the two, inside the parentheses (§7.5.1)
            self.node(num);
            self.dots("25");
            self.node(den);
            return;
        }
        // §3.2.2: a numeric fraction, the numerator in the lower part of the cell after ⠼, the
        // denominator in the upper part without ⠼: 3/4 = ⠼⠒⠙
        if let Some((n, d)) = numeric_fraction(num, den, rule) {
            let lower: String = n.chars().map(|c| cells(lower_digit(c))).collect();
            let upper: String = d.chars().map(|c| cells(upper_digit(c))).collect();
            self.push(&format!("⠼{lower}{upper}"));
            self.after_number = true;
            return;
        }
        // §5.1: numerator ⠲ denominator, each in auxiliary parentheses when it has more than one term
        self.argument(num);
        self.dots("256");
        self.argument(den);
    }

    fn scripts(&mut self, base: &Node, sup: Option<&Node>, sub: Option<&Node>) {
        let base = base.operator_core();
        match base {
            Node::LargeOperator { symbol, .. } => return self.large_operator(*symbol, sub, sup),
            Node::Function { name, .. } if name == "lim" => {
                // §7.2: lim⠄ condition ⠱
                self.function_name("lim");
                if let Some(s) = sub {
                    self.node(s);
                }
                self.dots("156");
                return;
            }
            // max, min, sup…: in display style the limit is directly under the name, the "subscrito" ⠌⠌
            // (§2.1, §2.2); lim and log have their own forms below
            Node::Function { name, limits: true } if name != "lim" => {
                self.function_name(name);
                if let Some(s) = sub {
                    self.dots("34 34");
                    self.argument(s);
                }
                if let Some(s) = sup {
                    self.dots("16 16");
                    self.argument(s);
                }
                return;
            }
            Node::Function { name, .. } if name == "log" && sub.is_some() && sup.is_none() => {
                // §7.5.2: log⠄ base ⠱ x
                self.function_name("log");
                if let Some(s) = sub {
                    self.node(s);
                }
                self.dots("156");
                return;
            }
            _ => {}
        }
        self.node(base);
        self.indices(sup, sub);
    }

    /// Right marks and indices: primes ⠳ (§2.4) before the indices, the subscript before the superscript
    /// (§2.4.2).
    fn indices(&mut self, sup: Option<&Node>, sub: Option<&Node>) {
        let (primes, sup) = split_primes(sup);
        for _ in 0..primes {
            self.dots("1256");
        }
        if let Some(s) = sub {
            self.dots("34");
            self.argument(s);
        }
        if let Some(s) = sup {
            if matches!(s, Node::Operator { symbol: '°' | '∘', .. }) {
                // degree: 356 with no position sign (CIB 2024, §11.5; see spec/braille.md)
                self.dots("356");
            } else {
                self.dots("16");
                self.argument(&s);
            }
        }
    }

    /// Σ, Π, ∫: the sign, the lower bound, ⠒ and the upper bound, closed by ⠱ (§7.4, §7.5.1).
    fn large_operator(&mut self, c: char, lower: Option<&Node>, upper: Option<&Node>) {
        let sign = match c {
            '∑' => "45 234",
            '∏' => "45 1234",
            '∫' => "12346",
            '∬' => "12346 12346",
            '∭' => "12346 12346 12346",
            '∮' => "12346 356",
            '⋃' => "123456 345",
            '⋂' => "123456 156",
            // conjunction and disjunction "of larger size" (§6.2)
            '⋀' => "456 26",
            '⋁' => "456 24",
            other => {
                self.missing(other);
                return;
            }
        };
        self.dots(sign);
        if let Some(l) = lower {
            self.node(l);
        }
        if let Some(u) = upper {
            self.dots("25");
            self.node(u);
        }
        self.dots("156");
    }

    /// Marks over a letter are written before it (§2.3.2): bar ⠈⠉, vector ⠒⠂, left arrow ⠐⠒, tilde ⠐⠢,
    /// arc ⠈⠒, a dot ⠈ (with ⠐ before a Latin letter); more than one symbol goes in ⠢…⠔.
    fn mark(&mut self, base: &Node, accent: char, below: bool) {
        let dots = match (accent, below) {
            // the line segment with a tick at each end (KaTeX's \overlinesegment) as the bar of a segment
            ('\u{0304}' | '\u{20E9}', false) => "4 14",
            ('\u{0304}' | '\u{20E9}', true) => "6 36",
            ('\u{20D7}' | '→', false) => "25 2",
            ('←', false) => "5 25",
            ('\u{0303}', false) => "5 26",
            ('⏜' | '⏠', false) => "4 25",
            ('\u{0307}', false) => "4",
            ('\u{0308}', false) => "4 4",
            (other, _) => {
                self.node(base);
                self.missing(other);
                return;
            }
        };
        self.dots(dots);
        // a dotted Latin lowercase letter takes ⠐, or it would read as Greek (§1.1 b)
        if matches!(accent, '\u{0307}' | '\u{0308}') && matches!(base, Node::Identifier { name, .. } if name.chars().all(|c| c.is_ascii_lowercase())) {
            self.dots("5");
        }
        self.argument(base);
    }

    /// Matrices and systems: a line per row, in their print positions (§7.5.1), entries separated by a
    /// blank, the row's grouping signs at its ends.
    fn table(&mut self, shape: TableKind, rows: &[Vec<Node>]) {
        let (open, close) = match shape {
            TableKind::PMatrix => ("126", "345"),
            TableKind::BMatrix => ("12356", "23456"),
            TableKind::BraceMatrix => ("5 123", "456 2"),
            TableKind::VMatrix => ("456", "456"),
            TableKind::VVMatrix => ("456 123", "456 123"),
            TableKind::Cases => ("5 123", ""),
            TableKind::RCases => ("", "456 2"),
            _ => ("", ""),
        };
        if !self.out.is_empty() && !self.out.ends_with('\n') {
            self.out.push('\n');
        }
        for row in rows {
            if !open.is_empty() {
                self.dots(open);
            }
            for (j, cell) in row.iter().enumerate() {
                if j > 0 {
                    self.blank();
                }
                self.node(cell);
            }
            if !close.is_empty() {
                self.dots(close);
            }
            self.out.push('\n');
            self.after_number = false;
        }
    }
}

/// One term, with no auxiliary parentheses needed: a number, a letter, a symbol, a root, something
/// already in grouping signs, a numeric fraction, or one of these with its marks and indices.
fn simple(n: &Node) -> bool {
    match n {
        Node::Number { .. } | Node::Identifier { .. } | Node::Operator { .. } | Node::Root { .. } | Node::Function { .. } => true,
        Node::Delimited { open: Some(_), close: Some(_), .. } => true,
        Node::Fraction { num, den, rule, .. } => numeric_fraction(num, den, *rule).is_some(),
        Node::Scripts { base, sup, sub } => simple(base) && sup.as_deref().is_none_or(simple) && sub.as_deref().is_none_or(simple),
        Node::Accent { base, .. } | Node::Bar { base, .. } => simple(base),
        Node::Colored { base, .. } | Node::Attributed { base, .. } | Node::ForcedClass { base, .. } | Node::Style { base, .. } | Node::Unit { base, .. } => simple(base),
        // a signed term: x⁻¹ = ⠭⠡⠤⠼⠁ (§5.2)
        Node::Row { children } => match children.as_slice() {
            [one] => simple(one),
            [Node::Operator { symbol: '−' | '+', .. }, one] => simple(one),
            _ => bracketed(children),
        },
        _ => false,
    }
}

/// A row that is one group in ( ), [ ] or { }, as `(a+b)` arrives: the opening sign first, its closing
/// sign last, and the group closed nowhere before it (`(a)+(b)` is two terms). Already in grouping signs (§1.3).
fn bracketed(children: &[Node]) -> bool {
    let sign = |n: &Node| match n {
        Node::Operator { symbol: '(' | '[' | '{', .. } => 1,
        Node::Operator { symbol: ')' | ']' | '}', .. } => -1,
        _ => 0,
    };
    let pair = matches!(
        (children.first(), children.last()),
        (Some(Node::Operator { symbol: '(', .. }), Some(Node::Operator { symbol: ')', .. }))
            | (Some(Node::Operator { symbol: '[', .. }), Some(Node::Operator { symbol: ']', .. }))
            | (Some(Node::Operator { symbol: '{', .. }), Some(Node::Operator { symbol: '}', .. }))
    );
    let mut depth = 0;
    pair && children.iter().enumerate().all(|(i, n)| {
        depth += sign(n);
        (depth == 0) == (i + 1 == children.len())
    })
}

/// The digits of a fraction (its numerator, its denominator, whether it has a rule) of two whole numbers.
fn numeric_fraction(num: &Node, den: &Node, rule: bool) -> Option<(String, String)> {
    match (num, den) {
        (Node::Number { value: a, .. }, Node::Number { value: b, .. }) if rule && !a.contains('.') && !b.contains('.') => Some((a.clone(), b.clone())),
        _ => None,
    }
}

fn upper_digit(d: char) -> &'static str {
    match d {
        '1' => "1",
        '2' => "12",
        '3' => "14",
        '4' => "145",
        '5' => "15",
        '6' => "124",
        '7' => "1245",
        '8' => "125",
        '9' => "24",
        _ => "245",
    }
}

fn lower_digit(d: char) -> &'static str {
    match d {
        '1' => "2",
        '2' => "23",
        '3' => "25",
        '4' => "256",
        '5' => "26",
        '6' => "235",
        '7' => "2356",
        '8' => "236",
        '9' => "35",
        _ => "356",
    }
}

/// The accented letters of Portuguese braille, for words inside the equation.
fn portuguese_letter(c: char) -> Option<String> {
    let lower = c.to_lowercase().next()?;
    let dots = match lower {
        'á' => "12356",
        'à' => "1246",
        'â' => "16",
        'ã' => "345",
        'é' => "123456",
        'ê' => "126",
        'í' => "34",
        'ó' => "346",
        'ô' => "1456",
        'õ' => "246",
        'ú' => "23456",
        'ü' => "1256",
        'ç' => "12346",
        _ => return None,
    };
    Some(format!("{}{}", if c != lower { "⠨" } else { "" }, cells(dots)))
}

/// Signs of operations (§4.1), relations (§4.2–4.3, §6), grouping (§1.3) and logic (§6.2).
fn sign(c: char) -> Option<&'static str> {
    Some(match c {
        '+' => "235",
        '−' | '-' => "36",
        '±' => "235 25 36",
        '×' => "236",
        '·' | '⋅' => "3",
        '÷' | '/' => "256",
        '!' => "45 3",
        '%' => "456 356",
        '‰' => "456 356 356",
        '=' => "2356",
        '≈' => "4 2356",
        '≡' => "2356 2356",
        '<' => "246",
        '>' => "135",
        '≤' => "246 2356",
        '≥' => "135 2356",
        '≪' => "246 246",
        '≫' => "135 135",
        // negation: ⠘ before the relation (§4.3)
        '≠' => "45 2356",
        '≮' => "45 246",
        '≯' => "45 135",
        '≰' => "45 246 2356",
        '≱' => "45 135 2356",
        '≢' => "45 2356 2356",
        '≉' => "45 4 2356",
        '∉' => "45 126 2",
        '⊄' => "45 126 3",
        '⊈' => "45 126 23",
        '∈' => "126 2",
        '∋' => "5 345",
        '⊂' => "126 3",
        '⊆' => "126 23",
        '⊃' => "6 345",
        '⊇' => "56 345",
        '∪' => "456 345",
        '∩' => "456 156",
        '∅' => "456 245",
        '∖' | '\\' => "5 3",
        // degree, as after a superscript (CIB 2024, §11.5)
        '°' => "356",
        // negations by §4.3: ⠘ before the relation
        '≁' => "45 5 26 3",
        '⇏' => "45 25 135",
        '⇍' => "45 246 25",
        '⇎' => "45 246 25 135",
        '→' | '⟶' => "25 2",
        '⇒' | '⟹' => "25 135",
        '⇐' | '⟸' => "246 25",
        '⇔' | '⟺' => "246 25 135",
        '∞' => "3456 1256",
        '…' | '⋯' => "3 3 3",
        '∘' => "6 23",
        '∴' => "6 16",
        '∵' => "4 34",
        'ℵ' => "6 1256",
        '∼' => "5 26 3",
        // nabla: ^ï in the 2006 text, 4-12456 as the CIB 2024 edition prints it
        '∇' | '𝛁' => "4 12456",
        '∀' => "46 3",
        '∃' => "46 26",
        '¬' => "6 3",
        '∧' => "56 2",
        '∨' => "56 3",
        '∂' => "456 145",
        '′' | '\'' => "1256",
        '″' => "1256 1256",
        '(' => "126",
        ')' => "345",
        '[' => "12356",
        ']' => "23456",
        '{' => "5 123",
        '}' => "456 2",
        '⟨' => "5 13",
        '⟩' => "46 2",
        '|' => "456",
        '‖' => "456 123",
        ',' => "2",
        ';' => "23",
        ':' => "25",
        '.' => "3",
        _ => return None,
    })
}

/// The primes at the start of a superscript, and what remains of it.
fn split_primes(sup: Option<&Node>) -> (usize, Option<Node>) {
    let count = |n: &Node| match n {
        Node::Operator { symbol: '′', .. } => 1,
        Node::Operator { symbol: '″', .. } => 2,
        Node::Operator { symbol: '‴', .. } => 3,
        _ => 0,
    };
    match sup {
        Some(n) if count(n) > 0 => (count(n), None),
        Some(Node::Row { children }) if children.first().is_some_and(|c| count(c) > 0) => {
            let k = children.iter().take_while(|c| count(c) > 0).count();
            let primes = children[..k].iter().map(count).sum();
            let rest = &children[k..];
            (primes, if rest.is_empty() { None } else { Some(Node::row(rest.to_vec()).simplify()) })
        }
        other => (0, other.cloned()),
    }
}
