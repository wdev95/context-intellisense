//! Readable linear text (close to UnicodeMath): goes into the SVG's selection layer and `--text`.

use crate::layout::classes;
use crate::locale::Locale;
use crate::tree::*;

pub fn linear(node: &Node, loc: Locale) -> String {
    let mut s = String::new();
    write_node(node, loc, &mut s);
    // #176: a symbol drawn by composition, as the closest text Unicode has
    crate::composite::spell(&s).split_whitespace().collect::<Vec<_>>().join(" ")
}

fn alone(node: &Node) -> bool {
    match node {
        Node::Number { .. } | Node::Identifier { .. } | Node::Operator { .. } => true,
        Node::Row { children } => children.len() == 1 && alone(&children[0]),
        Node::Text { parts } => parts.len() == 1,
        Node::Unit { base, .. } => alone(base),
        _ => false,
    }
}

/// In parentheses when the piece has more than one atom.
fn group(node: &Node, loc: Locale, s: &mut String) {
    if alone(node) {
        write_node(node, loc, s);
    } else {
        s.push('(');
        write_node(node, loc, s);
        s.push(')');
    }
}

fn write_node(node: &Node, loc: Locale, s: &mut String) {
    match node {
        Node::Number { value, thousands, variant, .. } => s.extend(super::written_number(value, *thousands, loc).chars().map(|c| crate::layout::alphabet::character(c, *variant))),
        Node::Identifier { name, variant } => {
            let mapped: String = name.chars().map(|c| crate::layout::alphabet::character(c, *variant)).collect();
            s.push_str(if matches!(variant, Variant::Italic | Variant::Upright) { name } else { &mapped });
        }
        Node::Function { name, .. } => {
            s.push_str(&loc.function_name(name));
            s.push(' ');
        }
        Node::Operator { symbol, .. } | Node::FixedDelimiter { symbol, .. } => s.push(*symbol),
        Node::LargeOperator { symbol, .. } => {
            s.push(*symbol);
            s.push(' ');
        }
        Node::Row { children } => {
            let cl = classes(children, loc, None, false);
            for (child, class) in children.iter().zip(cl) {
                match (child, class) {
                    (Node::Operator { symbol, .. }, Some(Class::Bin | Class::Rel)) => {
                        s.push(' ');
                        s.push(*symbol);
                        s.push(' ');
                    }
                    (Node::Operator { symbol, .. }, Some(Class::Punct)) => {
                        s.push(*symbol);
                        s.push(' ');
                    }
                    (other, _) => write_node(other, loc, s),
                }
            }
        }
        Node::Fraction { num, den, rule, .. } => {
            if *rule {
                group(num, loc, s);
                s.push('/');
                group(den, loc, s);
            } else {
                s.push('(');
                write_node(num, loc, s);
                s.push_str(" ¦ ");
                write_node(den, loc, s);
                s.push(')');
            }
        }
        Node::Root { radicand, index } => {
            match index.as_deref() {
                None => s.push('√'),
                Some(Node::Number { value, .. }) if value == "3" => s.push('∛'),
                Some(Node::Number { value, .. }) if value == "4" => s.push('∜'),
                Some(i) => {
                    s.push_str("√[");
                    write_node(i, loc, s);
                    s.push(']');
                }
            }
            group(radicand, loc, s);
        }
        Node::Scripts { base, sup, sub } => {
            // sin²x, log₂x, ∑_(i = 1)^n i: the scripts stick to the function name or the large
            // operator, and the space goes after them
            let base = base.operator_core();
            let spaced = matches!(base, Node::Function { .. } | Node::LargeOperator { .. });
            match base {
                Node::Function { name, .. } => s.push_str(&loc.function_name(name)),
                Node::LargeOperator { symbol, .. } => s.push(*symbol),
                other => write_node(other, loc, s),
            }
            if let Some(b) = sub {
                s.push('_');
                group(b, loc, s);
            }
            if let Some(p) = sup {
                let prime = |n: &Node| matches!(n, Node::Operator { symbol: '′' | '″' | '‴', .. });
                match p.as_ref() {
                    n if prime(n) => write_node(n, loc, s),
                    Node::Row { children } if children.first().is_some_and(prime) => {
                        let qty = children.iter().take_while(|f| prime(f)).count();
                        for f in &children[..qty] {
                            write_node(f, loc, s);
                        }
                        s.push('^');
                        group(&Node::row(children[qty..].to_vec()).simplify(), loc, s);
                    }
                    other => {
                        s.push('^');
                        group(other, loc, s);
                    }
                }
            }
            if spaced {
                s.push(' ');
            }
        }
        Node::Delimited { open, close, parts, middles } => {
            if let Some(a) = open {
                s.push(*a);
            }
            // \binom is a fraction without a rule inside parentheses: (n ¦ k), not ((n ¦ k))
            if let [p] = parts.as_slice() {
                if let Node::Fraction { num, den, rule: false, .. } = p.single() {
                    write_node(num, loc, s);
                    s.push_str(" ¦ ");
                    write_node(den, loc, s);
                    if let Some(f) = close {
                        s.push(*f);
                    }
                    return;
                }
            }
            for (i, p) in parts.iter().enumerate() {
                if i > 0 {
                    s.push(' ');
                    s.push(middles.get(i - 1).copied().unwrap_or('|'));
                    s.push(' ');
                }
                write_node(p, loc, s);
            }
            if let Some(f) = close {
                s.push(*f);
            }
        }
        Node::Table { shape: kind, rows, .. } => {
            let cell = |c: &Node| linear(c, loc);
            match kind {
                TableKind::Aligned | TableKind::Centered | TableKind::Cases | TableKind::RCases | TableKind::Array | TableKind::Substack | TableKind::Diagram => {
                    let join_rows = if *kind == TableKind::Aligned { " " } else { ", " };
                    let l: Vec<String> = rows.iter().map(|l| l.iter().map(cell).collect::<Vec<_>>().join(join_rows)).collect();
                    if *kind == TableKind::Cases {
                        s.push('{');
                    }
                    s.push_str(&l.join("; "));
                    if *kind == TableKind::RCases {
                        s.push('}');
                    }
                }
                _ => {
                    let l: Vec<String> = rows.iter().map(|l| format!("[{}]", l.iter().map(cell).collect::<Vec<_>>().join(", "))).collect();
                    let (a, f) = match kind {
                        TableKind::PMatrix => ("(", ")"),
                        TableKind::VMatrix => ("|", "|"),
                        _ => ("[", "]"),
                    };
                    s.push_str(a);
                    s.push_str(&l.join(", "));
                    s.push_str(f);
                }
            }
        }
        Node::Accent { base, accent, .. } => {
            group(base, loc, s);
            if matches!(*accent as u32, 0x0300..=0x036F | 0x20D0..=0x20FF) {
                s.push(*accent);
            }
        }
        Node::Bar { base, above } => {
            if *above {
                s.push('‾');
            }
            group(base, loc, s);
        }
        Node::Brace { base, label, .. } => {
            write_node(base, loc, s);
            if let Some(r) = label {
                s.push_str(" [");
                write_node(r, loc, s);
                s.push(']');
            }
        }
        Node::Text { parts } => {
            for p in parts {
                match p {
                    TextPart::Text(t) => s.push_str(&t.replace(crate::input::latex::MARKERS, "")),
                    TextPart::Math(m) => write_node(m, loc, s),
                }
            }
        }
        Node::Space { em } => {
            if *em > 0.1 {
                s.push(' ');
            }
        }
        Node::Phantom { .. } => {}
        // \mathchoice: the text style's branch
        Node::Choice { branches } => {
            if let Some(b) = branches.get(1) {
                write_node(b, loc, s);
            }
        }
        // the Feynman slash as Unicode writes an overstruck slash: D̸, with the combining long solidus
        Node::Canceled { base, shape: Cancel::Slash } => {
            write_node(base, loc, s);
            s.push('\u{0338}');
        }
        Node::Canceled { base, .. }
        | Node::Colored { base, .. }
        | Node::Attributed { base, .. }
        | Node::ForcedClass { base, .. }
        | Node::Style { base, .. }
        | Node::Unit { base, .. } => write_node(base, loc, s),
        Node::Framed { base, shape, .. } => {
            match shape {
                Frame::Circle => {
                    s.push('(');
                    write_node(base, loc, s);
                    s.push(')');
                }
                // A∠θ, as the phasor is written in plain text; the annuity's a_(n⌉)
                Frame::Phase => {
                    s.push('∠');
                    write_node(base, loc, s);
                }
                Frame::ActuarialAngle => {
                    write_node(base, loc, s);
                    s.push('⌉');
                }
                _ => write_node(base, loc, s),
            }
        }
        Node::Quantity { unit, .. } => write_node(unit, loc, s),
        Node::Arrow { symbol, above, .. } => {
            s.push(' ');
            s.push(*symbol);
            if let Some(a) = above {
                s.push('[');
                write_node(a, loc, s);
                s.push(']');
            }
            s.push(' ');
        }
        Node::Stacked { base, .. }
        | Node::Multicolumn { base, .. }
        | Node::Raised { base, .. }
        | Node::Scaled { base, .. }
        | Node::Smashed { base, .. }
        | Node::VCenter { base }
        | Node::Resized { base, .. }
        | Node::Rotated { base, .. }
        | Node::Reflected { base }
        | Node::Overlapped { base, .. } => write_node(base, loc, s),
        Node::VdotsWithin { .. } => write_node(&Node::VDOTS, loc, s),
        Node::Rule { .. } => {}
        Node::VerticalArrow { symbol, .. } => {
            s.push(' ');
            s.push(*symbol);
            s.push(' ');
        }
        #[cfg(feature = "chem")]
        Node::Chemistry { scheme } => s.push_str(&crate::chem::speech::linear(scheme, loc)),
        Node::ArrowDiagram { cells, arrows } => {
            let rows: Vec<String> = cells.iter().map(|l| l.iter().map(|c| linear(c, loc)).collect::<Vec<_>>().join(", ")).collect();
            s.push_str(&rows.join("; "));
            for a in arrows {
                s.push_str("; ");
                s.push_str(linear(&a.as_row(cells), loc).trim());
            }
        }
        Node::LongDivision { dividend, divisor } => {
            let (q, r) = match (dividend.parse::<u128>(), divisor.parse::<u128>()) {
                (Ok(a), Ok(b)) if b > 0 => (a / b, a % b),
                _ => (0, 0),
            };
            let remainder = loc.phrase("remainder");
            s.push_str(&format!("{dividend} ÷ {divisor} = {q} ({remainder} {r})"));
        }
        Node::ColumnArithmetic { operation, terms } => {
            let a: i128 = terms[0].parse().unwrap_or(0);
            let b: i128 = terms[1].parse().unwrap_or(0);
            let r = match operation {
                '+' => a + b,
                '−' => a - b,
                _ => a.saturating_mul(b),
            };
            s.push_str(&format!("{a} {operation} {b} = {r}"));
        }
    }
}
