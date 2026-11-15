//! xcolor's colors: the models of `\definecolor` and `\color[model]`, the expressions of `\colorlet`
//! and `\color{red!40!blue}`, and the colors the equation defined, written as the SVG's `#rrggbb`.

use std::collections::HashMap;

/// A color the equation defined (`\definecolor`, `\colorlet`): what the SVG writes and, when contextEquation
/// knows them, its red, green and blue from 0 to 1 (a CSS name such as `RoyalBlue` has none).
#[derive(Clone, Debug)]
pub(super) struct Defined {
    pub css: String,
    pub rgb: Option<[f32; 3]>,
}

pub(super) type Colors = HashMap<String, Defined>;

/// xcolor's base colors, which its mixes start from. Without a mix, a name stays as written: the SVG
/// draws CSS's color of that name, as before.
const BASE: [(&str, [f32; 3]); 19] = [
    ("red", [1.0, 0.0, 0.0]),
    ("green", [0.0, 1.0, 0.0]),
    ("blue", [0.0, 0.0, 1.0]),
    ("cyan", [0.0, 1.0, 1.0]),
    ("magenta", [1.0, 0.0, 1.0]),
    ("yellow", [1.0, 1.0, 0.0]),
    ("black", [0.0, 0.0, 0.0]),
    ("white", [1.0, 1.0, 1.0]),
    ("gray", [0.5, 0.5, 0.5]),
    ("darkgray", [0.25, 0.25, 0.25]),
    ("lightgray", [0.75, 0.75, 0.75]),
    ("brown", [0.75, 0.5, 0.25]),
    ("lime", [0.75, 1.0, 0.0]),
    ("olive", [0.5, 0.5, 0.0]),
    ("orange", [1.0, 0.5, 0.0]),
    ("pink", [1.0, 0.75, 0.75]),
    ("purple", [0.75, 0.0, 0.25]),
    ("teal", [0.0, 0.5, 0.5]),
    ("violet", [0.5, 0.0, 0.5]),
];

pub(super) fn hex([r, g, b]: [f32; 3]) -> String {
    // a half rounds up, also when the decimal (0.9 × 255 = 229.5) falls just below it in binary
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 1e-3).round().min(255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b))
}

/// A value in one of xcolor's models (`HTML`, `rgb`, `RGB`, `gray`, `cmyk`); None when the model is
/// another or the value does not fit it. With several models (`rgb/cmyk`), the first one.
pub(super) fn model(model: &str, value: &str) -> Option<[f32; 3]> {
    let model = model.split('/').next()?.trim();
    let value = value.split('/').next()?.trim();
    let numbers = |n: usize, max: f32| -> Option<Vec<f32>> {
        let v: Vec<f32> = value.split(',').map(|x| x.trim().parse::<f32>().ok().filter(|x| (0.0..=max).contains(x))).collect::<Option<_>>()?;
        (v.len() == n).then_some(v)
    };
    match model {
        "HTML" => {
            let digits = (value.len() == 6 && value.chars().all(|c| c.is_ascii_hexdigit())).then_some(value)?;
            let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok().map(|b| f32::from(b) / 255.0);
            Some([byte(0)?, byte(2)?, byte(4)?])
        }
        "rgb" => numbers(3, 1.0).map(|v| [v[0], v[1], v[2]]),
        "RGB" => numbers(3, 255.0).map(|v| [v[0] / 255.0, v[1] / 255.0, v[2] / 255.0]),
        "gray" => numbers(1, 1.0).map(|v| [v[0]; 3]),
        // xcolor's conversion: each of red, green, blue is 1 − min(1, its complement + black)
        "cmyk" => numbers(4, 1.0).map(|v| [0, 1, 2].map(|i| 1.0 - (v[i] + v[3]).min(1.0))),
        _ => None,
    }
}

/// A name the equation defined, or one of xcolor's base colors, with its components.
fn components(name: &str, defined: &Colors) -> Option<[f32; 3]> {
    match defined.get(name) {
        Some(d) => d.rgb,
        None => BASE.iter().find(|(n, _)| *n == name).map(|(_, rgb)| *rgb),
    }
}

/// A plain color name nobody defined (`red`, `RoyalBlue`), kept as written, with xcolor's components
/// when it is a base color.
pub(super) fn name(name: &str) -> Option<Defined> {
    let plain = !name.is_empty() && name.len() < 40 && name.chars().all(|c| c.is_ascii_alphanumeric());
    plain.then(|| Defined { css: name.to_string(), rgb: components(name, &Colors::new()) })
}

/// A color expression: a name, or xcolor's mix `red!40` (40 % red, the rest white) and its chains
/// `red!40!blue`, `red!40!blue!50!green`, each step mixing what came before with the next color.
/// None when it is not one contextEquation can compute: a plain name nobody defined is the caller's to keep.
pub(super) fn expression(expr: &str, defined: &Colors) -> Option<Defined> {
    let parts: Vec<&str> = expr.split('!').map(str::trim).collect();
    if parts.len() == 1 {
        return defined.get(parts[0]).cloned();
    }
    let mut rgb = components(parts[0], defined)?;
    for step in parts[1..].chunks(2) {
        let p = step[0].parse::<f32>().ok().filter(|p| (0.0..=100.0).contains(p))? / 100.0;
        let other = match step.get(1) {
            Some(name) => components(name, defined)?,
            None => [1.0; 3],
        };
        rgb = [0, 1, 2].map(|i| p * rgb[i] + (1.0 - p) * other[i]);
    }
    Some(Defined { css: hex(rgb), rgb: Some(rgb) })
}
