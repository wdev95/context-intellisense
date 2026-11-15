//! Corners of the public API: the composite symbols' table.

use context_equation_core::composite;

#[test]
fn only_the_doubled_ceilings_and_floors_are_doubled() {
    assert_eq!(composite::doubled(composite::LLCEIL), Some('⌈'));
    assert_eq!(composite::doubled(composite::RRFLOOR), Some('⌋'));
    assert_eq!(composite::doubled(composite::VAROCIRCLE), None);
    assert_eq!(composite::doubled('⌈'), None);
}

