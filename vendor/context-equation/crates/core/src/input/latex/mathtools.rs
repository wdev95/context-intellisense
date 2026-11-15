//! mathtools' commands and environments written through others (mathtools manual, v1.30): split
//! fractions, `\adjustlimits`, `\MoveEqLeft`, `\Aboxed`, `\ArrowBetweenLines`, the cramped commands,
//! `\mathmakebox`, the small matrices with delimiters, `multlined`, `lgathered`, `rgathered`,
//! `spreadlines` and `crampedsubarray`. Each one becomes LaTeX contextEquation already reads, so the tree and
//! every output come from the commands it already has.

use super::*;

/// The commands `mathtools_expansion` reads.
const COMMANDS: [&str; 26] = [
    "splitfrac",
    "splitdfrac",
    "cramped",
    "crampedllap",
    "crampedrlap",
    "crampedclap",
    "crampedsubstack",
    "adjustlimits",
    "MoveEqLeft",
    "Aboxed",
    "ArrowBetweenLines",
    "xlongrightarrow",
    "xLongrightarrow",
    "xlongleftarrow",
    "xLongleftarrow",
    "nuparrow",
    "ndownarrow",
    "vcentercolon",
    "ordinarycolon",
    "mathmbox",
    "mathmakebox",
    "refeq",
    "newtagform",
    "renewtagform",
    "providetagform",
    "usetagform",
];

/// The environments `mathtools_environment` reads (and their starred forms).
const ENVIRONMENTS: [&str; 10] =
    ["psmallmatrix", "bsmallmatrix", "Bsmallmatrix", "vsmallmatrix", "Vsmallmatrix", "multlined", "lgathered", "rgathered", "spreadlines", "crampedsubarray"];

/// Is `name` (with, for `\begin`, the environment at `c[pos]`) one `mathtools_expansion` reads?
/// `package_expansion` asks it first, so the frame of `mathtools_expansion` stays off the stack of
/// every other command, \frac and \sqrt among them, at the deepest level of their nesting.
pub(super) fn is_mathtools(name: &str, c: &[char], pos: usize) -> bool {
    match name {
        "begin" => ENVIRONMENTS.contains(&environment_name(c, pos).trim().trim_end_matches('*')),
        _ => COMMANDS.contains(&name),
    }
}

