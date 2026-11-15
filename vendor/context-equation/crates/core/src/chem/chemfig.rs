//! chemfig's molecule language (spec/input.md): atoms, bonds with their angles, branches, rings
//! (fused or not, aromatic or not), distant bonds (`?`), Lewis marks, charges (`\charge`) and hooks
//! for mechanisms, and the default bond values a structure may start with (`[:20,1.5]`).

use std::collections::BTreeMap;

use super::model::*;
use super::scheme::{split_top, Reader, R};
use crate::rules;

/// Characters that end an atom label.
const STOPS: &[char] = &['-', '=', '~', '<', '>', '(', ')', '*', '[', ']', '@'];

/// The state of the ring being read: how its bonds turn and where it closes.
#[derive(Clone, Copy)]
struct RingState {
    id: usize,
    first: f32,
    /// Degrees each bond turns from the previous one: positive counterclockwise.
    turn: f32,
    /// Bonds the ring content may have, and the atom the last one closes on.
    bonds: usize,
    close: AtomId,
}

struct Mol<'r, 'w> {
    c: Vec<char>,
    pos: usize,
    m: Molecule,
    reader: &'r mut Reader<'w>,
    /// Exterior bisector of each ring vertex: the default direction of a branch there.
    bisector: BTreeMap<AtomId, f32>,
    /// The first `?[name]` of each name: the atom and the piece every later mark of the name is bonded to.
    closures: BTreeMap<String, (AtomId, usize)>,
    hook: Option<String>,
    /// `[angle,length,departure,arrival,tikz]` at the start of the structure.
    defaults: Defaults,
    /// What the label being read asks of its atom, given to it once its bond is drawn.
    pending: Pending,
    /// The direction of the bond reaching the atom being read: picks a dual submolecule's code.
    arriving: Option<f32>,
    /// Submolecules expanded so far: a submolecule that calls itself stops at a limit.
    expansions: usize,
}

/// chemfig manual, section 2.6: the values a bond takes for the fields it leaves empty, which a
/// structure may set by starting with `[angle,length,departure,arrival,tikz]`. Without them, an empty
/// angle keeps the previous direction, as contextEquation always read it (spec/chemistry.md).
struct Defaults {
    angle: Option<f32>,
    length: f32,
    departure: Option<usize>,
    arrival: Option<usize>,
    tikz: String,
}

impl Default for Defaults {
    fn default() -> Self {
        Defaults { angle: None, length: 1.0, departure: None, arrival: None, tikz: String::new() }
    }
}

/// Read in a label, applied to its atom: the `?[…]` marks with the piece they follow, the
/// `\charge`s and a `\chemskipalign`.
#[derive(Default)]
struct Pending {
    closures: Vec<(Option<String>, usize)>,
    charges: Vec<Charge>,
    skip_align: bool,
    attributes: Vec<(String, String)>,
}

/// Reads a `\chemfig{…}` body.
pub fn molecule(src: &str, reader: &mut Reader) -> R<Molecule> {
    let mut p = Mol {
        c: src.chars().collect(),
        pos: 0,
        m: Molecule::default(),
        reader,
        bisector: BTreeMap::new(),
        closures: BTreeMap::new(),
        hook: None,
        defaults: Defaults::default(),
        pending: Pending::default(),
        arriving: None,
        expansions: 0,
    };
    p.skip_spaces();
    if p.current() == Some('[') {
        p.read_defaults()?;
    }
    let start = p.atom(None, None)?;
    p.attach(start)?;
    p.chain(start, p.defaults.angle.unwrap_or(0.0), None)?;
    p.skip_spaces();
    while p.pos < p.c.len() {
        let ch = p.c[p.pos];
        p.reader.tolerate(rules::E_REC_9, format!("'{ch}' with nothing to close or attach to"), "ignored")?;
        p.pos += 1;
        p.chain(start, 0.0, None)?;
    }
    // a ?[name] no other mark names draws nothing, as in chemfig
    // a submolecule rewrote the code: the places read are not those written
    if p.expansions > 0 {
        p.m.atoms.iter_mut().for_each(|a| a.span = None);
        p.m.bonds.iter_mut().for_each(|b| b.span = None);
    }
    Ok(p.m)
}

/// A bond's `#(dim1,dim2)`: the space at its start and at its end, in em, when it sets them.
type Gaps = (Option<f32>, Option<f32>);

/// `[angle,length,departure,arrival,tikz]` as defaults: each field left empty keeps `inherited`'s,
/// and a relative angle (`::x`) is relative to `previous`.
fn defaults_from(args: &[String], previous: f32, inherited: &Defaults) -> Defaults {
    let field = |i: usize| args.get(i).map(|a| a.trim()).filter(|a| !a.is_empty());
    let piece = |i: usize| field(i).and_then(|v| v.parse::<usize>().ok()).filter(|v| *v > 0).map(|v| v - 1);
    let tikz = args.get(4..).map(|t| t.join(",")).filter(|t| !t.trim().is_empty());
    Defaults {
        angle: field(0).map(|a| resolve_angle(a, previous)).or(inherited.angle),
        length: field(1).and_then(|l| l.parse::<f32>().ok()).filter(|l| *l > 0.0).unwrap_or(inherited.length),
        departure: piece(2).or(inherited.departure),
        arrival: piece(3).or(inherited.arrival),
        tikz: tikz.unwrap_or_else(|| inherited.tikz.clone()),
    }
}

