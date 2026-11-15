//! README banners, drawn by contextEquation itself: every letter, the tagline and the equations come
//! from the embedded font, with no text or external font. The same version always writes the same bytes.
//!
//! `cargo run -p contextEquation --example banner` writes `docs/banner.svg` (contextEquation's),
//! `docs/editor-banner.svg` (the editor's) and `docs/vscode-banner.svg` (the VS Code extension's), and
//! `docs/vscode-icon.svg` (the extension's icon); `tests/banner.rs` checks that they are up to date.

use context_equation_core::{convert, font, Options};

const WIDTH: f32 = 1280.0;
const HEIGHT: f32 = 320.0;

const BOARD: &str = "#21403A";
const CHALK: &str = "#F2EFE4";
const DIM: &str = "#BFD0C8";
const YELLOW: &str = "#F4E59A";
const PINK: &str = "#F4B6BE";
const WOOD: &str = "#8A5B33";
/// The empty places, as the editor marks them.
const ORANGE: &str = "#F2A65A";
/// The editor's window and its buttons, a shade lighter than the board.
const PANEL: &str = "#2B4F48";

/// contextEquation's SVG for `tex`, placed with its baseline at `(x, base)` and 1 em = `em` px.
/// Returns the element and the width it takes, in px.
fn equation(tex: &str, x: f32, base: f32, em: f32, color: &str) -> (String, f32) {
    let svg = convert(tex, &Options::default()).expect("the banner equations are valid").svg;
    // viewBox="left -top width top+bottom", in font units (1000 per em); left is below 0 when ink
    // passes the start of the box, which stays at x
    let vb: Vec<f32> = svg.split("viewBox=\"").nth(1).unwrap().split('"').next().unwrap().split(' ').map(|n| n.parse().unwrap()).collect();
    let per_unit = em / font::font().units_per_em();
    let (width, height) = (vb[2] * per_unit, vb[3] * per_unit);
    let top = base + vb[1] * per_unit;
    let placed = svg.replacen("<svg ", &format!(r#"<svg x="{}" y="{}" "#, num(x + vb[0] * per_unit), num(top)), 1);
    let placed = placed.replacen(r#"width=""#, r#"data-w=""#, 1).replacen(r#"height=""#, r#"data-h=""#, 1);
    let placed = placed.replacen("<svg ", &format!(r#"<svg width="{}" height="{}" "#, num(width), num(height)), 1);
    (format!(r#"<g color="{color}">{placed}</g>"#), width)
}

/// contextEquation's SVG for `tex` set at `em` px: its width, its height above the baseline and its depth
/// below it, in px, and the boxes of what it marks with `\htmlData`, in its own units.
fn measure(tex: &str, em: f32) -> (f32, f32, f32, Vec<context_equation_core::output::boxes::GroupBox>) {
    let out = convert(tex, &Options::default()).expect("the banner equations are valid");
    let vb: Vec<f32> = out.svg.split("viewBox=\"").nth(1).unwrap().split('"').next().unwrap().split(' ').map(|n| n.parse().unwrap()).collect();
    let per_unit = em / font::font().units_per_em();
    (vb[2] * per_unit, -vb[1] * per_unit, (vb[3] + vb[1]) * per_unit, out.boxes)
}

/// `tex` centered on `(cx, cy)`.
fn centered(tex: &str, cx: f32, cy: f32, em: f32, color: &str) -> String {
    let (width, above, below, _) = measure(tex, em);
    equation(tex, cx - width / 2.0, cy + (above - below) / 2.0, em, color).0
}

fn num(v: f32) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

pub fn generate() -> String {
    let mut body = String::new();

    body.push_str(&equation(r"\mathit{contextEquation}", 84.0, 176.0, 124.0, CHALK).0);
    body.push_str(&equation(r"\text{LaTeX to accessible SVG, no TeX install}", 88.0, 246.0, 26.0, DIM).0);

    // on the right, what it draws: a derivation, written down the board
    let right = 700.0;
    body.push_str(&equation(r"ax^2 + bx + c = 0", right, 62.0, 27.0, CHALK).0);
    body.push_str(&equation(r"x^2 + \frac{b}{a}x = -\frac{c}{a}", right, 126.0, 27.0, CHALK).0);
    body.push_str(&equation(r"\left(x + \frac{b}{2a}\right)^2 = \frac{b^2 - 4ac}{4a^2}", right, 196.0, 27.0, YELLOW).0);
    body.push_str(&equation(r"x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}", right, 268.0, 27.0, PINK).0);

    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" role="img" aria-label="contextEquation: LaTeX to accessible SVG, no TeX install"><rect width="{w}" height="{h}" fill="{BOARD}"/>{body}<rect y="304" width="{w}" height="16" fill="{WOOD}"/><rect x="1120" y="294" width="46" height="10" rx="3" fill="{CHALK}"/><rect x="1176" y="294" width="30" height="10" rx="3" fill="{PINK}"/></svg>
"#,
        w = num(WIDTH),
        h = num(HEIGHT),
    )
}

/// The editor's banner: its name on the left, on the right a window of the editor with a row of
/// palette buttons and a formula being built, an empty place to fill and the cursor after it.
pub fn generate_editor() -> String {
    let mut body = String::new();

    body.push_str(&equation(r"\mathit{contextEquation}", 84.0, 150.0, 104.0, CHALK).0);
    body.push_str(&equation(r"\text{editor}", 90.0, 214.0, 46.0, YELLOW).0);
    body.push_str(&equation(r"\text{Build a formula in a box, drawn by contextEquation}", 90.0, 266.0, 24.0, DIM).0);

    // the window
    let (left, top, right, bottom) = (700.0, 36.0, 1196.0, 284.0);
    body.push_str(&format!(
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="14" fill="{PANEL}" stroke="{DIM}" stroke-opacity="0.5" stroke-width="1.5"/>"#,
        num(left),
        num(top),
        num(right - left),
        num(bottom - top)
    ));
    // a row of palette buttons, each drawn by contextEquation, the empty places in orange
    let place = format!(r"\color{{{ORANGE}}}{{\square}}");
    let buttons = [
        format!(r"\frac{{{place}}}{{{place}}}"),
        format!(r"\sqrt{{{place}}}"),
        format!(r"{place}^{{{place}}}"),
        r"\sum".to_string(),
        r"\int".to_string(),
        format!(r"\begin{{pmatrix}}{place}&{place}\\{place}&{place}\end{{pmatrix}}"),
        r"\alpha".to_string(),
        r"\ce{H2O}".to_string(),
    ];
    let (size, gap) = (48.0, 10.0);
    let row = (right - left - 48.0 - (buttons.len() as f32 - 1.0) * gap) / buttons.len() as f32;
    for (k, tex) in buttons.iter().enumerate() {
        let x = left + 24.0 + k as f32 * (row + gap);
        body.push_str(&format!(
            r#"<rect x="{}" y="{}" width="{}" height="{size}" rx="8" fill="{BOARD}" stroke="{DIM}" stroke-opacity="0.35"/>"#,
            num(x),
            num(top + 20.0),
            num(row)
        ));
        // a wide button (the matrix) is set smaller, to fit
        let em = 20.0_f32.min(20.0 * (row - 12.0) / measure(tex, 20.0).0);
        body.push_str(&centered(tex, x + row / 2.0, top + 20.0 + size / 2.0, em, CHALK));
    }
    body.push_str(&format!(r#"<rect x="{}" y="{}" width="{}" height="1" fill="{DIM}" fill-opacity="0.35"/>"#, num(left), num(top + 88.0), num(right - left)));
    // the formula, its empty place marked to find where the cursor goes after it
    let formula = format!(r"\int_0^1 x^2\,dx = \frac{{1}}{{\htmlData{{m=c}}{{{place}}}}}");
    let em = 36.0;
    let (width, above, below, boxes) = measure(&formula, em);
    let (x, base) = ((left + right - width) / 2.0, (top + 88.0 + bottom) / 2.0 + (above - below) / 2.0);
    body.push_str(&equation(&formula, x, base, em, CHALK).0);
    let per_unit = em / font::font().units_per_em();
    let mark = boxes.iter().find(|b| b.attributes.iter().any(|(k, v)| k == "data-m" && v == "c")).expect("the empty place is marked");
    let caret = x + (mark.x + mark.width) * per_unit + 5.0;
    body.push_str(&format!(
        r#"<rect x="{}" y="{}" width="2.5" height="{}" rx="1" fill="{YELLOW}"/>"#,
        num(caret),
        num(base + mark.y * per_unit - 2.0),
        num(mark.height * per_unit + 4.0)
    ));

    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" role="img" aria-label="contextEquation editor: build a formula in a box, drawn by contextEquation"><rect width="{w}" height="{h}" fill="{BOARD}"/>{body}<rect y="304" width="{w}" height="16" fill="{WOOD}"/><rect x="1120" y="294" width="46" height="10" rx="3" fill="{CHALK}"/><rect x="1176" y="294" width="30" height="10" rx="3" fill="{ORANGE}"/></svg>
"#,
        w = num(WIDTH),
        h = num(HEIGHT),
    )
}

/// The VS Code extension's banner: its name on the left, on the right an editor window with a line of
/// LaTeX and, under the pointer, the hover that draws it with what a screen reader says.
pub fn generate_vscode() -> String {
    let mut body = String::new();

    body.push_str(&equation(r"\mathit{contextEquation}", 84.0, 150.0, 104.0, CHALK).0);
    body.push_str(&equation(r"\text{for VS Code}", 90.0, 214.0, 46.0, YELLOW).0);
    body.push_str(&equation(r"\text{Equations drawn as you write Markdown and LaTeX}", 90.0, 266.0, 24.0, DIM).0);

    // the window, its title bar with a tab
    let (left, top, right, bottom) = (680.0, 28.0, 1200.0, 286.0);
    body.push_str(&format!(
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="14" fill="{PANEL}" stroke="{DIM}" stroke-opacity="0.5" stroke-width="1.5"/>"#,
        num(left),
        num(top),
        num(right - left),
        num(bottom - top)
    ));
    for (k, color) in [PINK, YELLOW, DIM].iter().enumerate() {
        body.push_str(&format!(r#"<circle cx="{}" cy="{}" r="6" fill="{color}"/>"#, num(left + 22.0 + k as f32 * 20.0), num(top + 19.0)));
    }
    body.push_str(&equation(r"\text{notes.md}", left + 96.0, top + 24.0, 15.0, CHALK).0);
    body.push_str(&format!(r#"<rect x="{}" y="{}" width="{}" height="1" fill="{DIM}" fill-opacity="0.35"/>"#, num(left), num(top + 38.0), num(right - left)));

    // a line of the document: its number and its LaTeX, as written
    let tex = r"\int_0^1 x^2\,dx = \frac{1}{3}";
    let base = top + 74.0;
    body.push_str(&equation(r"\text{1}", left + 20.0, base, 16.0, DIM).0);
    let source = r"\texttt{\$\textbackslash int\_0\textasciicircum 1 x\textasciicircum 2\textbackslash ,dx = \textbackslash frac\{1\}\{3\}\$}";
    let code = left + 48.0;
    body.push_str(&equation(source, code, base, 17.0, CHALK).0);

    // the hover under it: the equation drawn, then the sentence a screen reader says
    let (hl, ht, hr, hb) = (left + 40.0, base + 22.0, right - 28.0, bottom - 18.0);
    body.push_str(&format!(
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="8" fill="{BOARD}" stroke="{DIM}" stroke-opacity="0.5"/>"#,
        num(hl),
        num(ht),
        num(hr - hl),
        num(hb - ht)
    ));
    body.push_str(&centered(tex, (hl + hr) / 2.0, ht + 52.0, 32.0, CHALK));
    body.push_str(&format!(r#"<rect x="{}" y="{}" width="{}" height="1" fill="{DIM}" fill-opacity="0.35"/>"#, num(hl), num(hb - 42.0), num(hr - hl)));
    let speech = convert(tex, &Options::default()).expect("the banner equations are valid").speech;
    body.push_str(&equation(&format!(r"\textit{{{speech}}}",), hl + 18.0, hb - 16.0, 15.0, DIM).0);

    // the pointer, its tip just under the LaTeX it hovers
    let (px, py) = (code + 84.0, base + 3.0);
    body.push_str(&format!(
        r#"<path transform="translate({} {})" d="M0 0 L0 19 L4.6 14.8 L7.7 21.6 L10.6 20.3 L7.6 13.7 L13.6 13.5 Z" fill="{CHALK}" stroke="{BOARD}" stroke-width="1.4" stroke-linejoin="round"/>"#,
        num(px),
        num(py)
    ));

    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" role="img" aria-label="contextEquation for VS Code: equations drawn as you write Markdown and LaTeX"><rect width="{w}" height="{h}" fill="{BOARD}"/>{body}<rect y="304" width="{w}" height="16" fill="{WOOD}"/><rect x="1120" y="294" width="46" height="10" rx="3" fill="{CHALK}"/><rect x="1176" y="294" width="30" height="10" rx="3" fill="{YELLOW}"/></svg>
"#,
        w = num(WIDTH),
        h = num(HEIGHT),
    )
}

/// The VS Code extension's icon: contextEquation's italic m in chalk on the board, over the ledge with its chalk.
pub fn generate_icon() -> String {
    let size = 256.0;
    let m = centered(r"\mathit{m}", size / 2.0, 112.0, 210.0, CHALK);
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {s} {s}" width="{s}" height="{s}" role="img" aria-label="contextEquation"><clipPath id="icon-board"><rect width="{s}" height="{s}" rx="44"/></clipPath><g clip-path="url(#icon-board)"><rect width="{s}" height="{s}" fill="{BOARD}"/>{m}<rect y="214" width="{s}" height="42" fill="{WOOD}"/><rect x="146" y="196" width="50" height="18" rx="5" fill="{CHALK}"/><rect x="204" y="196" width="30" height="18" rx="5" fill="{YELLOW}"/></g></svg>
"#,
        s = num(size),
    )
}

#[allow(dead_code)]
fn main() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs");
    std::fs::write(format!("{dir}/banner.svg"), generate()).expect("writes docs/banner.svg");
    std::fs::write(format!("{dir}/editor-banner.svg"), generate_editor()).expect("writes docs/editor-banner.svg");
    std::fs::write(format!("{dir}/vscode-banner.svg"), generate_vscode()).expect("writes docs/vscode-banner.svg");
    std::fs::write(format!("{dir}/vscode-icon.svg"), generate_icon()).expect("writes docs/vscode-icon.svg");
    println!("docs/banner.svg docs/editor-banner.svg docs/vscode-banner.svg docs/vscode-icon.svg");
}
