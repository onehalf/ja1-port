//! PlayInvExtra? 0x25AD8: an inventory remark (inventory.md "inv_extra"): the sound starts, with Subtitles on the text shows in
//! the message box for 1000 ms (ShowMessage? 0x27B98 with KeyOrClick 0x67E44), then the game waits while the sound plays until
//! a mouse button is down. The inventory screen and the diff test (ja1f_inv_extra) are hosts; invextra_diff.py compares.

/// SpeechStream 0x5CC6C: rate 0x5622 and volume 0x46.
pub const RATE: i32 = 0x5622;
pub const VOLUME: i32 = 70;
/// The time limit of the box (the 7th argument of ShowMessage? in 0x25AD8).
pub const BOX_MS: i32 = 1000;

/// One poll: the level of the buttons (1 left, 2 right) and the keys that came.
pub struct Poll {
    pub buttons: u8,
    pub keys: Vec<u8>,
}

pub trait Host {
    /// OpenArchiveForSpeech? 0x5C0EC + ArcSeek 0x534A4 + SpeechStream 0x5CC6C.
    fn start(&mut self, member: &str, rate: i32, volume: i32);
    /// WrapText? 0x362AC in ShowMessage?; the box time starts here (TimerStart 0x47020).
    fn show_box(&mut self, text: &str) -> Result<(), i32>;
    /// RestoreScreenArea 0x468B0 at the end of ShowMessage?.
    fn box_end(&mut self);
    /// MousePoll? 0x678E8 (`mode`: 0 in the box, 1 in the sound wait). None = the input stopped.
    fn poll(&mut self, mode: i32) -> Option<Poll>;
    /// TimerElapsedMs 0x47040 since the box showed.
    fn elapsed(&mut self) -> i32;
    /// Word 0x96372: the speech channel is busy.
    fn speaking(&mut self) -> bool;
    /// SpeechRefill? 0x5CCF8.
    fn refill(&mut self);
    /// A key that nothing read here: the original leaves it in the keyboard queue.
    fn queue(&mut self, key: u8);
}