/// `@{name,coeff}` at the start of a bond's first field (chemfig manual, section 5.8.1): the node's
/// name and where it sits along the bond (0.5 by default), and what is left of the field, the angle.
fn bond_hook(field: &str) -> (Option<(String, f32)>, &str) {
    let Some(rest) = field.strip_prefix("@{") else { return (None, field) };
    let Some(close) = rest.find('}') else { return (None, field) };
    let (spec, angle) = (&rest[..close], &rest[close + 1..]);
    let (name, coeff) = spec.split_once(',').map_or((spec, None), |(n, c)| (n, c.trim().parse::<f32>().ok()));
    let coeff = coeff.filter(|c| c.is_finite()).map_or(0.5, |c| c.clamp(0.0, 1.0));
    (Some((name.trim().to_string(), coeff)), angle.trim())
}

/// Whether `text` ends with a control word, `\\name`: a letter after it would join its name.
fn ends_with_control_word(text: &str) -> bool {
    let letters = text.chars().rev().take_while(char::is_ascii_alphabetic).count();
    letters > 0 && text.chars().rev().nth(letters) == Some('\\') && text.chars().rev().nth(letters + 1) != Some('\\')
}

/// A TeX dimension in em (`2pt`, `0.5em`, `1mm`), as chemfig's `#(…)` takes them; 10pt is an em.
pub(super) fn dimension_em(s: &str) -> Option<f32> {
    let s = s.trim();
    let units = [("pt", 0.1), ("bp", 0.1004), ("em", 1.0), ("ex", 0.45), ("mm", 0.2845), ("cm", 2.845), ("in", 7.227)];
    units.iter().find_map(|(u, f)| s.strip_suffix(u).and_then(|v| v.trim().parse::<f32>().ok()).filter(|v| v.is_finite()).map(|v| v * f))
}

fn is_bond_start(ch: char) -> bool {
    matches!(ch, '-' | '=' | '~' | '<' | '>')
}

impl Mol<'_, '_> {
    fn current(&self) -> Option<char> {
        self.c.get(self.pos).copied()
    }

