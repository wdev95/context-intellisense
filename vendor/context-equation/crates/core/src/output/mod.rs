pub mod boxes;
pub mod mathml;
pub mod pen;
pub mod speech;
pub mod svg;
pub mod text;

use crate::locale::Locale;

/// A number as written in the locale, with the thousands separator the author used (L-NUM-2, L-NUM-3).
pub fn written_number(value: &str, thousands: Option<char>, loc: Locale) -> String {
    let (integer_part, fraction) = value.split_once('.').map_or((value, None), |(a, b)| (a, Some(b)));
    let mut s = String::new();
    let n = integer_part.chars().count();
    for (i, c) in integer_part.chars().enumerate() {
        if let Some(m) = thousands {
            if i > 0 && (n - i) % 3 == 0 {
                s.push(m);
            }
        }
        s.push(c);
    }
    if let Some(f) = fraction {
        s.push(loc.decimal_separator());
        s.push_str(f);
    }
    s
}
