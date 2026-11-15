//! L-CHE-2: the chemfig constructions inside an equation. The reader takes the whole construction as
//! written (`\chemfig{…}`, `\schemestart … \schemestop`, `\chemname{…}{…}`, and the `\chemmove{…}`
//! right after any of them) and hands it to the chemistry reader, which returns one node. `\charge`
//! and `\subscheme` outside a structure are a structure and a scheme of their own; chemfig's
//! delimiters (`\chemleft…\chemright`, `\chemup…\chemdown`) are written through contextEquation's.

use super::*;
use crate::chem;

impl Reader<'_> {
    /// `name` was just read; the reader is right after it.
    pub(super) fn chem_construction(&mut self, name: &str, col: usize) -> R<Node> {
        let mut source = format!("\\{name}");
        match name {
            "schemestart" => {
                let stop: Vec<char> = "\\schemestop".chars().collect();
                let start = self.pos;
                match (start..self.c.len()).find(|&i| self.c[i..].starts_with(&stop)) {
                    Some(end) => {
                        source.extend(&self.c[start..end]);
                        self.pos = end + stop.len();
                    }
                    None => {
                        self.tolerate(rules::E_REC_5, col, "\\schemestart without \\schemestop", "closed at the end")?;
                        source.extend(&self.c[start..]);
                        self.pos = self.c.len();
                    }
                }
            }
            // chemfig manual, section 5.8: \charge works outside \chemfig too, around one atom
            "charge" | "Charge" => {
                let positions = self.raw_text()?;
                let atom = self.raw_text()?;
                source = format!("\\chemfig{{\\{name}{{{positions}}}{{{atom}}}}}");
            }
            _ => {
                // \chemfig[options]{body}, \chemname[dim]{compound}{name}, \chemname*{…}{…}
                if name == "chemname" && self.current() == Some('*') {
                    self.pos += 1;
                }
                if let Some(options) = self.raw_optional() {
                    source.push_str(&format!("[{options}]"));
                }
                for _ in 0..if name == "chemname" { 2 } else { 1 } {
                    let arg = self.raw_text()?;
                    source.push_str(&format!("{{{arg}}}"));
                }
            }
        }
        // the mechanism arrows that follow belong to it
        loop {
            let saved = self.pos;
            self.skip_spaces();
            let next: String = self.c[self.pos..].iter().take(9).collect();
            if next != "\\chemmove" || self.c.get(self.pos + 9).is_some_and(char::is_ascii_alphabetic) {
                self.pos = saved;
                break;
            }
            self.pos += 9;
            let options = self.raw_optional().map(|o| format!("[{o}]")).unwrap_or_default();
            let body = self.raw_text()?;
            source.push_str(&format!(" \\chemmove{options}{{{body}}}"));
        }
        let mut reader = chem::scheme::Reader { strict: self.strict, registry: self.registry, column: col, submols: &self.macros.submols };
        let scheme = chem::scheme::read(&source, &mut reader)?;
        let formulas: Vec<String> = scheme
            .items
            .iter()
            .filter_map(|i| match i {
                chem::model::Item::Compound(chem::model::Compound { body: chem::model::Body::Structure(m), .. }) => Some(chem::formula::of(m).written()),
                _ => None,
            })
            .collect();
        let what = if formulas.is_empty() { "reaction scheme".to_string() } else { formulas.join(", ") };
        self.registry.note(rules::L_CHE_2, format!("\\{name}: {what}"));
        Ok(Node::Chemistry { scheme })
    }

    /// chemfig's delimiters (manual, section 6.8), `\chemskipalign` outside a structure (section 5.4.3,
    /// where it changes nothing) and `\merge` outside a scheme (section 6.11, where chemfig does not
    /// define it). `name` was just read; `start` is its backslash. Out of line, asked by exact name.
    #[inline(never)]
    pub(super) fn chem_delimiters(&mut self, name: &str, start: usize, col: usize) -> R<()> {
        match name {
            // \chemleft<d>…\chemright<d>: \left and \right, both sized to the material between them
            // the delimiter may come between braces, as in the manual's \chemleft{[}…\chemright{]}
            "chemleft" | "chemright" => {
                let side = if name == "chemleft" { "\\left" } else { "\\right" };
                self.skip_spaces();
                let braced = if self.current() == Some('{') { Some(self.raw_text()?) } else { None };
                match braced {
                    Some(d) => self.inject(start, &format!("{side}{} ", d.trim()), col),
                    None => self.inject(start, side, col),
                }
            }
            // \chemup<d>…\chemdown<d>: the delimiters turned a quarter, over and under the material
            "chemup" => {
                let open = self.raw_delimiter();
                let body_start = self.pos;
                let end = chem::scheme::delimited_end(&self.c, self.pos, "chemup", "chemdown");
                let close_word: Vec<char> = "\\chemdown".chars().collect();
                let Some(close_at) = (body_start..end).rev().find(|&i| self.c[i..].starts_with(&close_word)) else {
                    self.tolerate(rules::E_REC_5, col, "\\chemup without \\chemdown", "the delimiter left out")?;
                    return self.inject(start, "", col);
                };
                let body: String = self.c[body_start..close_at].iter().collect();
                self.pos = close_at + close_word.len();
                let close = self.raw_delimiter();
                let mut tex = format!("{{{body}}}");
                for (delimiter, over) in [(close, false), (open, true)] {
                    match chem_up_down(&delimiter, over) {
                        Some("") => {}
                        Some(command) => tex = format!("\\{command}{{{tex}}}"),
                        None => self.tolerate(rules::E_REC_9, col, format!("\\chemup or \\chemdown with the delimiter {delimiter:?}"), "left out")?,
                    }
                }
                self.inject(start, &tex, col)
            }
            "chemdown" => {
                let delimiter = self.raw_delimiter();
                self.tolerate(rules::E_REC_9, col, format!("\\chemdown{delimiter} without \\chemup"), "ignored")?;
                self.inject(start, "", col)
            }
            "chemskipalign" => self.inject(start, "", col),
            _ => {
                let end = chem::scheme::merge_end(&self.c, self.pos);
                let written: String = self.c[start..end].iter().collect();
                self.pos = end;
                let problem = format!("unsupported command: {} (chemfig defines it only inside \\schemestart)", written.trim());
                self.tolerate(rules::E_REC_1, col, problem, "left out")?;
                self.inject(start, "", col)
            }
        }
    }

    /// A delimiter as written after `\chemup` or `\chemdown`: a character or a control sequence.
    fn raw_delimiter(&mut self) -> String {
        self.skip_spaces();
        let start = self.pos;
        if self.current() == Some('\\') {
            self.pos += 1;
            let letters = self.c[self.pos..].iter().take_while(|x| x.is_ascii_alphabetic()).count();
            self.pos += letters.max(1);
        } else if self.current().is_some() {
            self.pos += 1;
        }
        self.c[start..self.pos.min(self.c.len())].iter().collect()
    }
}

/// The command that draws a `\chemup` delimiter over the material (`over`) or a `\chemdown` one under
/// it: `[` and `]` a bracket, `(` and `)` a parenthesis, `\{` and `\}` a brace, `.` nothing.
fn chem_up_down(delimiter: &str, over: bool) -> Option<&'static str> {
    Some(match (delimiter, over) {
        (".", _) => "",
        ("[" | "]" | "\\lbrack" | "\\rbrack", true) => "overbracket",
        ("[" | "]" | "\\lbrack" | "\\rbrack", false) => "underbracket",
        ("(" | ")", true) => "overparen",
        ("(" | ")", false) => "underparen",
        ("\\{" | "\\}" | "\\lbrace" | "\\rbrace", true) => "overbrace",
        ("\\{" | "\\}" | "\\lbrace" | "\\rbrace", false) => "underbrace",
        _ => return None,
    })
}