    fn skip_spaces(&mut self) {
        while self.current().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    /// Content between `open` and its matching `close`; the current character is `open`.
    fn group(&mut self, open: char, close: char) -> R<String> {
        let mut level = 0;
        let mut s = String::new();
        while let Some(ch) = self.current() {
            self.pos += 1;
            if ch == open {
                level += 1;
                if level == 1 {
                    continue;
                }
            } else if ch == close {
                level -= 1;
                if level == 0 {
                    return Ok(s);
                }
            }
            s.push(ch);
        }
        self.reader.tolerate(rules::E_REC_4, format!("'{open}' without a matching '{close}'"), "closed at the end")?;
        Ok(s)
    }

    /// Reads a branch, the rest of a chain: bonds, atoms, branches, rings and `?` marks, until `)` or the end.
    /// `angle` is the default direction of the first bond (each bond's is then the previous one's).
    fn chain(&mut self, start: AtomId, angle: f32, ring: Option<RingState>) -> R<()> {
        let mut cur = start;
        // with the structure's defaults, an empty angle is the branch's default, not the previous one
        let default_angle = self.defaults.angle.map(|_| angle);
        let mut angle = angle;
        // in a ring, the bond count so far and the direction of the last ring bond
        let mut k = 0usize;
        let mut last_edge: Option<f32> = None;
        // the atom a nested ring already put where this ring's next vertex goes
        let mut preset: Option<AtomId> = None;
        loop {
            self.skip_spaces();
            let Some(ch) = self.current() else { break };
            match ch {
                ')' => break,
                '!' => {
                    self.arriving = None;
                    // an unknown one at the start of a branch has no label to stay in: it is left out
                    self.submolecule()?;
                }
                '(' => {
                    let mut default = self.bisector.get(&cur).copied().unwrap_or(self.defaults.angle.unwrap_or(angle));
                    self.pos += 1;
                    self.skip_spaces();
                    // a branch may start with defaults of its own, valid in all of it (chemfig manual,
                    // section 2.6: `B([:60]-D-E)`, `([::-30,1.5]-X-Y)`); a relative angle is relative to
                    // the bond that reached the atom
                    let saved = (self.current() == Some('[')).then(|| {
                        let parent = std::mem::take(&mut self.defaults);
                        (parent, angle)
                    });
                    if let Some((parent, previous)) = &saved {
                        let args = split_top(&self.group('[', ']')?, ',');
                        let own = defaults_from(&args, *previous, parent);
                        if let Some(a) = own.angle {
                            default = a;
                        }
                        self.defaults = own;
                    }
                    self.chain(cur, default, None)?;
                    if let Some((parent, _)) = saved {
                        self.defaults = parent;
                    }
                    self.skip_spaces();
                    if self.current() == Some(')') {
                        self.pos += 1;
                    } else {
                        self.reader.tolerate(rules::E_REC_4, "branch '(' without its ')'", "closed at the end")?;
                    }
                }
                '*' => {
                    // a ring opened at a vertex a ring bond reached is fused to the parent's next side
                    let fused = match (ring, last_edge) {
                        (Some(r), Some(edge)) => Some((r.turn, edge)),
                        _ => None,
                    };
                    let incoming = if ring.is_some() { angle } else { self.bisector.get(&cur).copied().unwrap_or(angle) };
                    if let Some(shared) = self.ring(cur, incoming, fused)? {
                        if fused.is_some() {
                            preset = Some(shared);
                        }
                    }
                }
                '?' => {
                    self.pos += 1;
                    let spec = if self.current() == Some('[') { Some(self.group('[', ']')?) } else { None };
                    let piece = self.m.atoms[cur].anchor;
                    self.closure(cur, piece, spec)?;
                }
                '@' => {
                    self.pos += 1;
                    if self.current() == Some('{') {
                        let name = self.group('{', '}')?;
                        self.m.atoms[cur].hook.get_or_insert_with(|| name.split(',').next().unwrap_or("").trim().to_string());
                    }
                }
                c if is_bond_start(c) => {
                    let bond_start = self.pos;
                    let (kind, gaps, args) = self.bond_spec()?;
                    let bond_span = Some((bond_start, self.pos));
                    // a node on the bond, before its angle: [@{name,coeff}angle,…]
                    let (bond_hook, written) = bond_hook(args.first().map(|a| a.trim()).unwrap_or(""));
                    let mut bond_angle = match default_angle {
                        Some(d) if written.is_empty() => d,
                        _ => resolve_angle(written, angle),
                    };
                    let length = args.get(1).and_then(|l| l.trim().parse::<f32>().ok()).filter(|l| *l > 0.0).unwrap_or(self.defaults.length);
                    let (default_departure, default_arrival) = (self.defaults.departure, self.defaults.arrival);
                    let explicit = |i: usize| {
                        let given = args.get(i).and_then(|v| v.trim().parse::<usize>().ok()).filter(|v| *v > 0).map(|v| v - 1);
                        given.or(if i == 2 { default_departure } else { default_arrival })
                    };
                    let tikz = args.get(4).map(String::as_str).filter(|t| !t.trim().is_empty()).unwrap_or(&self.defaults.tikz);
                    let kind = tikz_kind(tikz, kind);
                    let mut target = None;
                    let mut ring_id = None;
                    if let Some(r) = ring {
                        if k >= r.bonds {
                            self.reader.tolerate(rules::E_REC_9, format!("ring of {} sides with more bonds than it can take", self.m.rings[r.id].size), "extra bond left out")?;
                            // read and drop what follows the extra bond
                            let _ = self.atom(None, None)?;
                            self.pending = Pending::default();
                            continue;
                        }
                        bond_angle = r.first + r.turn * k as f32;
                        ring_id = Some(r.id);
                        if k + 1 == r.bonds {
                            target = Some(r.close);
                        }
                    }
                    let departure = explicit(2).unwrap_or_else(|| self.default_departure(cur, bond_angle));
                    let to = match target {
                        Some(t) => {
                            // the label after a closing bond names the start atom again; it is not a new atom
                            let _ = self.label()?;
                            t
                        }
                        None => match preset.take() {
                            Some(p) => {
                                let span_start = self.pos;
                                let (label, lewis) = self.label()?;
                                let span = self.trimmed_span(span_start);
                                let atom = &mut self.m.atoms[p];
                                if span.1 > span.0 || atom.span.is_none() {
                                    atom.span = Some(span);
                                }
                                if atom.label.is_empty() && !label.is_empty() {
                                    atom.pieces = pieces(&label);
                                    atom.label = label;
                                }
                                atom.lewis.extend(lewis);
                                p
                            }
                            None => self.atom(Some(bond_angle), explicit(3))?,
                        },
                    };
                    let arrival = if target.is_some() { explicit(3).unwrap_or(self.m.atoms[to].anchor) } else { self.m.atoms[to].anchor };
                    self.add_bond(Bond {
                        from: cur,
                        to,
                        kind,
                        angle: normalize(bond_angle),
                        length,
                        departure,
                        arrival,
                        ring: ring_id,
                        start_gap: gaps.0,
                        end_gap: gaps.1,
                        hook: bond_hook,
                        span: bond_span,
                    });
                    self.attach(to)?;
                    if let Some(r) = ring {
                        if target.is_none() && !self.m.rings[r.id].atoms.contains(&to) {
                            self.m.rings[r.id].atoms.push(to);
                            // exterior bisector: halfway between this bond and the next, outward
                            let outward = bond_angle + r.turn / 2.0 - 90.0 * r.turn.signum();
                            self.bisector.entry(to).or_insert(normalize(outward));
                        }
                        k += 1;
                        last_edge = Some(bond_angle);
                    }
                    cur = to;
                    angle = bond_angle;
                }
                other => {
                    self.reader.tolerate(rules::E_REC_9, format!("unexpected '{other}' in a structure"), "ignored")?;
                    self.pos += 1;
                }
            }
        }
        Ok(())
    }

    /// `*n(…)` or `**n(…)` at `at`. A ring hanging from a bond has its center in that bond's direction.
    /// A ring opened at a vertex of another ring (`fused`: the parent's turn and the side that reached
    /// `at`) turns the same way and lies outside it, sharing the parent's next side: its last bond comes
    /// back to `at` along that side. Returns the atom at the far end of the shared side.
    fn ring(&mut self, at: AtomId, incoming: f32, fused: Option<(f32, f32)>) -> R<Option<AtomId>> {
        self.pos += 1;
        let aromatic = self.current() == Some('*');
        if aromatic {
            self.pos += 1;
        }
        // **[angle1,angle2,tikz]: the part of the circle drawn, and how (chemfig manual, section 3.2)
        let (mut arc, mut dashed) = (None, false);
        if aromatic && self.current() == Some('[') {
            let args = split_top(&self.group('[', ']')?, ',');
            let angle = |i: usize, default: f32| args.get(i).map(|a| a.trim()).filter(|a| !a.is_empty()).and_then(|a| a.parse::<f32>().ok()).unwrap_or(default);
            let (from, to) = (angle(0, 0.0), angle(1, 360.0));
            if (to - from).abs() < 360.0 {
                arc = Some((from, to));
            }
            let tikz = args.get(2..).map(|t| t.join(",")).unwrap_or_default();
            dashed = tikz.contains("dash") || tikz.contains("dotted");
        }
        let mut digits = String::new();
        while let Some(d) = self.current().filter(char::is_ascii_digit) {
            digits.push(d);
            self.pos += 1;
        }
        let size = digits.parse::<usize>().unwrap_or(0);
        if !(3..=40).contains(&size) {
            self.reader.tolerate(rules::E_REC_9, format!("ring of {size} sides (from 3 to 40)"), "read as a hexagon")?;
        }
        let size = if (3..=40).contains(&size) { size } else { 6 };
        self.skip_spaces();
        if self.current() != Some('(') {
            self.reader.tolerate(rules::E_REC_4, "ring without its '(…)'", "left empty")?;
            return Ok(None);
        }
        self.pos += 1;
        let step = 360.0 / size as f32;
        let id = self.m.rings.len();
        let (first, turn) = match fused {
            Some((parent_turn, edge)) => {
                let turn = parent_turn.signum() * step;
                (edge + parent_turn + 180.0 + turn, turn)
            }
            None => (incoming - 90.0 + 180.0 / size as f32, step),
        };
        self.m.rings.push(Ring { size, atoms: vec![at], aromatic, arc, dashed });
        let start_outward = first - turn / 2.0 - 90.0 * turn.signum();
        self.bisector.entry(at).or_insert(normalize(start_outward));
        let state = RingState { id, first, turn, bonds: size, close: at };
        self.chain(at, first, Some(state))?;
        self.skip_spaces();
        if self.current() == Some(')') {
            self.pos += 1;
        } else {
            self.reader.tolerate(rules::E_REC_4, "ring '(' without its ')'", "closed at the end")?;
        }
        // the vertex the ring's second-to-last side reaches: the far end of the side it shares
        let shared = self.m.rings[id].atoms.get(size - 1).copied();
        Ok(shared)
    }

    /// Adds a bond, or strengthens the one already between the two atoms: a side two fused rings both
    /// draw is one bond, with the higher order.
    fn add_bond(&mut self, b: Bond) {
        if let Some(old) = self.m.bonds.iter_mut().find(|o| (o.from == b.from && o.to == b.to) || (o.from == b.to && o.to == b.from)) {
            if b.kind.order() > old.kind.order() {
                old.kind = b.kind;
                old.ring = b.ring;
                if old.from != b.from {
                    std::mem::swap(&mut old.from, &mut old.to);
                    std::mem::swap(&mut old.departure, &mut old.arrival);
                    old.angle = b.angle;
                }
            }
            return;
        }
        self.m.bonds.push(b);
    }

    /// `?`, `?[name]`, `?[name,link,tikz]` (chemfig manual, section 2.8): the first mark of a name is
    /// the hook, and every later mark of the same name is bonded to it, with the link it gives (a bond
    /// number 1 to 9 or its characters in braces, `{=}`) and its TikZ options. `piece`: the piece of the
    /// label the mark follows.
    fn closure(&mut self, at: AtomId, piece: usize, spec: Option<String>) -> R<()> {
        let parts = spec.map(|s| split_top(&s, ',')).unwrap_or_default();
        let name = parts.first().map(|n| n.trim()).filter(|n| !n.is_empty()).unwrap_or("a").to_string();
        match self.closures.get(&name) {
            None => {
                self.closures.insert(name, (at, piece));
            }
            Some(&(other, other_piece)) => {
                let link = parts.get(1).map(String::as_str).unwrap_or("");
                let tikz = parts.get(2..).map(|t| t.join(",")).unwrap_or_default();
                let kind = tikz_kind(&tikz, hook_kind(link));
                self.add_bond(Bond {
                    from: other,
                    to: at,
                    kind,
                    angle: 0.0,
                    length: 1.0,
                    departure: other_piece,
                    arrival: piece,
                    ring: None,
                    start_gap: None,
                    end_gap: None,
                    hook: None,
                    span: None,
                });
            }
        }
        Ok(())
    }

    /// `!{name}`, `!\name` or `!x` with its arguments (chemfig manual, section 9), the current character
    /// being `!`: replaced in the input by the submolecule's code, as chemfig expands it. An unknown name
    /// warns (E-REC-1) and returns what was written, for a label to keep as it is.
    fn submolecule(&mut self) -> R<Option<String>> {
        let start = self.pos;
        self.pos += 1;
        let name = match self.current() {
            Some('{') => self.group('{', '}')?.trim().to_string(),
            Some('\\') => {
                let mut n = String::from('\\');
                self.pos += 1;
                while let Some(ch) = self.current().filter(char::is_ascii_alphabetic) {
                    n.push(ch);
                    self.pos += 1;
                }
                n
            }
            Some(ch) => {
                self.pos += 1;
                ch.to_string()
            }
            None => String::new(),
        };
        let Some(sub) = self.reader.submols.get(&name).cloned() else {
            self.reader.tolerate(rules::E_REC_1, format!("unknown submolecule !{{{name}}} (\\definesubmol defines one)"), "written as it is")?;
            return Ok(Some(self.c[start..self.pos].iter().collect()));
        };
        self.expansions += 1;
        if self.expansions > 200 {
            self.reader.tolerate(rules::E_REC_1, format!("submolecule !{{{name}}} expanded more than 200 times: does it call itself?"), "left out")?;
            return Ok(None);
        }
        let mut args = Vec::new();
        for _ in 0..sub.arguments {
            self.skip_spaces();
            args.push(match self.current() {
                Some('{') => self.group('{', '}')?,
                Some(ch) => {
                    self.pos += 1;
                    ch.to_string()
                }
                None => String::new(),
            });
        }
        // the code for a bond that comes from the right, when there is one: seen from the submolecule,
        // the bond goes back strictly between −90° and 90° (it is drawn leftward), as `\definesubmol\Me[H_3C]{CH_3}`
        // puts the carbon next to whatever it hangs from
        let from_right = self.arriving.is_some_and(|a| normalize(a).abs() > 90.0);
        let code = match (&sub.right, from_right) {
            (Some(right), true) => right.clone(),
            _ => sub.code.clone(),
        };
        // #1…#n are the arguments; any other # stays (a #(…) bond spacing, or a # drawn as it is)
        let mut text = String::new();
        let chars: Vec<char> = code.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            match (chars[i], chars.get(i + 1).and_then(|d| d.to_digit(10))) {
                ('#', Some(k)) if k >= 1 && (k as usize) <= sub.arguments => {
                    // an argument is tokens of its own: after a control word it does not lengthen its name
                    let arg = &args[k as usize - 1];
                    if ends_with_control_word(&text) && arg.starts_with(|c: char| c.is_ascii_alphabetic()) {
                        text.push(' ');
                    }
                    text.push_str(arg);
                    i += 2;
                }
                // any other #n is drawn as it is (the manual's "#3"); #( stays a bond's spacing
                ('#', Some(_)) => {
                    text.push_str("\\#");
                    i += 1;
                }
                (ch, _) => {
                    text.push(ch);
                    i += 1;
                }
            }
        }
        let end = self.pos;
        self.c.splice(start..end, text.chars());
        self.pos = start;
        Ok(None)
    }

