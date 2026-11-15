//! #176: the stmaryrd and mathabx symbols that Unicode has no character for, drawn by composition;
//! #267: mhchem's dashed bond and its bonds of more than one kind of line (`\bond{~}`, `\bond{~-}`,
//! `\bond{~=}`, `\bond{-~-}`) and of four dots (`\bond{....}`), which are not characters either.
//!
//! The tree keeps each one as an ordinary symbol node (`Operator`, `LargeOperator`, a delimiter of
//! `Delimited`) whose character is a marker from the Supplementary Private Use Area-A (U+F0B00 on):
//! the node stays the size it is, and every output asks this table what the marker stands for. The
//! tree's JSON writes the command in its place (`"symbol":"\\llceil"`, #207).
//! The layout draws it from the math font's own glyphs (overlapped, turned, mirrored or scaled) and,
//! where the package draws a stroke the font has no glyph for, from lines at the font's rule
//! thickness, as the package's METAFONT source draws them (`layout/composite.rs`). The MathML is the
//! S-MML-1 fallback, with `text` as the closest drawing; the linear text writes `text`; the speech
//! reads `meaning` as that character is read, or `text` as it is; braille writes the full cell
//! (S-BRL-1).

use crate::tree::Class;

/// One symbol drawn by composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Composite {
    /// The marker in the tree.
    pub symbol: char,
    /// The package's command, without the backslash.
    pub command: &'static str,
    /// The class the package declares (`\mathopen`, `\mathbin`…); `Op` for a large operator.
    pub class: Class,
    /// The closest text Unicode has: the linear text, and the drawing of the MathML fallback.
    pub text: &'static str,
    /// The character whose meaning it has, when it is that symbol drawn another way (a short arrow
    /// is an arrow): speech reads it as that character.
    pub meaning: Option<char>,
}

pub const LLCEIL: char = '\u{F0B00}';
pub const RRCEIL: char = '\u{F0B01}';
pub const LLFLOOR: char = '\u{F0B02}';
pub const RRFLOOR: char = '\u{F0B03}';
pub const VAROCIRCLE: char = '\u{F0B04}';
pub const VAROLESSTHAN: char = '\u{F0B05}';
pub const VAROGREATERTHAN: char = '\u{F0B06}';
pub const VAROVEE: char = '\u{F0B07}';
pub const VAROWEDGE: char = '\u{F0B08}';
pub const YDOWN: char = '\u{F0B09}';
pub const YLEFT: char = '\u{F0B0A}';
pub const YRIGHT: char = '\u{F0B0B}';
pub const SHORTLEFTARROW: char = '\u{F0B0C}';
pub const SHORTRIGHTARROW: char = '\u{F0B0D}';
pub const SHORTUPARROW: char = '\u{F0B0E}';
pub const SHORTDOWNARROW: char = '\u{F0B0F}';
pub const NNEARROW: char = '\u{F0B10}';
pub const ARROWNOT: char = '\u{F0B11}';
pub const MAPSFROMCHAR: char = '\u{F0B12}';
pub const TRIANGLELEFTEQSLANT: char = '\u{F0B13}';
pub const INPLUS: char = '\u{F0B14}';
pub const BIGBOX: char = '\u{F0B15}';
pub const BIGPARALLEL: char = '\u{F0B16}';
pub const BIGNPLUS: char = '\u{F0B17}';
pub const BIGCURLYVEE: char = '\u{F0B18}';
pub const BIGCURLYWEDGE: char = '\u{F0B19}';
pub const BIGBOXPLUS: char = '\u{F0B1A}';
pub const BIGOMINUS: char = '\u{F0B1B}';
pub const BIGOSLASH: char = '\u{F0B1C}';
pub const BOND_DASHED_OVER_SINGLE: char = '\u{F0B1D}';
pub const BOND_DASHED_OVER_DOUBLE: char = '\u{F0B1E}';
pub const BOND_DASHED_BETWEEN: char = '\u{F0B1F}';
pub const BOND_FOUR_DOTS: char = '\u{F0B20}';
pub const BOND_DASHED: char = '\u{F0B21}';

const fn c(symbol: char, command: &'static str, class: Class, text: &'static str, meaning: Option<char>) -> Composite {
    Composite { symbol, command, class, text, meaning }
}

