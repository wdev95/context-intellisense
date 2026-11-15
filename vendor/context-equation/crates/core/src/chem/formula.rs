//! The molecular formula of a structure, counted from its graph (rule C-FOR-1): the elements written in
//! the labels, the abbreviations (Me, Et, Ph…) expanded, and the hydrogens a skeletal carbon carries
//! implicitly (4 minus its bond orders). Written in Hill order: C, then H, then the rest alphabetically.

use std::collections::BTreeMap;

use super::model::{Atom, ChargeContent, Molecule};
use crate::input::symbols::ELEMENTS;

/// Group abbreviations of organic chemistry and what they stand for. They take precedence over
/// element symbols they look like (`Ac` is acetyl in a structure, not actinium; `Ts` is tosyl).
const ABBREVIATIONS: &[(&str, &[(&str, u32)])] = &[
    ("Me", &[("C", 1), ("H", 3)]),
    ("Et", &[("C", 2), ("H", 5)]),
    ("Pr", &[("C", 3), ("H", 7)]),
    ("iPr", &[("C", 3), ("H", 7)]),
    ("Bu", &[("C", 4), ("H", 9)]),
    ("tBu", &[("C", 4), ("H", 9)]),
    ("Ph", &[("C", 6), ("H", 5)]),
    ("Bn", &[("C", 7), ("H", 7)]),
    ("Ac", &[("C", 2), ("H", 3), ("O", 1)]),
    ("Bz", &[("C", 7), ("H", 5), ("O", 1)]),
    ("Ts", &[("C", 7), ("H", 7), ("O", 2), ("S", 1)]),
    ("Ms", &[("C", 1), ("H", 3), ("O", 2), ("S", 1)]),
    ("Tf", &[("C", 1), ("F", 3), ("O", 2), ("S", 1)]),
    ("Boc", &[("C", 5), ("H", 9), ("O", 2)]),
    ("Cbz", &[("C", 8), ("H", 7), ("O", 2)]),
    ("TMS", &[("C", 3), ("H", 9), ("Si", 1)]),
];

/// A formula: element → count, plus what could not be counted (R, X, Nu…) and the net charge.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Formula {
    pub counts: BTreeMap<String, u32>,
    /// Labels that are not elements nor known groups, kept as written: `R`, `X`, `Nu`.
    pub unknown: Vec<String>,
    pub charge: i32,
}

impl Formula {
    pub fn add(&mut self, element: &str, n: u32) {
        *self.counts.entry(element.to_string()).or_insert(0) += n;
    }

    /// Hill order: C and H first when there is carbon, then alphabetical.
    pub fn ordered(&self) -> Vec<(&str, u32)> {
        let mut out: Vec<(&str, u32)> = Vec::new();
        let carbon = self.counts.contains_key("C");
        if carbon {
            for e in ["C", "H"] {
                if let Some(n) = self.counts.get(e) {
                    out.push((e, *n));
                }
            }
        }
        for (e, n) in &self.counts {
            if !(carbon && (e == "C" || e == "H")) {
                out.push((e, *n));
            }
        }
        out
    }

    /// `C₂H₆O`, with subscript digits and the charge as a superscript (`NH₄⁺`).
    pub fn written(&self) -> String {
        let mut s = String::new();
        for (e, n) in self.ordered() {
            s.push_str(e);
            if n > 1 {
                s.extend(n.to_string().chars().map(subscript));
            }
        }
        for u in &self.unknown {
            s.push_str(u);
        }
        s.push_str(&charge_text(self.charge, true));
        s
    }

    /// The formula as LaTeX for contextEquation: `C_{2}H_{6}O`.
    pub fn tex(&self) -> String {
        let mut s = String::new();
        for (e, n) in self.ordered() {
            s.push_str(&format!("\\mathrm{{{e}}}"));
            if n > 1 {
                s.push_str(&format!("_{{{n}}}"));
            }
        }
        for u in &self.unknown {
            s.push_str(&format!("\\mathrm{{{u}}}"));
        }
        if self.charge != 0 {
            s.push_str(&format!("^{{{}}}", charge_text(self.charge, false)));
        }
        s
    }

    /// Read aloud: element symbols with their counts, `C2 H6 O`.
    pub fn spoken(&self) -> String {
        let mut parts: Vec<String> = self.ordered().into_iter().map(|(e, n)| if n > 1 { format!("{e}{n}") } else { e.to_string() }).collect();
        parts.extend(self.unknown.iter().cloned());
        parts.join(" ")
    }
}

fn subscript(c: char) -> char {
    char::from_u32('₀' as u32 + c.to_digit(10).unwrap_or(0)).unwrap_or(c)
}

fn superscript(c: char) -> char {
    match c {
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '+' => '⁺',
        '-' => '⁻',
        d => char::from_u32('⁰' as u32 + d.to_digit(10).unwrap_or(0)).unwrap_or(d),
    }
}