    /// Gives an atom what its label asked for: its charges, `\chemskipalign`, and its `?` marks, once
    /// the bond that places it is drawn.
    fn attach(&mut self, id: AtomId) -> R<()> {
        let pending = std::mem::take(&mut self.pending);
        let atom = &mut self.m.atoms[id];
        atom.charges.extend(pending.charges);
        atom.skip_align |= pending.skip_align;
        atom.attributes.extend(pending.attributes);
        for (spec, piece) in pending.closures {
            self.closure(id, piece, spec)?;
        }
        Ok(())
    }

    /// The structure's `[angle,length,departure,arrival,tikz]` (chemfig manual, section 2.6); the
    /// current character is `[`.
    fn read_defaults(&mut self) -> R<()> {
        let args = split_top(&self.group('[', ']')?, ',');
        self.defaults = defaults_from(&args, 0.0, &Defaults::default());
        Ok(())
    }

    /// A bond character, its `#(dim1,dim2)` gaps and its optional `[angle,length,departure,arrival,tikz]`.
    fn bond_spec(&mut self) -> R<(BondKind, Gaps, Vec<String>)> {
        let ch = self.current().unwrap_or('-');
        self.pos += 1;
        let kind = match ch {
            // chemfig's offset double bonds: =^ to the left, =_ to the right
            '=' if self.current() == Some('^') => {
                self.pos += 1;
                BondKind::DoubleLeft
            }
            '=' if self.current() == Some('_') => {
                self.pos += 1;
                BondKind::DoubleRight
            }
            '=' => BondKind::Double,
            '~' => BondKind::Triple,
            '>' | '<' => {
                let back = ch == '<';
                let variant = self.current();
                let pick = |plain, back_kind| if back { back_kind } else { plain };
                match variant {
                    Some(':') => {
                        self.pos += 1;
                        pick(BondKind::Hash, BondKind::HashBack)
                    }
                    Some('|') => {
                        self.pos += 1;
                        pick(BondKind::Hollow, BondKind::HollowBack)
                    }
                    _ => pick(BondKind::Wedge, BondKind::WedgeBack),
                }
            }
            _ => BondKind::Single,
        };
        let gaps = self.bond_gaps()?;
        self.skip_spaces();
        let args = if self.current() == Some('[') { split_top(&self.group('[', ']')?, ',') } else { Vec::new() };
        Ok((kind, gaps, args))
    }