/// Every symbol drawn by composition, in marker order.
pub const ALL: &[Composite] = {
    use Class::*;
    &[
        // stmaryrd (stmaryrd.dtx gives the classes; stmaryaj.mf and stmaryjg.mf the drawings)
        c(LLCEIL, "llceil", Open, "⌈⌈", None),
        c(RRCEIL, "rrceil", Close, "⌉⌉", None),
        c(LLFLOOR, "llfloor", Open, "⌊⌊", None),
        c(RRFLOOR, "rrfloor", Close, "⌋⌋", None),
        // the circular forms of \ocircle (amssymb's \circledcirc), \olessthan and \ogreaterthan
        c(VAROCIRCLE, "varocircle", Bin, "⊚", Some('⊚')),
        c(VAROLESSTHAN, "varolessthan", Bin, "⧀", Some('⧀')),
        c(VAROGREATERTHAN, "varogreaterthan", Bin, "⧁", Some('⧁')),
        // a vee and a wedge in a circle: the character with U+20DD, the combining enclosing circle
        c(VAROVEE, "varovee", Bin, "∨\u{20DD}", None),
        c(VAROWEDGE, "varowedge", Bin, "∧\u{20DD}", None),
        // stmaryjg.mf names them "Join (Y)", "-<" and ">-"
        c(YDOWN, "Ydown", Bin, "Y", None),
        c(YLEFT, "Yleft", Bin, "-<", None),
        c(YRIGHT, "Yright", Bin, ">-", None),
        c(SHORTLEFTARROW, "shortleftarrow", Rel, "←", Some('←')),
        c(SHORTRIGHTARROW, "shortrightarrow", Rel, "→", Some('→')),
        c(SHORTUPARROW, "shortuparrow", Rel, "↑", Some('↑')),
        c(SHORTDOWNARROW, "shortdownarrow", Rel, "↓", Some('↓')),
        c(NNEARROW, "nnearrow", Rel, "↗", None),
        // the slash that negates the arrow after it, and the bar that turns the arrow before it into ↤
        c(ARROWNOT, "arrownot", Rel, "⧸", None),
        c(MAPSFROMCHAR, "mapsfromchar", Rel, "|", None),
        c(TRIANGLELEFTEQSLANT, "trianglelefteqslant", Rel, "⊴", None),
        c(INPLUS, "inplus", Rel, "∈", None),
        c(BIGBOX, "bigbox", Op, "□", None),
        c(BIGPARALLEL, "bigparallel", Op, "∥", None),
        c(BIGNPLUS, "bignplus", Op, "⋂", None),
        // in both packages
        c(BIGCURLYVEE, "bigcurlyvee", Op, "⋎", None),
        c(BIGCURLYWEDGE, "bigcurlywedge", Op, "⋏", None),
        // mathabx (mathabx.dcl gives the classes; mathtest.pdf shows the drawings)
        c(BIGBOXPLUS, "bigboxplus", Op, "⊞", None),
        c(BIGOMINUS, "bigominus", Op, "⊖", None),
        c(BIGOSLASH, "bigoslash", Op, "⊘", None),
        // mhchem's bonds (mhchem.sty's \mhchem@bond table), written in \ce as \bond{…}: the command
        // is that, the text the closest Unicode drawing, and the speech says what the lines are
        c(BOND_DASHED_OVER_SINGLE, "bond{~-}", Ord, "≃", None),
        c(BOND_DASHED_OVER_DOUBLE, "bond{~=}", Ord, "≅", None),
        c(BOND_DASHED_BETWEEN, "bond{-~-}", Ord, "≋", None),
        c(BOND_FOUR_DOTS, "bond{....}", Ord, "····", None),
        c(BOND_DASHED, "bond{~}", Ord, "┄", None),
    ]
};

/// The symbol a marker stands for.
pub fn get(symbol: char) -> Option<&'static Composite> {
    let i = (symbol as u32).checked_sub(LLCEIL as u32)? as usize;
    ALL.get(i).filter(|k| k.symbol == symbol)
}

/// Whether a character is one of the markers.
pub fn is(symbol: char) -> bool {
    get(symbol).is_some()
}

/// The marker of a command (`llceil`), as a symbol.
pub fn command(name: &str) -> Option<&'static Composite> {
    ALL.iter().find(|k| k.command == name)
}

/// The source that reads as a symbol: `\llceil`, or for a bond `\ce{\bond{~-}}`.
pub fn source(k: &Composite) -> String {
    if k.command.starts_with("bond{") {
        format!("\\ce{{\\{}}}", k.command)
    } else {
        format!("\\{}", k.command)
    }
}

/// The phrase a symbol is read with, when it has its own (the bonds): a key of the locales.
pub fn speech_key(symbol: char) -> Option<&'static str> {
    Some(match symbol {
        BOND_DASHED_OVER_SINGLE => "chem.bond_dashed_over_single",
        BOND_DASHED_OVER_DOUBLE => "chem.bond_dashed_over_double",
        BOND_DASHED_BETWEEN => "chem.bond_dashed_between",
        BOND_FOUR_DOTS => "chem.bond_four_dots",
        BOND_DASHED => "chem.bond_dashed",
        _ => return None,
    })
}

/// The two characters stmaryrd overlaps for a doubled ceiling or floor.
pub fn doubled(symbol: char) -> Option<char> {
    Some(match symbol {
        LLCEIL => '⌈',
        RRCEIL => '⌉',
        LLFLOOR => '⌊',
        RRFLOOR => '⌋',
        _ => return None,
    })
}

/// `text` with every marker written as its closest text.
pub fn spell(text: &str) -> String {
    if !text.chars().any(is) {
        return text.to_string();
    }
    text.chars().fold(String::with_capacity(text.len()), |mut out, ch| {
        match get(ch) {
            Some(k) => out.push_str(k.text),
            None => out.push(ch),
        }
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_markers_follow_one_another() {
        for (i, k) in ALL.iter().enumerate() {
            assert_eq!(k.symbol as u32, LLCEIL as u32 + i as u32, "{}", k.command);
            assert_eq!(get(k.symbol), Some(k));
            assert_eq!(command(k.command), Some(k));
        }
        assert!(!is('⌈'));
        assert!(get(char::from_u32(LLCEIL as u32 + ALL.len() as u32).unwrap()).is_none());
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn a_symbol_built_at_run_time_is_the_one_in_the_table() {
        // `c` builds the table at compile time; at run time it builds the same symbol
        let k = c(LLCEIL, "llceil", Class::Open, "⌈⌈", None);
        assert_eq!(get(LLCEIL), Some(&k));
        assert_eq!(source(&k), r"\llceil");
    }
}