fn charge_text(charge: i32, unicode: bool) -> String {
    if charge == 0 {
        return String::new();
    }
    let sign = if charge > 0 { '+' } else { '-' };
    let n = charge.unsigned_abs();
    let plain = if n > 1 { format!("{n}{sign}") } else { sign.to_string() };
    if unicode {
        plain.chars().map(superscript).collect()
    } else {
        plain
    }
}

/// The formula of one molecule.
pub fn of(molecule: &Molecule) -> Formula {
    let mut f = Formula::default();
    for (id, atom) in molecule.atoms.iter().enumerate() {
        if atom.label.trim().is_empty() {
            // a skeletal vertex is a carbon with the hydrogens its bonds leave room for
            let mut used: u32 = molecule.bonds.iter().filter(|b| b.from == id || b.to == id).map(|b| b.kind.order()).sum();
            // a vertex of an aromatic ring shares half of the delocalized bond on each side
            if molecule.rings.iter().any(|r| r.aromatic && r.atoms.contains(&id)) {
                used += 1;
            }
            f.add("C", 1);
            f.add("H", 4u32.saturating_sub(used));
        } else {
            label(atom, &mut f);
        }
        // a \charge that writes a formal charge, ⊕ or ⊖ (a δ+ is not one)
        for charge in &atom.charges {
            if let ChargeContent::Text { tex } = &charge.content {
                f.charge += formal_charge(tex);
            }
        }
    }
    f
}

/// The formal charge a `\charge` writes: `$\oplus$`, `$+$`, `$\scriptstyle\ominus$`, `$2-$`; 0 for
/// anything else.
fn formal_charge(tex: &str) -> i32 {
    let mut t = tex.replace(['$', '{', '}', ' '], "");
    for style in ["\\scriptscriptstyle", "\\scriptstyle", "\\textstyle", "\\small", "\\scriptsize", "\\tiny"] {
        t = t.replace(style, "");
    }
    let t = t.trim_start_matches('^').replace("\\oplus", "+").replace("\\ominus", "-");
    let (digits, sign) = t.split_at(t.find(['+', '-']).unwrap_or(t.len()));
    let n: i32 = if digits.is_empty() { 1 } else { digits.parse().unwrap_or(0) };
    match sign {
        "+" => n,
        "-" => -n,
        _ => 0,
    }
}

/// Counts what a label says: `CH_2OH` is C, H₂, O, H; `(CH_2)_3` repeats; `NH_3^+` adds a charge.
fn label(atom: &Atom, f: &mut Formula) {
    let text = strip_tex(&atom.label);
    let c: Vec<char> = text.chars().collect();
    let mut stack: Vec<Formula> = vec![Formula::default()];
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '(' {
            stack.push(Formula::default());
            i += 1;
            continue;
        }
        if ch == ')' {
            i += 1;
            let n = number(&c, &mut i).unwrap_or(1);
            // a ')' with no '(' before it (the reader warned about it) counts nothing (#210)
            let [.., top, inner] = stack.as_mut_slice() else { continue };
            let inner = std::mem::take(inner);
            for (e, k) in inner.counts {
                top.add(&e, k * n);
            }
            for _ in 0..n {
                top.unknown.extend(inner.unknown.iter().cloned());
            }
            stack.pop();
            continue;
        }
        if ch == '^' {
            i += 1;
            let (sign, n) = charge(&c, &mut i);
            f.charge += sign * n as i32;
            continue;
        }
        if ch.is_ascii_uppercase() || (ch.is_ascii_lowercase() && matches!(c.get(i + 1), Some('P' | 'B'))) {
            // the longest abbreviation or element symbol that starts here
            let rest: String = c[i..].iter().take(3).collect();
            let found = ABBREVIATIONS.iter().filter(|(a, _)| rest.starts_with(a)).max_by_key(|(a, _)| a.len()).map(|(a, parts)| (a.len(), Some(*parts), *a)).or_else(|| {
                let two: String = c[i..].iter().take(2).collect();
                if two.len() == 2 && c[i + 1].is_ascii_lowercase() && ELEMENTS.contains(&two.as_str()) {
                    Some((2, None, ""))
                } else if ELEMENTS.contains(&ch.to_string().as_str()) && !c.get(i + 1).is_some_and(char::is_ascii_lowercase) {
                    // a capital followed by a lowercase letter is one symbol: Nu is not N and a lost u
                    Some((1, None, ""))
                } else {
                    None
                }
            });
            let top = stack.last_mut().expect("the outer level is never popped");
            match found {
                Some((len, parts, _)) => {
                    let symbol: String = c[i..i + len].iter().collect();
                    i += len;
                    let n = number(&c, &mut i).unwrap_or(1);
                    match parts {
                        Some(parts) => {
                            for (e, k) in parts {
                                top.add(e, k * n);
                            }
                        }
                        None => top.add(&symbol, n),
                    }
                }
                None => {
                    // R, R', X, Nu…: a letter and what follows it until the next capital
                    let mut s = String::from(ch);
                    i += 1;
                    while i < c.len() && (c[i].is_ascii_lowercase() || c[i] == '\'' || c[i].is_ascii_digit()) {
                        s.push(c[i]);
                        i += 1;
                    }
                    top.unknown.push(s);
                }
            }
            continue;
        }
        i += 1;
    }
    let top = stack.into_iter().next().unwrap_or_default();
    for (e, k) in top.counts {
        f.add(&e, k);
    }
    f.unknown.extend(top.unknown);
}

