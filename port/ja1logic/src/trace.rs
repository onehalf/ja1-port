//! A record for the fight event log (port/ja1front/src/events.rs, docs/events.md): the interrupt contests that a soldier
//! won. A won contest and its queue pop can both fall between two polls of the log, so the log cannot see them in the
//! world. Off unless the log turns it on. It holds no game state: the game never reads it.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static ON: AtomicBool = AtomicBool::new(false);
static GRANTS: Mutex<Vec<(u8, u8)>> = Mutex::new(Vec::new());

pub fn set_on(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

/// A soldier won an interrupt contest while `selected` was the selected soldier.
pub(crate) fn grant(winner: u8, selected: u8) {
    if ON.load(Ordering::Relaxed) {
        if let Ok(mut g) = GRANTS.lock() {
            g.push((winner, selected));
        }
    }
}

/// The contests won since the last call, oldest first.
pub fn take_grants() -> Vec<(u8, u8)> {
    if !ON.load(Ordering::Relaxed) {
        return Vec::new();
    }
    GRANTS.lock().map(|mut g| std::mem::take(&mut *g)).unwrap_or_default()
}