    /// chemfig manual, section 2.2: `#(dim1,dim2)` right after the bond sign (also written `\#` or
    /// `\CFhash`) sets the space between the bond and the letters at its start and at its end; a field
    /// left empty, or `dim2` left out, keeps the standard space. A dimension contextEquation cannot read warns.
    fn bond_gaps(&mut self) -> R<Gaps> {
        let rest: String = self.c[self.pos..].iter().take(8).collect();
        let marker = ["\\CFhash", "\\#", "#"].into_iter().find(|m| rest.starts_with(m));
        let Some(marker) = marker else { return Ok((None, None)) };
        let after = self.pos + marker.chars().count();
        if self.c.get(after) != Some(&'(') {
            return Ok((None, None));
        }
        self.pos = after;
        let spec = self.group('(', ')')?;
        let mut fields = spec.splitn(2, ',').map(str::trim);
        let mut gap = |field: Option<&str>| -> R<Option<f32>> {
            match field.filter(|f| !f.is_empty()) {
                None => Ok(None),
                Some(f) => match dimension_em(f) {
                    Some(em) => Ok(Some(em.clamp(-5.0, 5.0))),
                    None => {
                        self.reader.tolerate(rules::E_REC_9, format!("bond spacing #({spec}): unrecognized dimension {f}"), "the standard space")?;
                        Ok(None)
                    }
                },
            }
        };
        let start = gap(fields.next())?;
        let end = gap(fields.next())?;
        Ok((start, end))
    }

    /// Reads an atom label (possibly empty) and creates the atom. `arriving`: the direction of the bond
    /// that reaches it, which picks the piece it attaches to.
    fn atom(&mut self, arriving: Option<f32>, explicit: Option<usize>) -> R<AtomId> {
        self.arriving = arriving;
        let span_start = self.pos;
        let (label, lewis) = self.label()?;
        let span = Some(self.trimmed_span(span_start));
        let pieces = pieces(&label);
        let anchor = match explicit {
            Some(k) => k.min(pieces.len().saturating_sub(1)),
            None => default_arrival(&pieces, arriving),
        };
        let hook = self.hook.take();
        self.m.atoms.push(Atom { label, pieces, anchor, lewis, hook, span, ..Atom::default() });
        Ok(self.m.atoms.len() - 1)
    }