impl Reader<'_> {
    /// The mathtools command `name`, one `is_mathtools` accepts, written through other commands; None
    /// when the environment is one the input has redefined. Out of line: `package_expansion` is asked for every command.
    #[inline(never)]
    pub(super) fn mathtools_expansion(&mut self, name: &str, col: usize) -> R<Option<String>> {
        Ok(Some(match name {
            // \splitfrac{a}{b}: a fraction with no rule, the first line set to the left and the second
            // to the right, a \quad apart, in text style (\splitdfrac in display style)
            "splitfrac" | "splitdfrac" => {
                let (a, b) = (self.raw_text()?, self.raw_text()?);
                let (style, level) = if name == "splitfrac" { ("\\textstyle", 1) } else { ("\\displaystyle", 0) };
                format!("\\genfrac{{}}{{}}{{0pt}}{{{level}}}{{{style} {a}\\quad}}{{{style}\\quad\\mathstrut {b}}}")
            }
            // the cramped style is the one TeX already gives subscripts, lower limits, denominators and
            // radicands, where these commands go; contextEquation keeps the style it is in ([style] chooses one)
            "cramped" => {
                let style = self.raw_optional().unwrap_or_default();
                format!("{{{style} {}}}", self.raw_text()?)
            }
            "crampedllap" | "crampedrlap" | "crampedclap" => {
                let style = self.raw_optional().unwrap_or_default();
                format!("\\math{}{{{style} {}}}", &name[7..], self.raw_text()?)
            }
            "crampedsubstack" => format!("\\substack{{{}}}", self.raw_text()?),
            // \adjustlimits\lim_{n}\sup_{p}: each operator as deep as the other and each limit as tall
            // and as deep as the other, so the two limits sit on one line
            "adjustlimits" => match self.adjusted_limits()? {
                Some(text) => text,
                None => {
                    self.tolerate(rules::E_REC_9, col, "\\adjustlimits takes two operators with limits, as in \\adjustlimits\\lim_{n}\\sup_{p}", "ignored")?;
                    String::new()
                }
            },
            // \MoveEqLeft[n]: the line starts n em to the left of the alignment point of the lines
            // below; it stands for the first & of the line
            "MoveEqLeft" => {
                let n = self.raw_optional().and_then(|n| n.trim().replace(',', ".").parse::<f32>().ok()).filter(|n| n.is_finite()).unwrap_or(2.0);
                format!("\\hspace{{{n}em}}&\\hspace{{{}em}}", -n)
            }
            // \Aboxed{a &= b}: a box around both sides of the alignment point. The left cell is as wide
            // as `a` plus the box's padding and line (0.3em and 0.04em, \fboxsep and \fboxrule), and the
            // box laps out of it to the right, as mathtools kerns it. The right cell ends where the box
            // does (#251): the box is an ordinary atom, so a relation after it gets its thick space
            "Aboxed" => {
                let raw = self.raw_text()?;
                let (left, right) = split_at_ampersand(&raw).unwrap_or((&raw, ""));
                format!("\\mathrlap{{\\boxed{{{left}{right}}}}}\\hphantom{{{left}{{}}}}\\hspace{{0.34em}}&\\hphantom{{{{}}{right}}}\\hspace{{0.34em}}")
            }
            // \ArrowBetweenLines[symbol]: a line of its own with the arrow (⇕ by default) in the first
            // column; the starred form puts it in the fourth, the column mathtools' example leaves
            // empty at the right of an alignat{2}
            "ArrowBetweenLines" => {
                let star = self.current() == Some('*');
                if star {
                    self.pos += 1;
                }
                let symbol = self.raw_optional().unwrap_or_else(|| "\\Updownarrow".into());
                if star {
                    format!("&&&{symbol}\\\\")
                } else {
                    format!("{symbol}\\\\")
                }
            }
            // extarrows, which mathtools' manual points to: the long arrows that stretch under their labels
            "xlongrightarrow" | "xLongrightarrow" | "xlongleftarrow" | "xLongleftarrow" => {
                let short = match name {
                    "xlongrightarrow" => "xrightarrow",
                    "xLongrightarrow" => "xRightarrow",
                    "xlongleftarrow" => "xleftarrow",
                    _ => "xLeftarrow",
                };
                let below = self.raw_optional().map(|b| format!("[{b}]")).unwrap_or_default();
                format!("\\{short}{below}{{{}}}", self.raw_text()?)
            }
            // the negated vertical arrows: struck out, as MathJax draws them
            "nuparrow" => "\\mathrel{\\cancel{\\uparrow}}".into(),
            "ndownarrow" => "\\mathrel{\\cancel{\\downarrow}}".into(),
            "vcentercolon" => "\\vcentcolon ".into(),
            "ordinarycolon" => ":".into(),
            // boxes in math: the content in the style it is in
            "mathmbox" => format!("{{{}}}", self.raw_text()?),
            "mathmakebox" => {
                let width = self.raw_optional().filter(|w| measure_em(w).is_some());
                let position = self.raw_optional().unwrap_or_default();
                let content = self.raw_text()?;
                match width {
                    None => format!("{{{content}}}"),
                    Some(w) => {
                        let half = measure_em(&w).unwrap_or(0.0) / 2.0;
                        match position.trim() {
                            "l" => format!("{{\\mathrlap{{{content}}}\\hspace{{{w}}}}}"),
                            "r" => format!("{{\\hspace{{{w}}}\\mathllap{{{content}}}}}"),
                            _ => format!("{{\\hspace{{{half}em}}\\mathclap{{{content}}}\\hspace{{{half}em}}}}"),
                        }
                    }
                }
            }
            // \refeq{key}: the number of the label, without \eqref's parentheses
            "refeq" => format!("\\ref{{{}}}", self.raw_text()?),
            // the tag forms change how \tag and the automatic numbers look; contextEquation does not number
            // equations, so they are read and ignored
            "newtagform" | "renewtagform" | "providetagform" => {
                self.raw_text()?;
                self.raw_optional();
                self.raw_text()?;
                self.raw_text()?;
                String::new()
            }
            "usetagform" => {
                self.raw_text()?;
                String::new()
            }
            // `\begin`, the one name left that `is_mathtools` accepts: one of the ENVIRONMENTS
            _ => return self.mathtools_environment(col),
        }))
    }

    /// `\adjustlimits op₁_{a} op₂_{b}`, after the command: the two operators and their limits, written
    /// as mathtools does; None when they are not there (nothing read).
    fn adjusted_limits(&mut self) -> R<Option<String>> {
        let saved = self.pos;
        let mut parts = Vec::with_capacity(6);
        for k in 0..6 {
            self.skip_spaces();
            if k % 3 == 1 {
                match self.current() {
                    Some(mark @ ('_' | '^')) => {
                        self.pos += 1;
                        parts.push(mark.to_string());
                    }
                    _ => {
                        self.pos = saved;
                        return Ok(None);
                    }
                }
            } else if self.current().is_none() {
                self.pos = saved;
                return Ok(None);
            } else {
                parts.push(self.raw_text()?);
            }
        }
        let [op1, m1, l1, op2, m2, l2] = &parts[..] else { return Ok(None) };
        Ok(Some(format!("\\mathop{{{op1}\\vphantom{{{op2}}}}}{m1}{{{l1}\\vphantom{{{l2}}}}}\\mathop{{{op2}\\vphantom{{{op1}}}}}{m2}{{{l2}\\vphantom{{{l1}}}}}")))
    }

    /// The mathtools environments, after `\begin`: written as the environments contextEquation reads, with their
    /// body. None (nothing read) for any other environment, or one the equation defined itself.
    fn mathtools_environment(&mut self, col: usize) -> R<Option<String>> {
        let saved = self.pos;
        let name = self.raw_text()?.trim().to_string();
        let base = name.trim_end_matches('*');
        let delimiters = match base {
            "psmallmatrix" => Some(("(", ")")),
            "bsmallmatrix" => Some(("[", "]")),
            "Bsmallmatrix" => Some(("\\{", "\\}")),
            "vsmallmatrix" => Some(("|", "|")),
            "Vsmallmatrix" => Some(("\\|", "\\|")),
            _ => None,
        };
        let known = delimiters.is_some() || matches!(base, "multlined" | "lgathered" | "rgathered" | "spreadlines" | "crampedsubarray");
        if !known || self.macros.environments.contains_key(&name) {
            self.pos = saved;
            return Ok(None);
        }
        let text = if let Some((open, close)) = delimiters {
            // \begin{psmallmatrix*}[r]: the alignment goes to smallmatrix*
            let star = if name.ends_with('*') { "*" } else { "" };
            let alignment = if star.is_empty() { String::new() } else { self.raw_optional().map(|a| format!("[{a}]")).unwrap_or_default() };
            let body = self.environment_body(&name, col)?;
            format!("\\left{open}\\begin{{smallmatrix{star}}}{alignment}{body}\\end{{smallmatrix{star}}}\\right{close}")
        } else {
            match base {
                // [position][width]: where the block sits and how wide it is; contextEquation sets it as wide as
                // its lines, first at the left and last at the right, as multline does
                "multlined" => {
                    self.raw_optional();
                    self.raw_optional();
                    let body = self.environment_body(&name, col)?;
                    format!("\\begin{{multline*}}{body}\\end{{multline*}}")
                }
                // gathered with its lines at the left (in the left-aligned column of aligned) or right
                "lgathered" | "rgathered" => {
                    self.raw_optional();
                    let body = self.environment_body(&name, col)?;
                    let body = if base == "lgathered" { format!("&{}", left_aligned_rows(&body)) } else { body };
                    format!("\\begin{{aligned}}{body}\\end{{aligned}}")
                }
                // the space between lines is read and not used: contextEquation keeps its own
                "spreadlines" => {
                    self.raw_text()?;
                    format!("{{{}}}", self.environment_body(&name, col)?)
                }
                _ => {
                    let spec = self.raw_text()?;
                    let body = self.environment_body(&name, col)?;
                    format!("\\begin{{subarray}}{{{spec}}}{body}\\end{{subarray}}")
                }
            }
        };
        Ok(Some(text))
    }
}

