//! At most 65,535 fallback fonts: a [`FallbackFont`] is a 16-bit number. In its own test binary, as
//! the fonts loaded stay for as long as the process runs.

use context_equation_core::font::{FallbackFont, FontError};

#[test]
fn the_fallback_font_after_the_65535th_is_refused() {
    let font = std::fs::read(format!("{}/tests/fonts/fallback-a.ttf", env!("CARGO_MANIFEST_DIR"))).unwrap();
    // the same font with a different number after its tables: a different font each time
    let numbered = |n: u16| [font.as_slice(), &n.to_be_bytes()].concat();
    for n in 0..u16::MAX {
        FallbackFont::load(numbered(n)).unwrap();
    }
    assert_eq!(FallbackFont::loaded().len(), usize::from(u16::MAX));
    let e = FallbackFont::load(numbered(u16::MAX)).unwrap_err();
    assert!(matches!(e, FontError::NotAFont(_)), "{e:?}");
    assert_eq!(e.to_string(), "not a font: too many fallback fonts loaded");
    // one already loaded is still given back
    assert_eq!(FallbackFont::load(numbered(7)).ok(), FallbackFont::loaded().get(7).copied());
}