    /// From `start` to where the reader is, without the spaces at either end.
    fn trimmed_span(&self, start: usize) -> (usize, usize) {
        let (mut a, mut b) = (start, self.pos.min(self.c.len()));
        while a < b && self.c[a].is_whitespace() {
            a += 1;
        }
        while b > a && self.c[b - 1].is_whitespace() {
            b -= 1;
        }
        (a, b)
    }

    /// The raw label up to the next bond or structure character, without spaces; `\lewis{…,X}` gives X and its marks.
    fn label(&mut self) -> R<(String, Vec<Lewis>)> {
        let mut s = String::new();
        let mut lewis = Vec::new();
        while let Some(ch) = self.current() {
            match ch {
                c if c.is_whitespace() => self.pos += 1,
                // a submolecule, at the start of a label or inside it: its code is read in its place
                '!' => {
                    if let Some(written) = self.submolecule()? {
                        s.push_str(&written);
                    }
                }
                '@' => {
                    self.pos += 1;
                    if self.current() == Some('{') {
                        let name = self.group('{', '}')?;
                        self.hook = Some(name.split(',').next().unwrap_or("").trim().to_string());
                    }
                }
                c if STOPS.contains(&c) => break,
                // a distant bond's mark, on the piece it follows (on the next one at the start)
                '?' => {
                    self.pos += 1;
                    let spec = if self.current() == Some('[') { Some(self.group('[', ']')?) } else { None };
                    let piece = pieces(&s).len().saturating_sub(1);
                    self.pending.closures.push((spec, piece));
                }
                '{' => {
                    let inner = self.group('{', '}')?;
                    s.push('{');
                    s.push_str(&inner);
                    s.push('}');
                }
                '^' | '_' => {
                    // a script takes the next character even if it is a bond sign (^- is a charge), but
                    // not the parenthesis that closes a branch or a ring (#210)
                    s.push(ch);
                    self.pos += 1;
                    match self.current().filter(|n| !matches!(n, '(' | ')')) {
                        Some('{') => {
                            let inner = self.group('{', '}')?;
                            s.push('{');
                            s.push_str(&inner);
                            s.push('}');
                        }
                        Some(n) => {
                            s.push(n);
                            self.pos += 1;
                        }
                        None => {}
                    }
                }
                '\\' => {
                    self.pos += 1;
                    let mut name = String::new();
                    while let Some(l) = self.current().filter(char::is_ascii_alphabetic) {
                        name.push(l);
                        self.pos += 1;
                    }
                    if name.is_empty() {
                        if let Some(sym) = self.current() {
                            s.push('\\');
                            s.push(sym);
                            self.pos += 1;
                        }
                        continue;
                    }
                    match name.as_str() {
                        "lewis" | "Lewis" => {
                            self.skip_spaces();
                            let arg = if self.current() == Some('{') { self.group('{', '}')? } else { String::new() };
                            let (spec, inner) = arg.split_once(',').unwrap_or((arg.as_str(), ""));
                            lewis.extend(lewis_marks(spec));
                            s.push_str(inner.trim());
                        }
                        "charge" | "Charge" => {
                            let mut args = Vec::new();
                            for _ in 0..2 {
                                self.skip_spaces();
                                args.push(if self.current() == Some('{') { self.group('{', '}')? } else { String::new() });
                            }
                            let before = pieces(&s).len();
                            let piece = if args[1].trim().is_empty() { before.saturating_sub(1) } else { before };
                            let charges = self.charges(&args[0], piece)?;
                            self.pending.charges.extend(charges);
                            s.push_str(args[1].trim());
                        }
                        // a mark around the label (\htmlData, \class, \htmlId): read through, kept on the atom
                        "htmlData" | "class" | "htmlClass" | "htmlId" | "cssId" => {
                            let start = self.pos - name.chars().count() - 1;
                            let mut args = Vec::new();
                            for _ in 0..2 {
                                self.skip_spaces();
                                args.push(if self.current() == Some('{') { self.group('{', '}')? } else { String::new() });
                            }
                            self.pending.attributes.extend(crate::input::latex::html_attribute_list(&name, &args[0]));
                            // what the mark holds is read in its place, as if written there (\charge, \lewis…)
                            let end = self.pos;
                            self.c.splice(start..end, args[1].trim().chars());
                            self.pos = start;
                        }
                        // chemfig manual, section 5.4.3
                        "chemskipalign" => self.pending.skip_align = true,
                        "chemabove" | "chembelow" => {
                            let mut args = Vec::new();
                            for _ in 0..2 {
                                self.skip_spaces();
                                args.push(if self.current() == Some('{') { self.group('{', '}')? } else { String::new() });
                            }
                            let stack = if name == "chemabove" { "overset" } else { "underset" };
                            s.push_str(&format!("\\{stack}{{{}}}{{{}}}", args[1], args[0]));
                        }
                        _ => {
                            s.push('\\');
                            s.push_str(&name);
                            // braced arguments go with the command
                            while self.current() == Some('{') {
                                let inner = self.group('{', '}')?;
                                s.push('{');
                                s.push_str(&inner);
                                s.push('}');
                            }
                        }
                    }
                }
                _ => {
                    s.push(ch);
                    self.pos += 1;
                }
            }
        }
        Ok((s, lewis))
    }

