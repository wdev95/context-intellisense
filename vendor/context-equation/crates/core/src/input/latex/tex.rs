//! Plain TeX and amsmath constructs of older material: the generalized fractions with delimiters
//! (`\atopwithdelims` and its kin), `\root…\of`, `\leftroot` and `\uproot`, `\buildrel…\over`,
//! `\skew`, `\overunderset`, `\vline` and the `xalignat`, `xxalignat` and `darray` environments.
//! Except for the fractions and `\overunderset`, each one is written through commands contextEquation already
//! reads.
//!
//! Also plain TeX's small fonts (`\sevenrm`, `\fiverm`, `\sevenbf`, `\fivebf`), the size commands
//! written as environments in math (`\begin{tiny}…\end{tiny}`) and youngtab's `\yng`, which contextEquation
//! leaves out (spec/boundary.md).

use super::*;

impl Reader<'_> {
    /// TeX's generalized fractions: `\over`, `\atop`, `\above<dimen>`, their forms with delimiters
    /// (`\overwithdelims()`, `\atopwithdelims[]`, `\abovewithdelims..<dimen>`) and `\choose`,
    /// `\brack`, `\brace`. What came before in the group is the numerator, the rest of the group the
    /// denominator; the stop that ended the group is returned.
    #[inline(never)]
    pub(super) fn generalized_fraction(&mut self, name: &str, col: usize, items: &mut Vec<Node>) -> R<Stop> {
        let num = Node::row(std::mem::take(items));
        let (base, delimiters) = match name.strip_suffix("withdelims") {
            Some(base) => (base, Some((self.delimiter()?, self.delimiter()?))),
            None => (name, None),
        };
        // \above<dimen>: TeX reads a dimension, written bare (2pt) or, as KaTeX allows, braced
        let thickness = if base == "above" {
            self.skip_spaces();
            let raw = if self.current() == Some('{') {
                self.raw_text()?
            } else {
                let start = self.pos;
                while self.current().is_some_and(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-')) {
                    self.pos += 1;
                }
                while self.current().is_some_and(|c| c.is_ascii_alphabetic()) && self.pos - start < 12 {
                    self.pos += 1;
                    if measure_em(&self.c[start..self.pos].iter().collect::<String>()).is_some() {
                        break;
                    }
                }
                self.c[start..self.pos].iter().collect()
            };
            Some(measure_em(&raw).ok_or(Error::Syntax { column: col, message: format!("\\{name} needs a dimension, not {raw:?}") })?.max(0.0))
        } else {
            None
        };
        let (remainder, p) = self.list()?;
        let den = Node::row(remainder);
        let rule = matches!(base, "over" | "above");
        let fraction = Node::Fraction { numerator_align: None, num: Box::new(num), den: Box::new(den), style: FractionStyle::Auto, rule, thickness };
        let delimiters = match base {
            "choose" => Some((Some('('), Some(')'))),
            "brack" => Some((Some('['), Some(']'))),
            "brace" => Some((Some('{'), Some('}'))),
            _ => delimiters.filter(|d| *d != (None, None)),
        };
        items.push(match delimiters {
            Some((open, close)) => Node::Delimited { open, close, parts: vec![fraction], middles: vec![] },
            None => fraction,
        });
        Ok(p)
    }

    /// The commands of this file written through others; None when `name` is not one of them (or,
    /// for `\sqrt` and `\root`, when they are written as usual), with nothing read.
    #[inline(never)]
    pub(super) fn tex_expansion(&mut self, name: &str, col: usize) -> R<Option<String>> {
        Ok(Some(match name {
            // only a root with an index is looked at, out of line: \sqrt is common
            "sqrt" if self.c[self.pos..].iter().find(|c| !c.is_whitespace()) == Some(&'[') => return self.shifted_root_index(col),
            // plain TeX: \root 3 \of {x}, the index up to \of and the radicand after it
            "root" => {
                let saved = self.pos;
                self.skip_spaces();
                let Some(index) = (self.current() != Some('[')).then(|| self.raw_until("of")).flatten() else {
                    self.pos = saved;
                    return Ok(None);
                };
                format!("\\sqrt[{{{index}}}]{{{}}}", self.raw_text()?)
            }
            // amsmath allows them only in the index of a root
            "leftroot" | "uproot" => {
                self.raw_text()?;
                self.tolerate(rules::E_REC_9, col, format!("\\{name} outside the index of \\sqrt"), "ignored")?;
                String::new()
            }
            // plain TeX: \buildrel a \over b is b with a over it, a relation (\stackrel)
            "buildrel" => {
                let Some(above) = self.raw_until("over") else {
                    self.tolerate(rules::E_REC_7, col, "\\buildrel without its \\over", "ignored")?;
                    return Ok(Some(String::new()));
                };
                format!("\\stackrel{{{above}}}{{{}}}", self.raw_text()?)
            }
            // plain TeX: \skew{n}\hat{x}, the accent n mu to the right: it goes over the base and n mu of
            // space, which is taken back after it
            "skew" => {
                let n = self.raw_text()?;
                let n: f32 = n.trim().parse().map_err(|_| Error::Syntax { column: col, message: format!("\\skew takes a number of mu, not {n:?}") })?;
                let accent = self.raw_text()?;
                let base = self.raw_text()?;
                format!("{{{accent}{{{base}\\mkern{{{n}mu}}}}\\mkern{{{}mu}}}}", -n)
            }
            // a rule as tall as an array's row (its strut, times \arraystretch), where it is written
            "vline" => {
                let s = self.array_stretch();
                format!("\\rule[{}em]{{0.04em}}{{{}em}}", -0.36 * s, 1.2 * s)
            }
            "begin" => return self.tex_environment(col),
            _ => return Ok(None),
        }))
    }

    /// The commands of this file that make their own node; None when `name` is not one of them. Asked
    /// after the other groups, by `verbatim_command`.
    #[inline(never)]
    pub(super) fn tex_command(&mut self, name: &str, items: &mut Vec<Node>) -> R<Option<Option<Stop>>> {
        let node = match name {
            // KaTeX: \overunderset{above}{below}{base}, both at once over and under the base, which
            // keeps its class (as \overset and \underset)
            "overunderset" => {
                let above = self.argument()?;
                let below = self.argument()?;
                let base = self.base_with_class()?;
                let class = natural_class(&base);
                Node::ForcedClass { class, base: Box::new(Node::Stacked { base: Box::new(base), above: Some(Box::new(above)), below: Some(Box::new(below)) }) }
            }
            _ => return self.plain_font_or_young(name, items),
        };
        items.push(node);
        Ok(Some(None))
    }

    /// Plain TeX's font switches of its smaller sizes, for the rest of the group: `\sevenrm` (cmr7, the
    /// 7-point roman beside 10-point text) is `\scriptstyle\rm`, `\fiverm` (cmr5) `\scriptscriptstyle\rm`,
    /// and `\sevenbf`, `\fivebf` the same in bold (plain.tex, "\font\sevenrm=cmr7" and its kin).
    /// youngtab's `\yng(2,1)` is outside contextEquation (spec/boundary.md): an error in strict mode; in
    /// tolerant mode its row lengths are read with it and the command is written in red (E-REC-1).
    /// None when `name` is neither, with nothing read. Out of line, asked last by `tex_command`.
    #[inline(never)]
    fn plain_font_or_young(&mut self, name: &str, items: &mut Vec<Node>) -> R<Option<Option<Stop>>> {
        let (level, variant) = match name {
            "sevenrm" => (2, Variant::Upright),
            "fiverm" => (3, Variant::Upright),
            "sevenbf" => (2, Variant::Bold),
            "fivebf" => (3, Variant::Bold),
            "yng" => return self.young_diagram(items).map(|()| Some(None)),
            _ => return Ok(None),
        };
        self.variant = variant;
        let (remainder, p) = self.list()?;
        items.push(Node::Style { level, base: Box::new(Node::row(remainder)) });
        Ok(Some(Some(p)))
    }

    /// youngtab's `\yng(row lengths)`: a Young diagram, rows of boxes sharing their sides. No node of
    /// contextEquation draws it as the package does (a table's vertical rules run the whole column, and a
    /// `\multicolumn` carries none), so it is left out (spec/boundary.md) rather than drawn otherwise.
    fn young_diagram(&mut self, items: &mut Vec<Node>) -> R<()> {
        let col = self.column();
        self.skip_spaces();
        let lengths = if self.current() == Some('(') { Some(self.raw_delimited('(', ')')) } else { None };
        let written = format!("\\yng({})", lengths.unwrap_or_default());
        self.tolerate(rules::E_REC_1, col, format!("unsupported command: {written} (youngtab's Young diagrams are outside contextEquation)"), "drawn in red")?;
        items.push(Node::Colored { color: "#c0392b".into(), base: Box::new(Node::Text { parts: vec![TextPart::Text(written)] }) });
        Ok(())
    }

    /// The raw text up to the control word `\word` outside braces (which is consumed); None, with
    /// nothing read, when it does not come.
    fn raw_until(&mut self, word: &str) -> Option<String> {
        let target: Vec<char> = word.chars().collect();
        let mut depth = 0usize;
        let mut i = self.pos;
        while i < self.c.len() {
            match self.c[i] {
                '\\' if depth == 0 && self.c[i + 1..].starts_with(&target) && !self.c.get(i + 1 + target.len()).is_some_and(char::is_ascii_alphabetic) => {
                    let raw: String = self.c[self.pos..i].iter().collect();
                    self.pos = i + 1 + target.len();
                    return Some(raw);
                }
                '\\' => i += 1,
                '{' => depth += 1,
                '}' if depth == 0 => return None,
                '}' => depth -= 1,
                _ => {}
            }
            i += 1;
        }
        None
    }

    /// amsmath's `\sqrt[\leftroot{a}\uproot{b} n]{x}`: the index n moved a mu to the left and b mu up,
    /// in mu of the text size, as amsmath measures them. None (nothing read) for a root without them.
    #[inline(never)]
    fn shifted_root_index(&mut self, col: usize) -> R<Option<String>> {
        let saved = self.pos;
        let Some(index) = self.raw_optional().filter(|i| i.contains("\\leftroot") || i.contains("\\uproot")) else {
            self.pos = saved;
            return Ok(None);
        };
        let c: Vec<char> = index.chars().collect();
        let (mut left, mut up, mut rest, mut i) = (0.0f32, 0.0f32, String::new(), 0);
        while i < c.len() {
            let word = ["leftroot", "uproot"].into_iter().find(|w| c[i] == '\\' && c[i + 1..].starts_with(&w.chars().collect::<Vec<_>>()));
            let Some(word) = word else {
                rest.push(c[i]);
                i += 1;
                continue;
            };
            i += 1 + word.len();
            while c.get(i).is_some_and(|x| x.is_whitespace()) {
                i += 1;
            }
            let amount: String = if c.get(i) == Some(&'{') {
                let end = c[i..].iter().position(|x| *x == '}').map_or(c.len(), |p| i + p);
                let a = c[i + 1..end].iter().collect();
                i = (end + 1).min(c.len());
                a
            } else {
                let n = c[i..].iter().take_while(|x| x.is_ascii_digit() || **x == '-').count();
                let a = c[i..i + n].iter().collect();
                i += n;
                a
            };
            match amount.trim().parse::<f32>() {
                Ok(v) if word == "leftroot" => left = v,
                Ok(v) => up = v,
                Err(_) => self.tolerate(rules::E_REC_9, col, format!("\\{word}{{{amount}}}: not a number"), "ignored")?,
            }
        }
        // the index is set in script-script size: its mu are scaled back to the text's
        let scale = self.font.get().script_script_scale();
        let mu = |v: f32| v / 18.0 / scale;
        let mut shifted = rest.trim().to_string();
        if up != 0.0 {
            shifted = format!("\\raisebox{{{}em}}{{${shifted}$}}", mu(up));
        }
        if left != 0.0 {
            // a phantom space, which the text and the speech leave out
            shifted = format!("{shifted}\\hphantom{{\\kern{{{}em}}}}", mu(left));
        }
        Ok(Some(format!("\\sqrt[{{{shifted}}}]")))
    }

    /// `xalignat`, `xxalignat` (amsmath's obsolete forms of `alignat`, spread over the line width,
    /// which contextEquation does not have) and KaTeX's `darray` (an array in display style), after `\begin`.
    /// None (nothing read) for any other environment, or one the equation defined itself.
    fn tex_environment(&mut self, col: usize) -> R<Option<String>> {
        let saved = self.pos;
        let name = self.raw_text()?.trim().to_string();
        let size = matches!(name.as_str(), "tiny" | "scriptsize" | "footnotesize" | "small" | "normalsize" | "large" | "Large" | "LARGE" | "huge" | "Huge");
        if !(size || matches!(name.as_str(), "xalignat" | "xalignat*" | "xxalignat" | "darray")) || self.macros.environments.contains_key(&name) {
            self.pos = saved;
            return Ok(None);
        }
        if size {
            return self.size_environment(&name, col).map(Some);
        }
        let star = name.ends_with('*') || name == "xxalignat";
        let argument = self.raw_text()?;
        let body = self.environment_body(&name, col)?;
        Ok(Some(if name == "darray" {
            format!("\\begin{{array}}{{{argument}}}\\displaystyle {}\\end{{array}}", display_cells(&body))
        } else {
            let alignat = if star { "alignat*" } else { "alignat" };
            format!("\\begin{{{alignat}}}{{{argument}}}{body}\\end{{{alignat}}}")
        }))
    }
}

