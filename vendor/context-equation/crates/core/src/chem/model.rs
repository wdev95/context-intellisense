//! Tree of meaning for chemistry: what is drawn, not where. A scheme is a sequence of compounds,
//! plus signs and reaction arrows; a molecule is a graph of atoms and bonds, with the author's
//! drawing directions (chemfig's angles) kept as hints for the layout.

use serde::Serialize;

/// Index of an atom in [`Molecule::atoms`].
pub type AtomId = usize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Scheme {
    pub items: Vec<Item>,
    /// Curved arrows of a mechanism (`\chemmove`), between named atoms (`@{name}`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub moves: Vec<ElectronMove>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Item {
    Compound(Compound),
    Plus,
    Arrow(Arrow),
    Merge(Merge),
}

/// chemfig's `\merge` (manual, section 6.11): a line from each compound it joins to a junction line
/// across the direction, and an arrow from that line on to the next compound.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Merge {
    /// The compounds joined, as indices among the scheme's compounds (from 0), each with the angle of
    /// the anchor its line leaves from, when one is given (`(c1.120)`); otherwise the direction's.
    pub from: Vec<(usize, Option<f32>)>,
    /// The direction: 0 (`>`), 180 (`<`), 90 (`^`), -90 (`v`).
    pub angle: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub above: Option<Label>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub below: Option<Label>,
    /// `[c1,c2,c]`: the shortest line to the junction and the arrow, as fractions of the space between
    /// compounds, and where on the junction line the arrow leaves (0: its top end, or its left end for
    /// `^` and `v`). 0.5 each by default.
    pub coefficients: (f32, f32, f32),
}

