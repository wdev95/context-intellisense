//! What a screen reader says and the linear text (spec/speech.md): each structure by its molecular
//! formula and rings, formulas and labels through contextEquation, each arrow by what it means.

use super::formula;
use super::model::*;
use crate::locale::Locale;
use crate::rules::Registry;
use crate::tree::{Class, Node};

/// A chemistry phrase of the locale (`chem.*` in src/locales).
fn phrase(loc: Locale, key: &str) -> &'static str {
    loc.phrase(&format!("chem.{key}"))
}

fn fill(template: &str, args: &[&str]) -> String {
    let mut s = template.to_string();
    for (i, a) in args.iter().enumerate() {
        s = s.replace(&format!("{{{i}}}"), a);
    }
    s
}

/// A piece of LaTeX read as math (tolerantly: the drawing already reported its problems).
fn tree(tex: &str, loc: Locale) -> Option<Node> {
    crate::input::latex::read_with(tex, loc, &mut Registry::default(), false).ok()
}

fn math_speech(tex: &str, loc: Locale) -> String {
    tree(tex, loc).map(|n| crate::output::speech::speak(&n, loc)).unwrap_or_else(|| tex.to_string())
}

fn math_text(tex: &str, loc: Locale) -> String {
    tree(tex, loc).map(|n| crate::output::text::linear(&n, loc)).unwrap_or_else(|| tex.to_string())
}

/// Arrow labels and names are LaTeX text with `$…$` for math.
fn text_tex(t: &str) -> String {
    super::scheme::text_to_tex(t)
}

/// A ring read as aromatic: drawn with its circle, or a five- or six-membered ring whose atoms each
/// carry one double bond (a Kekulé drawing), allowing one heteroatom with a lone pair in a five-ring.
pub fn is_aromatic(m: &Molecule, r: &Ring) -> bool {
    if r.aromatic {
        return true;
    }
    if !matches!(r.atoms.len(), 5 | 6) {
        return false;
    }
    let doubles = |a: AtomId| m.bonds.iter().filter(|b| (b.from == a || b.to == a) && b.kind.order() == 2).count();
    let without: Vec<&AtomId> = r.atoms.iter().filter(|a| doubles(**a) != 1).collect();
    match (r.atoms.len(), without.as_slice()) {
        (6, []) => true,
        (5, [a]) => {
            let label = m.atoms[**a].label.trim_start_matches('{');
            ["N", "O", "S"].iter().any(|h| label.starts_with(h))
        }
        _ => false,
    }
}

pub fn molecule_speech(m: &Molecule, loc: Locale) -> String {
    let f = formula::of(m);
    let mut s = fill(phrase(loc, "structure"), &[&f.spoken()]);
    if f.charge != 0 {
        let sign = if f.charge > 0 { "+" } else { "−" };
        s.push_str(", ");
        s.push_str(&fill(phrase(loc, "charge"), &[&format!("{sign}{}", f.charge.unsigned_abs())]));
    }
    let aromatic = m.rings.iter().filter(|r| is_aromatic(m, r)).count();
    let plain = m.rings.len() - aromatic;
    if aromatic > 0 {
        s.push_str(", ");
        s.push_str(&if aromatic == 1 { phrase(loc, "aromatic_one").to_string() } else { fill(phrase(loc, "aromatic"), &[&aromatic.to_string()]) });
    }
    if plain > 0 {
        s.push_str(", ");
        s.push_str(&if plain == 1 { phrase(loc, "ring").to_string() } else { fill(phrase(loc, "rings"), &[&plain.to_string()]) });
    }
    s
}

fn label_speech(l: &Label, loc: Locale) -> String {
    match l {
        Label::Text { tex } => math_speech(&text_tex(tex), loc),
        Label::Structure(m) => molecule_speech(m, loc),
    }
}

/// The phrase an arrow is read by; an invisible arrow has none.
fn arrow_key(kind: ArrowKind) -> Option<&'static str> {
    Some(match kind {
        ArrowKind::Forward => "forward",
        ArrowKind::Backward => "backward",
        ArrowKind::Equilibrium => "equilibrium",
        ArrowKind::EquilibriumRight => "equilibrium_right",
        ArrowKind::EquilibriumLeft => "equilibrium_left",
        ArrowKind::Resonance => "resonance",
        ArrowKind::Retro => "retro",
        ArrowKind::NoReaction => "no_reaction",
        ArrowKind::Invisible => return None,
    })
}

/// The name a mechanism arrow's end is read by: the hooked atom's label, or the hook's name.
fn hook_name(s: &Scheme, hook: &str, loc: Locale) -> String {
    for item in &s.items {
        if let Item::Compound(Compound { body: Body::Structure(m), .. }) = item {
            if let Some(a) = m.atoms.iter().find(|a| a.hook.as_deref() == Some(hook)) {
                let label = a.label.trim();
                if !label.is_empty() {
                    return math_text(&format!("\\mathrm{{{label}}}"), Locale::En);
                }
            }
            // a node on a bond: the bond, by the atoms it joins (a vertex with no label is a carbon)
            if let Some(b) = m.bonds.iter().find(|b| b.hook.as_ref().is_some_and(|h| h.0 == hook)) {
                let atom = |id: usize| {
                    let label = m.atoms[id].label.trim();
                    if label.is_empty() {
                        "C".to_string()
                    } else {
                        math_text(&format!("\\mathrm{{{label}}}"), Locale::En)
                    }
                };
                return fill(phrase(loc, "bond_between"), &[&atom(b.from), &atom(b.to)]);
            }
        }
    }
    hook.to_string()
}