// The button start flags 0x284F1A (bit 0) and 0x284F1C (bit 1) as the last KeyOrClick left them. They are globals in the
// original: ShowMessage? sets them from the button level at its start, KeyOrClick clears one when it reads its button
// up, and WaitKeyOrClick 0x67F34 starts with them as they are (a box that a key ended while a button was down leaves 1).
thread_local! {
    static LATCH: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

/// See `LATCH`.
pub fn latch() -> u8 {
    LATCH.with(|l| l.get())
}

/// Sets `LATCH` (the window and the tests run on one thread each).
pub fn set_latch(v: u8) {
    LATCH.with(|l| l.set(v & 3))
}

/// KeyOrClick 0x67E44 with the start flags ShowMessage? sets from the button level at its start (0x284F1A left, 0x284F1C right):
/// a key ends the wait; a button that was not down at the start ends it when it comes up, right button first; a button that
/// was down at the start counts only after it came up. The original polls in its own loop until the release: here `step` is
/// called for each poll and answers false until then.
pub struct KeyOrClick {
    left_held: bool,
    right_held: bool,
    release: u8,
}

impl KeyOrClick {
    pub fn new(start: u8) -> Self {
        KeyOrClick { left_held: start & 1 != 0, right_held: start & 2 != 0, release: 0 }
    }

    /// True while the wait is for a release (the original reads no key and no timer then).
    pub fn in_release(&self) -> bool {
        self.release != 0
    }

    pub fn step(&mut self, buttons: u8, key: bool) -> bool {
        let r = self.poll(buttons, key);
        set_latch(self.left_held as u8 | (self.right_held as u8) << 1);
        r
    }

    fn poll(&mut self, buttons: u8, key: bool) -> bool {
        if self.release != 0 {
            return buttons & self.release == 0;
        }
        if key {
            return true;
        }
        if buttons & 2 != 0 {
            if !self.right_held {
                self.release = 2;
                return false;
            }
        } else {
            self.right_held = false;
        }
        if buttons & 1 != 0 {
            if !self.left_held {
                self.release = 1;
            }
        } else {
            self.left_held = false;
        }
        false
    }
}

/// WaitMs? 0x67F48 (ms): TimerElapsedMs first, then a poll and KeyOrClick 0x67E44 until `ms` passed or a key or a click
/// came. A key or a click ends only this wait; the answer is true only when the key byte 0x95DC3 holds Escape (the
/// byte is cleared then). A click reads the old byte too. No timer read while KeyOrClick waits for a release.
pub struct WaitMs {
    ms: i32,
    wait: KeyOrClick,
    /// Byte 0x95DC3.
    pub last_key: u8,
}

impl WaitMs {
    pub fn new(ms: i32, start: u8, last_key: u8) -> Self {
        WaitMs { ms, wait: KeyOrClick::new(start), last_key }
    }

    /// WaitMs? 0x67F48 as its callers run it: the start flags 0x284F1A / 0x284F1C stay as the last KeyOrClick left them
    /// (WaitMs sets no flag before its KeyOrClick loop).
    pub fn from_latch(ms: i32, last_key: u8) -> Self {
        WaitMs::new(ms, latch(), last_key)
    }

    /// True while KeyOrClick waits for a release: no key is read then.
    pub fn in_release(&self) -> bool {
        self.wait.in_release()
    }

    /// The button start flags 0x284F1A (bit 0), 0x284F1C (bit 1).
    pub fn flags(&self) -> u8 {
        self.wait.left_held as u8 | (self.wait.right_held as u8) << 1
    }

    /// One poll. `key`: the key read, only when not `in_release`. Some(escape) = the wait ended.
    pub fn step(&mut self, elapsed: i32, buttons: u8, key: Option<u8>) -> Option<bool> {
        if !self.wait.in_release() && self.ms <= elapsed {
            return Some(false);
        }
        if let Some(k) = key {
            self.last_key = k;
        }
        if !self.wait.step(buttons, key.is_some()) {
            return None;
        }
        if self.last_key == 0x1B {
            self.last_key = 0;
            return Some(true);
        }
        Some(false)
    }
}

/// KeyOrClick? 0x67E44 key read: GetKey 0x560F4, and a second GetKey when the first byte is 0 (an extended key,
/// 0 + scan code). The byte that 0x95DC3 keeps; None when no key waits.
pub fn read_key(q: &mut std::collections::VecDeque<u8>) -> Option<u8> {
    q.pop_front().map(|k| if k == 0 { q.pop_front().unwrap_or(0) } else { k })
}

/// WaitKeyOrClick 0x67F34: MousePoll, then KeyOrClick 0x67E44, with no time limit. The popups of key Z and key S and the
/// debug pages wait here. The start flags stay as the last box left them (the window passes `latch()`).
pub struct WaitKey {
    wait: KeyOrClick,
    /// Byte 0x95DC3.
    pub last_key: u8,
}

impl WaitKey {
    pub fn new(start: u8, last_key: u8) -> Self {
        WaitKey { wait: KeyOrClick::new(start), last_key }
    }

    /// True while KeyOrClick waits for a release: no key is read then.
    pub fn in_release(&self) -> bool {
        self.wait.in_release()
    }

    /// The button start flags 0x284F1A (bit 0), 0x284F1C (bit 1).
    pub fn flags(&self) -> u8 {
        self.wait.left_held as u8 | (self.wait.right_held as u8) << 1
    }

    /// One poll. `key`: the key read, only when not `in_release`. True = the wait ended.
    pub fn step(&mut self, buttons: u8, key: Option<u8>) -> bool {
        if let Some(k) = key {
            self.last_key = k;
        }
        self.wait.step(buttons, key.is_some())
    }
}

/// EscPressed? 0x67300: the next waiting key is read (an extended key takes its two bytes, 0 + scan code); true only
/// when that key is Escape. No key waiting: false.
pub fn esc_pressed(queue: &mut std::collections::VecDeque<u8>) -> bool {
    match queue.pop_front() {
        None => false,
        Some(0) => {
            queue.pop_front();
            false
        }
        Some(k) => k == 0x1B,
    }
}

/// The remark. `start` = the button level before the call. Ok(false) = the input stopped.
pub fn run(h: &mut dyn Host, member: &str, text: &str, subtitles: bool, start: u8) -> Result<bool, i32> {
    if !member.is_empty() {
        h.start(member, RATE, VOLUME);
    }
    if !text.is_empty() && subtitles {
        let mut wait = KeyOrClick::new(start);
        h.show_box(text)?;
        loop {
            let Some(p) = h.poll(0) else { return Ok(false) };
            let mut keys = p.keys.into_iter();
            let done = if wait.in_release() {
                wait.step(p.buttons, false)
            } else {
                let key = keys.next().is_some();
                wait.step(p.buttons, key)
            };
            keys.for_each(|k| h.queue(k));
            if done || (!wait.in_release() && h.elapsed() >= BOX_MS) {
                break;
            }
        }
        h.box_end();
    }
    while h.speaking() {
        let Some(p) = h.poll(1) else { return Ok(false) };
        h.refill();
        p.keys.into_iter().for_each(|k| h.queue(k));
        if p.buttons & 3 != 0 {
            break;
        }
    }
    Ok(true)
}

#[cfg(test)]
mod latch_tests {
    use super::*;

    /// Ledger row c2aea50:ja1front:1977: an extended key (0, scan code) leaves its scan code; a plain key its code.
    #[test]
    fn an_extended_key_reads_its_scan_code() {
        let mut q: std::collections::VecDeque<u8> = [0, 0x48, b'a'].into_iter().collect();
        assert_eq!(read_key(&mut q), Some(0x48));
        assert_eq!(read_key(&mut q), Some(b'a'));
        assert_eq!(read_key(&mut q), None);
    }

    /// A box that a key ends while the left button is down leaves flag 0x284F1A at 1: the next WaitKeyOrClick that starts
    /// with the button down ignores that press, and only a new press after the release ends it.
    #[test]
    fn a_key_end_under_a_press_keeps_the_latch_for_the_next_popup() {
        let mut b = KeyOrClick::new(1);
        assert!(b.step(1, true));
        assert_eq!(latch(), 1);
        let mut w = WaitKey::new(latch(), 0);
        assert!(!w.step(1, None));
        assert!(!w.step(0, None));
        assert_eq!(latch(), 0);
        assert!(!w.step(1, None));
        assert!(w.step(0, None));
        // A box whose button came up before its end leaves 0: the held press then ends the popup at its release.
        let mut b = KeyOrClick::new(1);
        assert!(!b.step(0, false));
        assert!(b.step(0, true));
        let mut w = WaitKey::new(latch(), 0);
        assert!(!w.step(1, None));
        assert!(w.step(0, None));
    }

    /// WaitMs? 0x67F48 also starts with the flags as they are: after a box that a key ended under a held left button,
    /// The right button has its own start flag (0x284F1C, bit 1 of the latch): a key end under a held right button
    /// leaves it set, so the next wait ignores that press until its release, as for the left button (0x284F1A).
    #[test]
    fn a_key_end_under_a_right_press_keeps_the_right_latch() {
        let mut b = KeyOrClick::new(2);
        assert!(b.step(2, true));
        assert_eq!(latch(), 2);
        let mut w = WaitKey::new(latch(), 0);
        assert!(!w.step(2, None));
        assert!(!w.step(0, None));
        assert_eq!(latch(), 0);
        assert!(!w.step(2, None));
        assert!(w.step(0, None));
    }

    /// the release of that press does not end the timed wait; a new click does.
    #[test]
    fn a_timed_wait_starts_from_the_latch() {
        let mut b = KeyOrClick::new(1);
        assert!(b.step(1, true));
        let mut w = WaitMs::from_latch(1000, 0);
        assert_eq!(w.step(0, 1, None), None);
        assert_eq!(w.step(10, 0, None), None);
        assert_eq!(latch(), 0);
        assert_eq!(w.step(20, 1, None), None);
        assert_eq!(w.step(30, 0, None), Some(false));
        // With flag 0 the held press ends the wait at its release.
        set_latch(0);
        let mut w = WaitMs::from_latch(1000, 0);
        assert_eq!(w.step(0, 1, None), None);
        assert_eq!(w.step(10, 0, None), Some(false));
    }
}
