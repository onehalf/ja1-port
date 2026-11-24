//! Ledger rows ja1front:18160/18164, c2aea50:ja1front:18993: the button hold's press place is packed as
//! x << 16 | (y & 0xFFFF) for frame::hold_still, and -1 (no hold) at the start and after the release.
//! A test of its own (own process): the place is one static.
use ja1front::pointer::{ButtonHold, HOLD_AT};
use std::sync::atomic::Ordering::Relaxed;

#[test]
fn the_hold_place_is_packed_and_starts_unset() {
    assert_eq!(HOLD_AT.load(Relaxed), -1);
    let h = ButtonHold::press((3, 5));
    assert_eq!(HOLD_AT.load(Relaxed), 3 << 16 | 5);
    assert_eq!(h.step(1), None);
    assert_eq!(h.step(0), Some((3, 5)));
    assert_eq!(HOLD_AT.load(Relaxed), -1);
}