    /// The list of `\charge{[params]position[tikz]=charge, …}` (chemfig manual, section 5.8): each
    /// position an angle in degrees or a TikZ anchor (`north east`), with `:offset`; each charge one of
    /// the Lewis shortcuts `\.`, `\:`, `\|`, `\"` or LaTeX text. Of the general parameters, `extra sep`
    /// is kept (in the offset), and of the TikZ options `anchor` (`anchor=-90`,
    /// `anchor=180+\chargeangle`); the others style the nodes and are left out.
    fn charges(&mut self, spec: &str, piece: usize) -> R<Vec<Charge>> {
        let mut spec = spec.trim();
        // of the general parameters, `extra sep` (1.5 pt by default) moves every position
        let mut extra = 0.0;
        if spec.starts_with('[') {
            let end = spec.find(']').map_or(spec.len(), |e| e + 1);
            for key in split_top(&spec[1..end.saturating_sub(1).max(1)], ',') {
                if let Some(v) = key.trim().strip_prefix("extra sep").map(|v| v.trim().trim_start_matches('=').trim()) {
                    extra = super::scheme::length_em(v).map_or(0.0, |v| v - 0.15);
                }
            }
            spec = &spec[end..];
        }
        let mut out = Vec::new();
        for item in split_top(spec, ',') {
            if item.trim().is_empty() {
                continue;
            }
            // the first '=' outside the TikZ options (`45[anchor=…]=$\oplus$`)
            let mut depth = 0i32;
            let equals = item.char_indices().find(|&(_, c)| {
                depth += match c {
                    '[' | '{' => 1,
                    ']' | '}' => -1,
                    _ => 0,
                };
                c == '=' && depth == 0
            });
            let Some((position, content)) = equals.map(|(i, _)| (&item[..i], &item[i + 1..])) else {
                self.reader.tolerate(rules::E_REC_9, format!("\\charge{{{item}}} without '='"), "left out")?;
                continue;
            };
            let (position, tikz) = match position.split_once('[') {
                Some((p, t)) => (p, t.trim_end().trim_end_matches(']')),
                None => (position, ""),
            };
            let (angle, offset) = position.split_once(':').unwrap_or((position, ""));
            let Some(angle) = charge_angle(angle.trim()) else {
                self.reader.tolerate(rules::E_REC_9, format!("\\charge position {:?}", angle.trim()), "left out")?;
                continue;
            };
            let anchor = match split_top(tikz, ',').iter().find_map(|o| o.trim().strip_prefix("anchor").map(|v| v.trim().trim_start_matches('=').trim().to_string())) {
                None => None,
                Some(a) if a.replace(' ', "") == "180+\\chargeangle" => Some(normalize(angle + 180.0)),
                Some(a) => match charge_angle(&a) {
                    Some(a) => Some(a),
                    None => {
                        self.reader.tolerate(rules::E_REC_9, format!("\\charge anchor {a:?}"), "the charge centered on its position")?;
                        None
                    }
                },
            };
            let offset = super::scheme::length_em(offset.trim()).unwrap_or(0.0) + extra;
            let content = match content.trim() {
                "\\." => ChargeContent::Mark { mark: LewisMark::Radical },
                "\\:" => ChargeContent::Mark { mark: LewisMark::Pair },
                "\\|" => ChargeContent::Mark { mark: LewisMark::Line },
                "\\\"" => ChargeContent::Mark { mark: LewisMark::Vacancy },
                tex => ChargeContent::Text { tex: tex.to_string() },
            };
            out.push(Charge { angle, offset, piece, anchor, content });
        }
        Ok(out)
    }

    /// The piece a bond leaves from by default: the last one going right, the first going left, and for
    /// a vertical bond the heavy atom (`H_3C` leaves from C).
    fn default_departure(&self, at: AtomId, angle: f32) -> usize {
        let pieces = &self.m.atoms[at].pieces;
        if pieces.len() <= 1 {
            return 0;
        }
        let x = angle.to_radians().cos();
        if x > 0.1 {
            pieces.len() - 1
        } else if x < -0.1 {
            0
        } else {
            heavy(pieces)
        }
    }
}

/// The piece a bond arrives at by default: the first coming from the left, the last coming from the
/// right, the heavy atom coming vertically or for the first atom.
fn default_arrival(pieces: &[String], arriving: Option<f32>) -> usize {
    if pieces.len() <= 1 {
        return 0;
    }
    match arriving.map(|a| a.to_radians().cos()) {
        Some(x) if x > 0.1 => 0,
        Some(x) if x < -0.1 => pieces.len() - 1,
        _ => heavy(pieces),
    }
}

/// The first piece that is not hydrogen (or the first piece).
fn heavy(pieces: &[String]) -> usize {
    pieces.iter().position(|p| !p.trim_start_matches('{').starts_with('H')).unwrap_or(0)
}

/// Splits a label into atoms as chemfig does: a capital letter starts a new one, lowercase letters,
/// digits, scripts, braces and symbol commands stay with the current one.
pub fn pieces(label: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let c: Vec<char> = label.chars().collect();
    let mut i = 0;
    let mut level = 0;
    while i < c.len() {
        let ch = c[i];
        let starts = level == 0 && (ch.is_ascii_uppercase() || (ch == '\\' && starts_piece(&c[i + 1..])));
        // a script's argument never starts a piece: H_2, ^{+}
        let after_script = i > 0 && matches!(c[i - 1], '_' | '^');
        if (starts && !after_script) || out.is_empty() {
            out.push(String::new());
        }
        match ch {
            '{' => level += 1,
            '}' => level -= 1,
            '\\' => {
                // the whole command name goes at once
                let last = out.last_mut().expect("a piece was just opened");
                last.push('\\');
                i += 1;
                while i < c.len() && c[i].is_ascii_alphabetic() {
                    last.push(c[i]);
                    i += 1;
                }
                continue;
            }
            _ => {}
        }
        out.last_mut().expect("a piece was just opened").push(ch);
        i += 1;
    }
    out.retain(|p| !p.is_empty());
    out
}

