//! InitMainDatMouse 0x36F90: the program start before the main menu. DOS set-up (atexit, mouse driver install, video mode,
//! panel file open) is the window's own start; these are the values that a player can see or hear afterwards.

/// What 0x36F90 leaves behind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Start {
    /// PlayXmiMusic 0x5C4C8: the music file (MISC\INTRO, short name as in music::Music::load).
    pub music: &'static str,
    /// PlayTitleIntro 0x66604, argument 1: the films, then the credits.
    pub title_arg: i32,
    /// SetMouseLimit? 0x682AC: the whole screen.
    pub limit: (i32, i32, i32, i32),
    /// MouseSetPos? 0x681D0 after the title part.
    pub mouse: (i32, i32),
    /// SetMouseShape? 0x674B0: the arrow.
    pub shape: i32,
    /// 0x36FAA: 0x284F0B = 1, the pointer is off through the title part.
    pub pointer_off: bool,
}

pub const START: Start = Start { music: "INTRO", title_arg: 1, limit: (0, 0, 319, 199), mouse: (159, 100), shape: 0, pointer_off: true };

/// 0x36FD6..0x36FF4: SetMusic(0, 0) runs when word 0x95DC0 or byte 0x9639B is 1. The port plays the FM form (both 0), and
/// `music::Music` also treats theme 0 as no change, so the window never takes this road.
pub fn set_music_zero(driver: u16, music_on: u8) -> bool {
    driver == 1 || music_on == 1
}

/// InitSound 0x5CED4 + InitAil 0x5C1DC: what SOUND.CFG (2 x 0x28 driver records + 2 bytes) means to the game. The device
/// word of a record below 1 = no driver: the Sound FX or Music switch goes off and stays refused. The port has its own
/// mixer and FM synth, so a driver file name, I/O port and the other music forms (device 8, 9, 0xB) have no use here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundSetup {
    pub sound_driver: bool,
    pub music_driver: bool,
    pub subtitles: Option<u8>,
}

impl Default for SoundSetup {
    fn default() -> SoundSetup {
        SoundSetup { sound_driver: true, music_driver: true, subtitles: None }
    }
}

impl SoundSetup {
    pub fn from_cfg(cfg: &[u8]) -> SoundSetup {
        let dev = |o: usize| cfg.get(o..o + 2).map_or(1, |b| i16::from_le_bytes([b[0], b[1]]));
        SoundSetup { sound_driver: dev(0x18) >= 1, music_driver: dev(0x40) >= 1, subtitles: cfg.get(80).copied() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn music_zero_only_for_one() {
        assert!(!set_music_zero(0, 0));
        assert!(set_music_zero(1, 0));
        assert!(set_music_zero(0, 1));
        assert!(!set_music_zero(2, 2));
    }
}