/// A label without the LaTeX that does not count atoms: `\mathrm`, braces, `_`, `\oplus` as `^+`.
fn strip_tex(label: &str) -> String {
    let s = label.replace("\\oplus", "^+").replace("\\ominus", "^-").replace("\\Theta", "^-");
    let mut out = String::new();
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '\\' => {
                // drop the command name; its braced argument stays as text
                i += 1;
                while i < c.len() && c[i].is_ascii_alphabetic() {
                    i += 1;
                }
            }
            '{' | '}' | '_' | ' ' | '$' => i += 1,
            ch => {
                out.push(ch);
                i += 1;
            }
        }
    }
    out
}

fn number(c: &[char], i: &mut usize) -> Option<u32> {
    let start = *i;
    while *i < c.len() && c[*i].is_ascii_digit() {
        *i += 1;
    }
    (*i > start).then(|| c[start..*i].iter().collect::<String>().parse().unwrap_or(1))
}

/// `^+`, `^{2-}`, `^-`, `^{+}`: (sign, size) of a charge written as a superscript.
fn charge(c: &[char], i: &mut usize) -> (i32, u32) {
    let n = number(c, i).unwrap_or(1);
    let sign = match c.get(*i) {
        Some('+') => 1,
        Some('-') => -1,
        _ => 0,
    };
    if sign != 0 {
        *i += 1;
    }
    let n = number(c, i).unwrap_or(n);
    (sign, n)
}

#[cfg(test)]
mod tests {
    use super::super::model::{Charge, Ring};
    use super::*;

    fn atom(label: &str) -> Formula {
        let mut f = Formula::default();
        label_into(label, &mut f);
        f
    }

    fn label_into(text: &str, f: &mut Formula) {
        label(&Atom { label: text.to_string(), ..Atom::default() }, f);
    }

    fn charged(label: &str, tex: &str) -> Atom {
        let charge = Charge { angle: 0.0, offset: 0.0, piece: 0, anchor: None, content: ChargeContent::Text { tex: tex.to_string() } };
        Atom { label: label.to_string(), charges: vec![charge], ..Atom::default() }
    }

    #[test]
    fn a_molecule_counts_its_vertices_labels_and_formal_charges() {
        // a lone vertex of an aromatic ring: a carbon, its shared delocalized bond and three hydrogens
        let ring = Ring { size: 1, atoms: vec![0], aromatic: true, arc: None, dashed: false };
        let molecule = Molecule { atoms: vec![Atom::default(), charged("N", "$\\oplus$")], rings: vec![ring], ..Molecule::default() };
        let f = of(&molecule);
        assert_eq!(f.written(), "CH₃N⁺");
        // a δ+ is no formal charge; the sizes and styles around a sign are not read
        assert_eq!(of(&Molecule { atoms: vec![charged("O", "$\\delta^+$")], ..Molecule::default() }).charge, 0);
        assert_eq!(formal_charge("$\\scriptstyle 2-$"), -2);
        assert_eq!(formal_charge("{x+}"), 0);
    }

    #[test]
    fn labels_count_their_atoms() {
        assert_eq!(atom("CH_3").written(), "CH₃");
        assert_eq!(atom("H_3C").written(), "CH₃");
        assert_eq!(atom("CH_2OH").written(), "CH₃O");
        assert_eq!(atom("(CH_2)_3CH_3").written(), "C₄H₉");
        assert_eq!(atom("NH_3^+").written(), "H₃N⁺");
        assert_eq!(atom("SO_4^{2-}").written(), "O₄S²⁻");
        assert_eq!(atom("O\\ominus").written(), "O⁻");
        assert_eq!(atom("OMe").written(), "CH₃O");
        assert_eq!(atom("Ph").written(), "C₆H₅");
        assert_eq!(atom("NHBoc").written(), "C₅H₁₀NO₂");
        assert_eq!(atom("Cl").written(), "Cl");
        assert_eq!(atom("R'").written(), "R'");
        assert_eq!(atom("\\mathrm{Br}").written(), "Br");
        // a capital and a lowercase letter that are no element are kept whole, not read as the capital
        assert_eq!(atom("Nu").written(), "Nu");
        assert_eq!(atom("CNu").written(), "CNu");
        assert_eq!(atom("Cy").written(), "Cy");
    }

    #[test]
    fn hill_order_without_carbon_is_alphabetical() {
        let mut f = Formula::default();
        f.add("O", 1);
        f.add("H", 2);
        assert_eq!(f.written(), "H₂O");
        assert_eq!(f.tex(), "\\mathrm{H}_{2}\\mathrm{O}");
        assert_eq!(f.spoken(), "H2 O");
    }
}
