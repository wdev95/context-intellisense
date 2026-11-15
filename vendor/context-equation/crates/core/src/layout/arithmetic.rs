//! Column arithmetic as taught in Brazilian schools, computed from the numbers:
//! long division with the "chave" layout (L-CONTA-1) and column addition, subtraction and multiplication (L-CONTA-2).
//! Digits sit on a grid of equal-width columns, aligned by place value.

use crate::Error;

use super::{em, glyph, Item, MathBox, Style};

type R<T> = Result<T, Error>;

struct Grid {
    column: f32,
    step: f32,
    row_height: f32,
    math_box: MathBox,
    thickness: f32,
}

impl Grid {
    fn new(e: Style) -> R<Grid> {
        let zero = glyph('0', e)?;
        Ok(Grid { column: zero.width, step: em(e, 1.25), row_height: zero.height, math_box: MathBox::empty(), thickness: em(e, 0.05) })
    }

    /// Writes `text` ending at column `end` (exclusive), on row `row` (0 = top).
    fn write_digits(&mut self, text: &str, end: i32, row: i32, e: Style) -> R<()> {
        let n = text.chars().count() as i32;
        for (i, c) in text.chars().enumerate() {
            let g = glyph(c, e)?;
            let col = end - n + i as i32;
            let x = col as f32 * self.column + (self.column - g.width) / 2.0;
            self.math_box.place(&g, x, -(row as f32) * self.step);
        }
        Ok(())
    }

    fn sign(&mut self, c: char, col: i32, row: i32, e: Style) -> R<()> {
        let g = glyph(c, e)?;
        let x = col as f32 * self.column + (self.column - g.width) / 2.0;
        self.math_box.place(&g, x, -(row as f32) * self.step);
        Ok(())
    }

    /// Horizontal rule between row `row` and the next, from column `from` to `to` (exclusive).
    fn rule(&mut self, from: i32, to: i32, row: i32) {
        let y = -(row as f32) * self.step - (self.step - self.row_height) * 0.42;
        self.math_box.items.push(Item::Rule { x: from as f32 * self.column, y, width: (to - from) as f32 * self.column, height: self.thickness, color: None });
    }

    fn finish(mut self, columns: i32, rows: i32, e: Style) -> MathBox {
        self.math_box.width = columns as f32 * self.column;
        // center the calculation on the math axis, by the bounds of what was drawn
        let top = self.math_box.height.max(self.row_height) + em(e, 0.1);
        let bottom = self.math_box.depth.max((rows - 1) as f32 * self.step) + em(e, 0.1);
        let axis = super::k(e, |c| c.axis_height());
        let offset = axis + (bottom - top) / 2.0;
        self.math_box.height = top;
        self.math_box.depth = bottom;
        self.math_box.shift_vertical(offset);
        self.math_box.single_glyph = false;
        self.math_box
    }
}

/// One step of the long division: where it ends, what is subtracted and what is left.
struct Step {
    end: i32,
    product: u128,
    remainder: u128,
}

pub fn long_division(dividend: &str, divisor: &str, e: Style) -> R<MathBox> {
    let error = || Error::Syntax { column: 1, message: "number too large for column arithmetic".into() };
    let dv: u128 = divisor.parse().map_err(|_| error())?;
    if dv == 0 {
        return Err(Error::Syntax { column: 1, message: "division by zero in column arithmetic".into() });
    }
    let digits: Vec<u32> = dividend.chars().map(|c| c.to_digit(10).unwrap_or(0)).collect();

    let mut steps = Vec::new();
    let mut current: u128 = 0;
    let mut quotient = String::new();
    let mut started = false;
    for (i, d) in digits.iter().enumerate() {
        current = current.checked_mul(10).and_then(|v| v.checked_add(u128::from(*d))).ok_or_else(error)?;
        if !started && current < dv && i + 1 < digits.len() {
            continue;
        }
        started = true;
        let q = current / dv;
        quotient.push(char::from_digit(q as u32, 10).unwrap_or('?'));
        let product = q * dv;
        steps.push(Step { end: i as i32 + 1, product, remainder: current - product });
        current -= product;
    }

    let n = digits.len() as i32;
    let mut g = Grid::new(e)?;
    // row 0: dividend (columns 1..=n; column 0 is left for the minus sign) | divisor
    g.write_digits(dividend, n + 1, 0, e)?;
    let x_brace = n + 1;
    let right_width = (divisor.len().max(quotient.len()) as i32) + 1;
    g.write_digits(divisor, x_brace + 1 + divisor.len() as i32, 0, e)?;
    g.write_digits(&quotient, x_brace + 1 + quotient.len() as i32, 1, e)?;

    // steps below the dividend: −product, rule, remainder (with the next digit brought down)
    let mut row = 1;
    let last_row = steps.len().saturating_sub(1);
    for (idx, step) in steps.iter().enumerate() {
        if step.product == 0 && idx != last_row {
            continue;
        }
        let product = step.product.to_string();
        let width = product.len() as i32;
        g.write_digits(&product, step.end + 1, row, e)?;
        g.sign('−', step.end - width, row, e)?;
        g.rule(step.end - width, step.end + 1, row);
        row += 1;
        let mut remainder = step.remainder.to_string();
        let mut end = step.end + 1;
        if idx < last_row {
            // bring the next digit down beside the remainder
            let next_char = digits[step.end as usize];
            if step.remainder == 0 {
                remainder = String::new();
            }
            remainder.push(char::from_digit(next_char, 10).unwrap_or('?'));
            end += 1;
        }
        g.write_digits(&remainder, end, row, e)?;
        row += 1;
    }

    // the bracket: vertical rule between dividend and divisor, horizontal rule under the divisor
    let top = g.row_height + em(e, 0.15);
    let x = x_brace as f32 * g.column + g.column * 0.35;
    let bottom = -(g.step - g.row_height) * 0.42 - g.thickness;
    g.math_box.items.push(Item::Rule { x, y: bottom, width: g.thickness, height: top - bottom, color: None });
    g.math_box.items.push(Item::Rule { x, y: bottom, width: (right_width as f32 + 0.15) * g.column, height: g.thickness, color: None });
    Ok(g.finish(x_brace + right_width + 1, row.max(2), e))
}