impl Merge {
    /// The arrow a merge ends in, as speech, text and braille read it: "reacts to give", with its labels.
    pub fn as_arrow(&self) -> Arrow {
        Arrow {
            kind: ArrowKind::Forward,
            above: self.above.clone(),
            below: self.below.clone(),
            angle: self.angle,
            length: 1.0,
            arc: None,
            from: None,
            to: None,
            shift: None,
            from_box: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Compound {
    pub body: Body,
    /// `\chemname{…}{name}`: written under the compound (LaTeX text, `$…$` for math).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Body {
    /// A structural formula (`\chemfig`).
    Structure(Molecule),
    /// A formula or any math, typeset by contextEquation: `\ce{2H2O}`, `$\Delta H$`.
    Math { tex: String },
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Molecule {
    pub atoms: Vec<Atom>,
    pub bonds: Vec<Bond>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rings: Vec<Ring>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Atom {
    /// The label as written (`H_3C`, `NH_2^+`); empty for a carbon at a vertex of the skeleton.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub label: String,
    /// The label split into atoms (`H_3C` → `H_3`, `C`): bonds attach to one of them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pieces: Vec<String>,
    /// The piece that sits on the atom's position (the one a bond arrives at).
    #[serde(skip_serializing_if = "is_zero")]
    pub anchor: usize,
    /// Lone pairs, radicals and pair lines around the atom (`\lewis`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lewis: Vec<Lewis>,
    /// `@{name}`: the name a mechanism arrow uses to point at this atom.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook: Option<String>,
    /// `\charge{90=\|,45=$\oplus$}{X}`: marks and text placed around one piece of the label.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub charges: Vec<Charge>,
    /// `\chemskipalign`: the label's baseline on the bonds' line instead of its letters' middle.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub skip_align: bool,
    /// The attributes of a mark around the label (`\htmlData{m=a}{OH}`): the label is drawn in a group
    /// that carries them, so an editor knows where the atom is; the label itself is read through it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attributes: Vec<(String, String)>,
    /// Where the label is written in the structure's code (characters, end excluded; empty for a
    /// vertex without one): what lets an editor change one atom in place. None after a submolecule.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<(usize, usize)>,
}

/// What chemfig's `\charge` puts around an atom (chemfig manual, section 5.8): at `angle` degrees on
/// the border of the piece's box (grown by 1.5 pt), `offset` em further out, the center of `content`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Charge {
    pub angle: f32,
    #[serde(skip_serializing_if = "is_zero_f32")]
    pub offset: f32,
    /// The piece of the label the charge is around.
    #[serde(skip_serializing_if = "is_zero")]
    pub piece: usize,
    /// TikZ's `anchor=`: the point of the content's border at this angle goes on the position, in
    /// place of its center (`anchor=180+\chargeangle` sets it just outside).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<f32>,
    pub content: ChargeContent,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ChargeContent {
    /// The Lewis shortcuts `\.`, `\:`, `\|` and `\"`.
    Mark { mark: LewisMark },
    /// Anything else: LaTeX text, `$…$` for math (`$\oplus$`, `$\delta^+$`).
    Text { tex: String },
}

fn is_zero(v: &usize) -> bool {
    *v == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Lewis {
    /// Direction in degrees, counterclockwise from the right.
    pub angle: f32,
    pub mark: LewisMark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LewisMark {
    /// `:` two dots
    Pair,
    /// `.` one dot
    Radical,
    /// `|` a short line for a pair
    Line,
    /// `o` an empty box: a vacancy
    Vacancy,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Bond {
    pub from: AtomId,
    pub to: AtomId,
    pub kind: BondKind,
    /// Drawing direction in degrees, counterclockwise from the right (chemfig's angle, resolved).
    pub angle: f32,
    /// Length as a multiple of the standard bond.
    #[serde(skip_serializing_if = "is_one")]
    pub length: f32,
    /// The piece of each label the bond leaves from and arrives at.
    #[serde(skip_serializing_if = "is_zero")]
    pub departure: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub arrival: usize,
    /// The ring this bond is a side of: a double bond is then drawn inside it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ring: Option<usize>,
    /// chemfig's `#(dim1,dim2)` (manual, section 2.2): the space left between the bond and the letters
    /// at its start and at its end, in em, instead of the standard one (C-LAY-3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_gap: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_gap: Option<f32>,
    /// `-[@{name,coeff}…]` (chemfig manual, section 5.8.1): a node on the bond, `coeff` of the way
    /// along it (0.5 by default), that a mechanism arrow can point at.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook: Option<(String, f32)>,
    /// Where the bond is written in the structure's code (its sign and its `[…]`, end excluded).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<(usize, usize)>,
}

fn is_one(v: &f32) -> bool {
    *v == 1.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BondKind {
    Single,
    Double,
    /// A double bond with one line on the bond's axis and the other on its left (`=^`) or right (`=_`),
    /// going the way the bond is drawn (chemfig's offset double bonds).
    DoubleLeft,
    DoubleRight,
    Triple,
    /// Cram wedge, narrow at the start, wide at the end (`>`); `WedgeBack` is the reverse (`<`).
    Wedge,
    WedgeBack,
    /// Hashed wedge (`>:`, `<:`).
    Hash,
    HashBack,
    /// Hollow wedge (`>|`, `<|`).
    Hollow,
    HollowBack,
    /// A dashed line: a partial or hydrogen bond.
    Dashed,
    /// Present in the graph, not drawn (`draw=none`).
    Invisible,
}

impl BondKind {
    /// How many electron pairs the bond shares, for the hydrogens a carbon carries.
    pub fn order(self) -> u32 {
        match self {
            BondKind::Double | BondKind::DoubleLeft | BondKind::DoubleRight => 2,
            BondKind::Triple => 3,
            BondKind::Dashed | BondKind::Invisible => 0,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ring {
    pub size: usize,
    /// The atoms around the ring, in drawing order.
    pub atoms: Vec<AtomId>,
    /// `**6(…)`: a circle inside for the delocalized bonds.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub aromatic: bool,
    /// `**[angle1,angle2,tikz]6(…)` (chemfig manual, section 3.2): the circle only from `angle1` to
    /// `angle2`, absolute, counterclockwise (0 and 360 by default, the whole circle).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arc: Option<(f32, f32)>,
    /// The circle dashed or dotted, as its TikZ options ask.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dashed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Arrow {
    pub kind: ArrowKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub above: Option<Label>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub below: Option<Label>,
    /// Direction in degrees (0: to the right, -90: down).
    #[serde(skip_serializing_if = "is_zero_f32")]
    pub angle: f32,
    #[serde(skip_serializing_if = "is_one")]
    pub length: f32,
    /// chemfig's `-U>` (manual, sections 6.2 and 6.9): a forward arrow with a circular arc tangent at
    /// its middle, from the label above (a reagent coming in) to the label below (a product going out).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arc: Option<UArc>,
    /// chemfig's branching, `\arrow(@name--…)` (manual, section 6.7): the compound the arrow leaves
    /// from, as an index among the scheme's compounds (from 0); the last one placed otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<usize>,
    /// `\arrow(…--@name)` (section 6.7): an arrow to a compound already placed, as an index; it draws
    /// straight between the two, and no new compound follows it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<usize>,
    /// `\arrow(--name[xshift=…,yshift=…])` (chemfig manual, section 6.6): the compound after the arrow
    /// moved by that much, in em (x to the right, y up); the arrow reaches it where it moved to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shift: Option<(f32, f32)>,
    /// `\arrow(@ck--…)` naming a `\subscheme`'s box (section 6.8): the arrow leaves from the box around
    /// those compounds, as indices from the first to the one after the last.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_box: Option<(usize, usize)>,
}

/// The arc of a `-U>` arrow: its radius as a multiple of the arrow's length (negative: below the
/// arrow) and the angle each half of it turns, in degrees.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct UArc {
    pub radius: f32,
    pub half_angle: f32,
}

fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

/// A label over or under a reaction arrow: text (`$…$` for math) or a structure (a reagent drawn).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Label {
    Text { tex: String },
    Structure(Molecule),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArrowKind {
    /// `->` reacts to give
    Forward,
    /// `<-`
    Backward,
    /// `<=>` equilibrium: two harpoons
    Equilibrium,
    /// `<->>` equilibrium favoring the products; `<<->` favoring the reactants
    EquilibriumRight,
    EquilibriumLeft,
    /// `<->` resonance: one shaft, a head at each end
    Resonance,
    /// `=>` retrosynthesis: an open double arrow
    Retro,
    /// `-/>` does not react: crossed
    NoReaction,
    /// `0` nothing drawn: only the labels, or space
    Invisible,
}

/// A curved arrow of a mechanism: an electron pair (or one electron) moving between two atoms.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ElectronMove {
    pub from: String,
    pub to: String,
    /// Leaving and arriving directions in degrees (TikZ's `controls +(θ:l)` and `out=`/`in=`); without
    /// them the arrow bends 45° to the left of the straight line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub out_angle: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_angle: Option<f32>,
    /// How far each control point reaches, in em; without it, 0.4 of the distance between the atoms.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reach: Option<(f32, f32)>,
    /// `bend left=θ` (positive) or `bend right=θ` (negative), when no angle is given.
    pub bend: f32,
    /// A fishhook (half head): one electron.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub single: bool,
}

/// chemfig's `\definesubmol{name}n[code1]{code2}` (manual, section 9): the code `!{name}` (or `!\name`)
/// stands for in a structure, with `#1`…`#n` its arguments; with `code1`, that one when the bond that
/// reaches it comes from the right (seen from the submolecule, strictly between −90° and 90°).
#[derive(Debug, Clone, PartialEq)]
pub struct Submol {
    pub arguments: usize,
    pub right: Option<String>,
    pub code: String,
}

/// The submolecules defined so far in the input, by name (`xy`, or `\xx` for a control sequence).
pub type Submols = std::collections::BTreeMap<String, Submol>;