pub fn speak(s: &Scheme, loc: Locale) -> String {
    let mut parts: Vec<String> = Vec::new();
    for item in &s.items {
        let merged;
        let a = match item {
            Item::Compound(c) => {
                let body = match &c.body {
                    Body::Structure(m) => molecule_speech(m, loc),
                    Body::Math { tex } => math_speech(tex, loc),
                };
                parts.push(match &c.name {
                    // a name's lines (`\\` breaks them) are read one after the other
                    Some(n) => format!("{}, {body}", math_speech(&text_tex(&super::layout::split_lines(n).join(" ")), loc)),
                    None => body,
                });
                continue;
            }
            Item::Plus => {
                parts.push(loc.phrase("op.+").to_string());
                continue;
            }
            Item::Arrow(a) => a,
            // a merge is read as the arrow it ends in
            Item::Merge(m) => {
                merged = m.as_arrow();
                &merged
            }
        };
        let mut p = arrow_key(a.kind).map_or_else(String::new, |k| phrase(loc, k).to_string());
        let mut labels = Vec::new();
        if let Some(l) = &a.above {
            labels.push(fill(phrase(loc, "over"), &[&label_speech(l, loc)]));
        }
        if let Some(l) = &a.below {
            labels.push(fill(phrase(loc, "under"), &[&label_speech(l, loc)]));
        }
        if !labels.is_empty() {
            p.push_str(&format!(" ({})", labels.join("; ")));
        }
        if !p.is_empty() {
            parts.push(p);
        }
    }
    let mut speech = parts.join(" ");
    for m in &s.moves {
        let key = if m.single { "move_single" } else { "move" };
        speech.push_str("; ");
        speech.push_str(&fill(phrase(loc, key), &[&hook_name(s, &m.from, loc), &hook_name(s, &m.to, loc)]));
    }
    speech
}

/// The scheme on one line: structures by their formula, `+`, and the arrow's sign with its labels.
pub fn linear(s: &Scheme, loc: Locale) -> String {
    let mut parts: Vec<String> = Vec::new();
    for item in &s.items {
        let merged;
        let a = match item {
            Item::Compound(c) => {
                parts.push(match &c.body {
                    Body::Structure(m) => formula::of(m).written(),
                    Body::Math { tex } => math_text(tex, loc),
                });
                continue;
            }
            Item::Plus => {
                parts.push("+".to_string());
                continue;
            }
            Item::Arrow(a) => a,
            // a merge is read as the arrow it ends in
            Item::Merge(m) => {
                merged = m.as_arrow();
                &merged
            }
        };
        let sign = match a.kind {
            ArrowKind::Forward => "→",
            ArrowKind::Backward => "←",
            ArrowKind::Equilibrium | ArrowKind::EquilibriumRight | ArrowKind::EquilibriumLeft => "⇌",
            ArrowKind::Resonance => "↔",
            ArrowKind::Retro => "⇒",
            ArrowKind::NoReaction => "↛",
            ArrowKind::Invisible => "",
        };
        let labels: Vec<String> = [&a.above, &a.below]
            .into_iter()
            .flatten()
            .map(|l| match l {
                Label::Text { tex } => math_text(&text_tex(tex), loc),
                Label::Structure(m) => formula::of(m).written(),
            })
            .collect();
        parts.push(if labels.is_empty() { sign.to_string() } else { format!("{sign}[{}]", labels.join("; ")) });
    }
    parts.retain(|p| !p.is_empty());
    parts.join(" ")
}

/// The scheme as a row of math nodes, for braille: each structure by its formula, the formulas and
/// labels as contextEquation reads them, `+` and the arrows with their labels.
pub fn as_nodes(s: &Scheme) -> Node {
    let loc = Locale::En;
    let read = |tex: &str| tree(tex, loc).unwrap_or_else(|| Node::row(vec![]));
    let label = |l: &Option<Label>| -> Option<Box<Node>> {
        l.as_ref().map(|l| {
            Box::new(match l {
                Label::Text { tex } => read(&text_tex(tex)),
                Label::Structure(m) => read(&formula::of(m).tex()),
            })
        })
    };
    let mut children = Vec::new();
    for item in &s.items {
        let merged;
        let a = match item {
            Item::Compound(c) => {
                children.push(match &c.body {
                    Body::Structure(m) => read(&formula::of(m).tex()),
                    Body::Math { tex } => read(tex),
                });
                continue;
            }
            Item::Plus => {
                children.push(Node::Operator { symbol: '+', class: Class::Bin });
                continue;
            }
            Item::Arrow(a) => a,
            // a merge is read as the arrow it ends in
            Item::Merge(m) => {
                merged = m.as_arrow();
                &merged
            }
        };
        let symbol = match a.kind {
            ArrowKind::Forward | ArrowKind::NoReaction => '→',
            ArrowKind::Backward => '←',
            ArrowKind::Equilibrium | ArrowKind::EquilibriumRight | ArrowKind::EquilibriumLeft => '⇌',
            ArrowKind::Resonance => '↔',
            ArrowKind::Retro => '⇒',
            ArrowKind::Invisible => continue,
        };
        children.push(Node::Arrow { symbol, above: label(&a.above), below: label(&a.below) });
    }
    Node::row(children)
}
