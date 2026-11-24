//! cursor.md "Debug keys (level 4)": the debug level L and the save-anywhere byte. Both live for the whole program
//! run (never saved), as in the original.

use std::sync::atomic::{AtomicU8, Ordering};

static LEVEL: AtomicU8 = AtomicU8::new(0);
static SAVE_ANYWHERE: AtomicU8 = AtomicU8::new(0);
/// tactical_map.md "Drawing order": the roofs flag, 1 at program start; Alt+Q flips it, the quick save keeps it.
static ROOFS: AtomicU8 = AtomicU8::new(1);
/// Tests that change the level take this lock (the level is one static for the whole test binary).
#[cfg(test)]
pub static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Key codes as the original's key handler sees them: a Ctrl letter = its toupper value 1..26, a plain key = its
/// upper-case character, an extended key = 0x100 + its scan code.
pub const CTRL_A: u16 = 1;
pub const CTRL_B: u16 = 2;
pub const CTRL_D: u16 = 4;
pub const CTRL_L: u16 = 12;
pub const CTRL_O: u16 = 15;
pub const CTRL_P: u16 = 16;
pub const CTRL_R: u16 = 18;
pub const CTRL_T: u16 = 20;
pub const ALT_Q: u16 = 0x110;
pub const ALT_E: u16 = 0x112;
pub const ALT_L: u16 = 0x126;
pub const CTRL_F1: u16 = 0x15E;

/// cursor.md "Debug keys (level 4)" 1, Ctrl+R (0x13AED): each of the 8 merc records (512 bytes each in `soldiers`)
/// with +0x182, +0x183 and +0x138 all non-zero and a hand item type t (+0x00) of 1..14 gets the full clip: its count
/// +0x01 = item table (`item_types`, 12 bytes per type) entry t byte 5.
pub fn full_clips(soldiers: &mut [u8], item_types: &[u8]) {
    for r in soldiers.chunks_exact_mut(512).take(8) {
        let ty = r[0] as usize;
        if r[0x182] != 0 && r[0x183] != 0 && r[0x138] != 0 && (1..=14).contains(&ty) {
            r[1] = item_types[ty * 12 + 5];
        }
    }
}

pub fn level() -> u8 {
    LEVEL.load(Ordering::Relaxed)
}

pub fn set_level(l: u8) {
    LEVEL.store(l, Ordering::Relaxed)
}

/// The save-anywhere byte (savegame.md "Quick save keys"): 1 lets Alt+S save with enemies near.
pub fn save_anywhere() -> bool {
    SAVE_ANYWHERE.load(Ordering::Relaxed) != 0
}

/// "Level keys": Ctrl+B, Ctrl+A, Ctrl+L, Ctrl+D step L 0 -> 1 -> 2 -> 3 -> 4; a level key out of order sets 0.
/// Returns true when `code` is a level key.
pub fn level_key(code: u16) -> bool {
    let need = match code {
        CTRL_B => 0,
        CTRL_A => 1,
        CTRL_L => 2,
        CTRL_D => 3,
        _ => return false,
    };
    set_level(if level() == need { need + 1 } else { 0 });
    true
}

/// Ctrl+F1 at L = 4: the save-anywhere byte flips.
pub fn flip_save_anywhere() {
    SAVE_ANYWHERE.fetch_xor(1, Ordering::Relaxed);
}

/// The roofs flag (tactical_map.md "Drawing order"): false = every ground tile and tall object drawn, no roofs.
pub fn roofs() -> u8 {
    ROOFS.load(Ordering::Relaxed)
}

pub fn set_roofs(v: u8) {
    ROOFS.store(v, Ordering::Relaxed)
}

/// Alt+Q at L = 4: the roofs flag flips; returns the new value.
pub fn flip_roofs() -> u8 {
    ROOFS.fetch_xor(1, Ordering::Relaxed) ^ 1
}

/// The script / key name of a code (`key Ctrl+B`, `key Alt+Q`, `key E`, `key @`).
pub fn parse(name: &str) -> Option<u16> {
    if let Some(c) = name.strip_prefix("Ctrl+") {
        if c == "F1" {
            return Some(CTRL_F1);
        }
        let b = c.as_bytes();
        return (b.len() == 1 && b[0].is_ascii_alphabetic()).then(|| (b[0].to_ascii_uppercase() - b'A' + 1) as u16);
    }
    if let Some(c) = name.strip_prefix("Alt+") {
        return match c {
            "Q" | "q" => Some(ALT_Q),
            "E" | "e" => Some(ALT_E),
            "L" | "l" => Some(ALT_L),
            _ => None,
        };
    }
    match name {
        "E" | "e" | "F" | "f" | "G" | "g" | "K" | "k" | "@" => Some(name.as_bytes()[0].to_ascii_uppercase() as u16),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_and_reset() {
        let _g = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_level(0);
        for (k, l) in [(CTRL_B, 1), (CTRL_A, 2), (CTRL_L, 3), (CTRL_D, 4), (CTRL_D, 0), (CTRL_B, 1), (CTRL_L, 0)] {
            assert!(level_key(k));
            assert_eq!(level(), l);
        }
        assert!(!level_key(CTRL_R));
        assert_eq!(parse("Ctrl+b"), Some(CTRL_B));
        assert_eq!(parse("Alt+L"), Some(ALT_L));
        assert_eq!(parse("@"), Some(0x40));
        set_level(0);
    }
}