impl Reader<'_> {
    /// `\begin{tiny}…\end{tiny}` in math: the size command over the body, which L-ENT-3 ignores. Out of
    /// line, so the frame of `tex_environment`, asked at every `\begin`, does not grow.
    #[inline(never)]
    fn size_environment(&mut self, name: &str, col: usize) -> R<String> {
        let body = self.environment_body(name, col)?;
        Ok(format!("{{\\{name} {body}}}"))
    }
}

/// The body of an array with `\displaystyle` at the start of each cell: after each `&` and each `\\`
/// (and its `[space]`) outside braces and nested environments.
fn display_cells(body: &str) -> String {
    let c: Vec<char> = body.chars().collect();
    let mut out = String::new();
    let (mut depth, mut environments, mut i) = (0usize, 0usize, 0);
    while i < c.len() {
        let top = depth == 0 && environments == 0;
        match c[i] {
            '\\' => {
                let name: String = c[i + 1..].iter().take_while(|x| x.is_ascii_alphabetic()).collect();
                match name.as_str() {
                    "begin" => environments += 1,
                    "end" => environments = environments.saturating_sub(1),
                    _ => {}
                }
                if name.is_empty() && c.get(i + 1) == Some(&'\\') && top {
                    out.push_str("\\\\");
                    i += 2;
                    let spaces = c[i..].iter().take_while(|x| x.is_whitespace()).count();
                    if c.get(i + spaces) == Some(&'[') {
                        let end = c[i..].iter().position(|x| *x == ']').map_or(c.len(), |p| i + p + 1);
                        out.extend(&c[i..end]);
                        i = end;
                    }
                    out.push_str("\\displaystyle ");
                    continue;
                }
                let length = 1 + name.chars().count().max(1);
                out.extend(&c[i..(i + length).min(c.len())]);
                i += length;
                continue;
            }
            '&' if top => {
                out.push_str("&\\displaystyle ");
                i += 1;
                continue;
            }
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        out.push(c[i]);
        i += 1;
    }
    out
}