/// The two sides of `a &= b` at its first `&` outside braces.
fn split_at_ampersand(raw: &str) -> Option<(&str, &str)> {
    let mut depth = 0usize;
    let mut escaped = false;
    for (i, c) in raw.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            '&' if depth == 0 => return Some((&raw[..i], &raw[i + 1..])),
            _ => {}
        }
    }
    None
}

/// The rows of `body` with a `&` after each `\\` that ends a row (outside braces and nested
/// environments, after its `[space]` or `*`): each line goes to the second column of `aligned`.
fn left_aligned_rows(body: &str) -> String {
    let c: Vec<char> = body.chars().collect();
    let mut out = String::new();
    let (mut depth, mut environments, mut i) = (0usize, 0usize, 0);
    while i < c.len() {
        if c[i] == '\\' {
            let name: String = c[i + 1..].iter().take_while(|x| x.is_ascii_alphabetic()).collect();
            match name.as_str() {
                "begin" => environments += 1,
                "end" => environments = environments.saturating_sub(1),
                _ => {}
            }
            if name.is_empty() && c.get(i + 1) == Some(&'\\') && depth == 0 && environments == 0 {
                out.push_str("\\\\");
                i += 2;
                if c.get(i) == Some(&'*') {
                    out.push('*');
                    i += 1;
                }
                let spaces = c[i..].iter().take_while(|x| x.is_whitespace()).count();
                if c.get(i + spaces) == Some(&'[') {
                    let end = c[i..].iter().position(|x| *x == ']').map_or(c.len(), |p| i + p + 1);
                    out.extend(&c[i..end]);
                    i = end;
                }
                out.push('&');
                continue;
            }
            let length = 1 + name.chars().count().max(1);
            out.extend(&c[i..(i + length).min(c.len())]);
            i += length;
            continue;
        }
        match c[i] {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        out.push(c[i]);
        i += 1;
    }
    out
}