pub fn column_arithmetic(operation: char, terms: &[String], e: Style) -> R<MathBox> {
    let error = || Error::Syntax { column: 1, message: "number too large for column arithmetic".into() };
    let [a, b] = terms else {
        return Err(Error::Syntax { column: 1, message: "column arithmetic takes two terms".into() });
    };
    let a: u128 = a.parse().map_err(|_| error())?;
    let b: u128 = b.parse().map_err(|_| error())?;
    let (result, negative) = match operation {
        '+' => (a.checked_add(b).ok_or_else(error)?, false),
        '−' => {
            if a >= b {
                (a - b, false)
            } else {
                (b - a, true)
            }
        }
        _ => (a.checked_mul(b).ok_or_else(error)?, false),
    };
    let ta = a.to_string();
    let tb = b.to_string();
    let tr = format!("{}{}", if negative { "−" } else { "" }, result);

    // partial products when multiplying by a number with more than one digit
    let partials: Vec<String> =
        if operation == '×' && tb.len() > 1 { tb.chars().rev().map(|d| (a * u128::from(d.to_digit(10).unwrap_or(0))).to_string()).collect() } else { Vec::new() };
    let width = [ta.len(), tb.len() + 1, tr.chars().count(), partials.iter().enumerate().map(|(i, p)| p.len() + i).max().unwrap_or(0) + 1].into_iter().max().unwrap_or(1) as i32;

    let mut g = Grid::new(e)?;
    let mut row = 0;

    // carries of the addition: small digits above the first addend
    if operation == '+' {
        let es = e.sup();
        let mut carry = 0u32;
        let digits_a = ta.chars().rev().map(|c| c.to_digit(10).unwrap_or(0)).collect::<Vec<_>>();
        let db = tb.chars().rev().map(|c| c.to_digit(10).unwrap_or(0)).collect::<Vec<_>>();
        for i in 0..digits_a.len().max(db.len()) {
            let digit_sum = digits_a.get(i).copied().unwrap_or(0) + db.get(i).copied().unwrap_or(0) + carry;
            carry = digit_sum / 10;
            if carry > 0 && i + 1 < digits_a.len().max(db.len()) {
                // the column of the next digit, counting from the right
                let col = width - i as i32 - 1;
                let digit = glyph(char::from_digit(carry, 10).unwrap_or('1'), es)?;
                let x = col as f32 * g.column + (g.column - digit.width) / 2.0;
                g.math_box.place(&digit, x, g.row_height + em(e, 0.18));
            }
        }
    }

    g.write_digits(&ta, width + 1, row, e)?;
    row += 1;
    g.write_digits(&tb, width + 1, row, e)?;
    g.sign(operation, 0, row, e)?;
    g.rule(0, width + 1, row);
    row += 1;
    if !partials.is_empty() {
        for (i, p) in partials.iter().enumerate() {
            g.write_digits(p, width + 1 - i as i32, row, e)?;
            if i == partials.len() - 1 {
                g.sign('+', 0, row, e)?;
                g.rule(0, width + 1, row);
            }
            row += 1;
        }
    }
    g.write_digits(&tr, width + 1, row, e)?;
    row += 1;
    Ok(g.finish(width + 1, row, e))
}
