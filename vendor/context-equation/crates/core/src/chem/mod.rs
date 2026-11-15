//! Chemistry beyond one-line formulas (L-CHE-2, spec/chemistry.md): structural formulas in chemfig's
//! language, reaction schemes and mechanism arrows. The LaTeX reader hands each construction here and
//! gets back one [`Node::Chemistry`](crate::tree::Node), which the layout draws as a box of the
//! equation and the outputs read by its molecular formulas and its arrows. Built with the `chem`
//! feature, on by default.

pub mod chemfig;
pub mod draw;
pub mod formula;
pub mod layout;
pub mod model;
pub mod scheme;
pub mod speech;

pub use model::Scheme;

/// The molecular formula of every structure in a tree, in order (C-FOR-1): `["C₂H₆O"]`.
pub fn formulas(node: &crate::tree::Node) -> Vec<String> {
    let mut out = Vec::new();
    collect(node, &mut out);
    out
}

fn collect(node: &crate::tree::Node, out: &mut Vec<String>) {
    match node {
        crate::tree::Node::Chemistry { scheme } => {
            for item in &scheme.items {
                if let model::Item::Compound(model::Compound { body: model::Body::Structure(m), .. }) = item {
                    out.push(formula::of(m).written());
                }
            }
        }
        crate::tree::Node::Row { children } => children.iter().for_each(|c| collect(c, out)),
        _ => {}
    }
}