/// Commands that hold a whole atom (`\mathrm{Br}`, `\overset{+}{N}`) start a piece; symbols (`\oplus`) do not.
fn starts_piece(rest: &[char]) -> bool {
    let name: String = rest.iter().take_while(|c| c.is_ascii_alphabetic()).collect();
    matches!(name.as_str(), "mathrm" | "text" | "textrm" | "mathbf" | "textbf" | "overset" | "underset" | "mathit")
}

/// `\lewis{0:2.4|6o,…}`: a direction digit (0 to 7, in steps of 45°) and its mark.
fn lewis_marks(spec: &str) -> Vec<Lewis> {
    let mut out = Vec::new();
    let c: Vec<char> = spec.chars().collect();
    let mut i = 0;
    while i < c.len() {
        if let Some(d) = c[i].to_digit(10).filter(|d| *d < 8) {
            let mark = match c.get(i + 1) {
                Some('.') => LewisMark::Radical,
                Some('|') => LewisMark::Line,
                Some('o') => LewisMark::Vacancy,
                _ => LewisMark::Pair,
            };
            if matches!(c.get(i + 1), Some(':' | '.' | '|' | 'o')) {
                i += 1;
            }
            out.push(Lewis { angle: d as f32 * 45.0, mark });
        }
        i += 1;
    }
    out
}

/// A `\charge` position: degrees, or one of TikZ's anchors on the border of a node.
fn charge_angle(position: &str) -> Option<f32> {
    if let Ok(a) = position.parse::<f32>() {
        return Some(normalize(a));
    }
    Some(match position {
        "east" => 0.0,
        "north east" => 45.0,
        "north" => 90.0,
        "north west" => 135.0,
        "west" => 180.0,
        "south west" => -135.0,
        "south" => -90.0,
        "south east" => -45.0,
        _ => return None,
    })
}

/// The link of a distant bond: chemfig's bond number (1 to 9) or the bond's characters in braces.
fn hook_kind(link: &str) -> BondKind {
    match link.trim().trim_start_matches('{').trim_end_matches('}').trim() {
        "2" | "=" => BondKind::Double,
        "3" | "~" => BondKind::Triple,
        "4" | ">" => BondKind::Wedge,
        "5" | "<" => BondKind::WedgeBack,
        "6" | ">:" => BondKind::Hash,
        "7" | "<:" => BondKind::HashBack,
        "8" | ">|" => BondKind::Hollow,
        "9" | "<|" => BondKind::HollowBack,
        _ => BondKind::Single,
    }
}

/// chemfig's angle argument: empty keeps the previous direction, `n` is n × 45°, `:x` is absolute,
/// `::x` is relative to the previous bond.
fn resolve_angle(arg: &str, previous: f32) -> f32 {
    let a = arg.trim();
    if a.is_empty() {
        return previous;
    }
    if let Some(r) = a.strip_prefix("::") {
        return previous + r.trim().parse::<f32>().unwrap_or(0.0);
    }
    if let Some(x) = a.strip_prefix(':') {
        return x.trim().parse::<f32>().unwrap_or(previous);
    }
    a.parse::<f32>().map(|n| n * 45.0).unwrap_or(previous)
}

/// The fifth argument, TikZ options: `draw=none` hides the bond, a dash pattern dashes it.
fn tikz_kind(tikz: &str, kind: BondKind) -> BondKind {
    if tikz.contains("draw=none") {
        BondKind::Invisible
    } else if tikz.contains("dash") || tikz.contains("dotted") {
        BondKind::Dashed
    } else {
        kind
    }
}

/// An angle in (-180, 180].
pub fn normalize(a: f32) -> f32 {
    let mut a = a % 360.0;
    if a > 180.0 {
        a -= 360.0;
    } else if a <= -180.0 {
        a += 360.0;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_split_into_atoms() {
        assert_eq!(pieces("H_3C"), ["H_3", "C"]);
        assert_eq!(pieces("CH_2OH"), ["C", "H_2", "O", "H"]);
        assert_eq!(pieces("NH_3^+"), ["N", "H_3^+"]);
        assert_eq!(pieces("O\\ominus"), ["O\\ominus"]);
        assert_eq!(pieces("{CH_3}"), ["{CH_3}"]);
        assert_eq!(pieces("\\mathrm{Br}"), ["\\mathrm{Br}"]);
        assert_eq!(pieces("SO_4^{2-}"), ["S", "O_4^{2-}"]);
        assert!(pieces("").is_empty());
    }

    #[test]
    fn angles() {
        assert_eq!(resolve_angle("", 30.0), 30.0);
        assert_eq!(resolve_angle("2", 0.0), 90.0);
        assert_eq!(resolve_angle(":-30", 0.0), -30.0);
        assert_eq!(resolve_angle("::60", 30.0), 90.0);
        assert_eq!(normalize(270.0), -90.0);
        assert_eq!(normalize(-180.0), 180.0);
    }

    #[test]
    fn lewis_specs() {
        let marks = lewis_marks("0:2.4|6o");
        let kinds: Vec<_> = marks.iter().map(|l| (l.angle, l.mark)).collect();
        assert_eq!(kinds, [(0.0, LewisMark::Pair), (90.0, LewisMark::Radical), (180.0, LewisMark::Line), (270.0, LewisMark::Vacancy)]);
    }
}
