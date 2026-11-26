//! Sound effects, cries and merc quotes (docs/spec/sound.md): the effect bank, 10 channels and the mixer.
//! The window drains the mix into an SDL audio queue; a play script prints `sound` lines and JA1_WAV writes the mix.
use crate::panel::archive_member;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Output rate (samples per second, mono, 16 bits).
pub const RATE: u32 = 22050;
/// The most sound (bytes of mono i16, 150 ms) that may wait in the window's audio queue before a new part.
pub const QUEUE_MAX: u32 = RATE * 2 * 15 / 100;

/// The window's audio queue holds `queued` bytes and a new part comes: true = clear the queue first. After a stall
/// (a slow frame, a save, a busy machine) the old part is late; it goes, so the new sounds start at once and the
/// delay does not stay (#20 H: with "drop the new part above 1 s" the queue stayed near 1 s for good).
pub fn queue_stale(queued: u32) -> bool {
    queued > QUEUE_MAX
}
/// The AIL digital master volume (DIG driver +0x68, 0..127): 127 from the driver install on; only the volume keys
/// set it (0x5CE94 -> AIL_set_digital_master_volume). Every Sound shares it, like the one driver of the original.
static DIG_MASTER: AtomicI32 = AtomicI32::new(127);

/// The volume keys' AIL_set_digital_master_volume (0x81150): the master volume of every sample from now on.
pub fn set_digital_master(v: i32) {
    DIG_MASTER.store(v, Ordering::Relaxed);
}

pub fn digital_master() -> i32 {
    DIG_MASTER.load(Ordering::Relaxed)
}

/// The output volume of a sample of volume `vol` (0x7F990): vol * master / 127, held to 0..127.
pub fn scaled_volume(vol: i32, master: i32) -> i32 {
    (vol * master / 127).clamp(0, 127)
}

/// The program's volume numbers (0x963A0 music, 0x963A8 sound, 0..100). InitAil starts them at 100 (FM driver; 80 for
/// device type 9) and 70; the AIL master volumes stay at 127 until a volume key sends them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Volumes {
    pub music: i32,
    pub sound: i32,
}

static VOLUMES: Mutex<Volumes> = Mutex::new(Volumes { music: 100, sound: 70 });

pub fn volumes() -> Volumes {
    *VOLUMES.lock().unwrap_or_else(|e| e.into_inner())
}

/// The loudness cap of a volume key: 100 in 1.13b; 127 under port fix P17 (#2 row 40), the AIL master range.
fn volume_cap() -> i32 {
    if ja1logic::fanfix::port_on(17) { 127 } else { 100 }
}

/// InitAil 0x5C25E / 0x5C351 sets the numbers to 100 / 70 but never sends them, so both AIL masters stay at 127 and
/// the first louder key makes the game quieter (#2 row 40). Port fix P17 (#71) starts the numbers at 127, the
/// loudness the game starts with (the first louder press changes nothing). Call it once after the fix level is set.
pub fn start_volumes() {
    if ja1logic::fanfix::port_on(17) {
        set_volumes(Volumes { music: 127, sound: 127 });
    }
}

/// Quick Restore reads both numbers back (QuickRestore 0x2D688 / 0x2D6BC) without sending them to AIL.
pub fn set_volumes(v: Volumes) {
    *VOLUMES.lock().unwrap_or_else(|e| e.into_inner()) = v;
}

/// The part of a battle sound that plays: two stream buffers (32768 bytes) in 1.13b (#2 row 56); port fix P13 (#71)
/// plays all of it (a new round's sound still stops the old one, as in the original).
fn battle_part(d: &[u8]) -> &[u8] {
    if ja1logic::fanfix::port_on(13) {
        d
    } else {
        &d[..d.len().min(2 * STREAM_BUFFER)]
    }
}

/// Unit tests that change the shared port fix mask take this lock first.
#[cfg(test)]
pub(crate) static TEST_FIX_MASK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Quick Restore's volume numbers (QuickRestore 0x2D688 / 0x2D6BC). 1.13b reads them back without sending them to AIL,
/// so the old loudness stays until a volume key (#2 row 41); port fix P09 sends both as the master volumes, as a volume
/// key does (0x5CEB4 XMIDI, 0x5CE94 digital).
pub fn restore_volumes(v: Volumes) {
    set_volumes(v);
    if ja1logic::fanfix::port_on(9) {
        crate::music::with(|mu| mu.set_master(v.music));
        set_digital_master(v.sound);
    }
}

/// What a volume key sends to AIL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MasterSet {
    Music(i32),
    Digital(i32),
}

/// TacticalKeyHandler 0x13490, the volume keys (jump table 0x13328, index = key - 1): `+` 0x13D0A music + 1 (at
/// most 100), `-` 0x13CE8 music - 1 (at least 0), `.` 0x13D2E sound + 1, `,` 0x13D54 sound - 1. Each then sends its
/// number as the master volume (0x5CEB4 XMIDI, 0x5CE94 digital). Other keys: None, nothing changes.
pub fn volume_key(key: u8, v: &mut Volumes) -> Option<MasterSet> {
    match key {
        b'+' => {
            v.music = v.music.wrapping_add(1).min(volume_cap());
            Some(MasterSet::Music(v.music))
        }
        b'-' => {
            v.music = v.music.wrapping_sub(1).max(0);
            Some(MasterSet::Music(v.music))
        }
        b'.' => {
            v.sound = v.sound.wrapping_add(1).min(volume_cap());
            Some(MasterSet::Digital(v.sound))
        }
        b',' => {
            v.sound = v.sound.wrapping_sub(1).max(0);
            Some(MasterSet::Digital(v.sound))
        }
        _ => None,
    }
}

/// A volume key in the game: the numbers change and AIL gets the master volume (music and samples at once).
pub fn press_volume_key(key: u8) -> Option<MasterSet> {
    let mut v = volumes();
    let m = volume_key(key, &mut v)?;
    set_volumes(v);
    match m {
        MasterSet::Music(x) => crate::music::with(|mu| mu.set_master(x)),
        MasterSet::Digital(x) => set_digital_master(x),
    }
    Some(m)
}

const EFFECTS: usize = 128;
const OVERHEAD: [&str; 45] = [
    "IN", "OUT", "1", "2", "3", "4", "8", "9", "10", "11", "12", "15", "EXP", "FAL", "R_L", "P_L", "ROK", "WAT",
    "KNE", "GRD", "R0", "R1", "R2", "SIL", "OPN", "WAD", "S_L", "S_R", "TUR", "FIR", "D_1", "D_2", "IMP", "BUB",
    "CRT", "SWI", "B0", "B1", "B2", "B3", "F1", "F2", "STU", "SNA", "TCK",
];
const MAINSND: [&str; 27] = [
    "IN", "OUT", "SWI", "LEV", "ROT", "SCR", "CR1", "CR2", "CVR", "LAP", "ME1", "ME2", "PA1", "PA2", "SL1", "SL2", "HEL",
    "NAT", "INS", "B_1", "B_2", "B_3", "F2", "IDL", "LIF", "P_L", "R_L",
];
/// The name of main bank effect `fx` (sound.md "Main bank").
pub fn main_name(fx: i32) -> &'static str {
    MAINSND.get(fx as usize).copied().unwrap_or("?")
}

/// sound.md "Merc quotes": the volume of every quote voice (SpeechWait streams it at 0x58).
pub const QUOTE_VOL: i32 = 88;
/// The volume of the name sample between the two halves of a quote (SpeechWait? 0x282C0 / 0x282F8, PlayVocSample
/// rate 0x5622, volume 0x68).
const NAME_VOL: i32 = 0x68;

/// sound.md "Requests", op 254: the sunset sound (night insects), its rate and volume.
const SUNSET_FX: i32 = 0x12;
const DOORS: [&str; 8] = ["KEY", "L_0", "PUL", "NOB", "K_I", "LOK", "P_I", "PCK"];
const CRIES: [&str; 5] = ["BAD", "GOOD", "EMPTY", "NOTHING", "HMM"];
/// sound.md "Requests", op 210: the cue start time per film frame (ms).
const CUE_FRAME_MS: u32 = 71;

/// A named sample.
#[derive(Clone)]
pub struct Sample {
    pub name: String,
    pub data: Arc<Vec<u8>>,
}

#[derive(Clone)]
/// The channel of the last button sound. It starts as channel 0, not as none: the original's word 0x337758 is 0 until the
/// first button sound, so that sound stops what plays on channel 0 (sfx_diff).
struct LastButton(Option<usize>);

impl Default for LastButton {
    fn default() -> LastButton {
        LastButton(Some(0))
    }
}

struct Chan {
    s: Sample,
    /// Position in bytes, 16.16 fixed point.
    pos: u64,
    step: u64,
    vol: i32,
    /// Plays left (0 = until stopped).
    loops: i32,
    /// `speech_stall`: the channel stays silent at this position (16.16) and stays busy.
    stall_at: Option<u64>,
}

impl Chan {
    fn new(s: Sample, rate: i32, vol: i32, loops: i32) -> Chan {
        let step = ((rate.max(0) as u64) << 16) / RATE as u64;
        Chan { s, pos: 0, step, vol: vol.clamp(0, 127), loops, stall_at: None }
    }
    /// The next output value; None when the channel ended.
    fn next(&mut self) -> Option<i32> {
        let len = self.s.data.len() as u64;
        if len == 0 || self.step == 0 {
            return None;
        }
        if self.stall_at.is_some_and(|at| self.pos >= at) {
            return Some(0);
        }
        if self.pos >> 16 >= len {
            if self.loops == 1 {
                return None;
            }
            if self.loops > 1 {
                self.loops -= 1;
            }
            self.pos = 0;
        }
        let b = self.s.data[(self.pos >> 16) as usize] as i32;
        self.pos += self.step;
        Some((b - 128) * scaled_volume(self.vol, digital_master()) * 256 / 127)
    }
}

/// One buffer of the voice stream (SpeechRefill? 0x5CCF8).
const STREAM_BUFFER: usize = 0x4000;
/// The time of that buffer at 22050 Hz: the most of a voice that a box lets through.
pub const STREAM_BUFFER_TIME: std::time::Duration = std::time::Duration::from_millis(STREAM_BUFFER as u64 * 1000 / RATE as u64);

/// OpenInvExtra 0x25CA4: the archive of the inventory remarks.
pub const INV_EXTRA: (&str, &str) = ("INV", "EXTRA");

#[derive(Default)]
pub struct Sound {
    /// The game's hard-disk folder (beside JA.EXE) and the CD image; None: no sound.
    dir: Option<PathBuf>,
    iso: Option<String>,
    exe: Option<String>,
    effects: Vec<Option<Sample>>,
    chans: [Option<Chan>; 8],
    cry: Option<Sample>,
    cry_chan: Option<Chan>,
    speech: Option<Chan>,
    /// What the lip clock reads of the voice on the speech channel, when it holds a cry sample part (lip_bytes).
    lip_view: Option<Arc<Vec<u8>>>,
    /// Op 210 cues: (output sample index when due, sample).
    cues: Vec<(u64, Sample)>,
    /// Op 210: the play time (ms) of the try film (phase 0); the outcome film (phase 1) and its cues start after it
    /// (PlayFilm 0x5BFF8 returns when the film ends).
    door_ms: u64,
    /// Op 254: the channel of the sunset sound (op 258 stops it).
    sunset: Option<usize>,
    /// sound.md "Button sounds": the channel of the last button sound.
    button: LastButton,
    /// The Control Panel switch "Sound FX" is off: no effect starts (panel.md "The Control Panel").
    pub fx_off: bool,
    /// Microseconds of screen time not yet mixed (less than one 10 ms tick).
    pub(crate) rest_us: u64,
    /// The tactical bank must be loaded again before the next pass (a new sector entry).
    pub bank_stale: bool,
    /// Names of the samples started since the last `sound` script line.
    pub started: Vec<String>,
    /// Output samples made so far, and ticks mixed.
    made: u64,
    passes: u64,
    /// Word 0x337940: the size that the last ArcSeek of a voice or effect member found. A missing member keeps it (name_voice).
    last_size: std::cell::Cell<usize>,
    /// The window's audio queue takes these (only kept when `live`).
    pub live: bool,
    pub pending: Vec<i16>,
    /// JA1_WAV: the whole mix.
    pub wav: Option<Vec<i16>>,
}

/// One sample of the tactical bank: `member`.8 of archive `folder`\\`archive` goes into effect `slot`.
pub struct BankLoad {
    pub folder: &'static str,
    pub archive: String,
    pub member: String,
    pub slot: usize,
}

/// sound.md "Tactical bank", in the original's order. LoadOverheadSounds 0x31B44 with 45 samples: OVERHEAD into 0..44,
/// then 0x31AF8 with EAX 1: a team soldier (first 8 records) with +0x182 and either +0x138 or +0x188 set loads his
/// four SNDS names (0x31A70) into 0x2D + 4k, then the enemy cries (LoadEnemySounds 0x31A10).
pub fn tactical_plan(soldiers: &[u8]) -> Vec<BankLoad> {
    let mut out = Vec::new();
    let mut add = |folder, archive: &str, member: String, slot| {
        out.push(BankLoad { folder, archive: archive.to_string(), member, slot });
    };
    for (k, n) in OVERHEAD.iter().enumerate() {
        add("FX", "OVERHEAD", n.to_string(), k);
    }
    for k in 0..8 {
        let Some(r) = soldiers.get(k * 512..k * 512 + 512) else { break };
        if r[0x182] != 0 && (r[0x138] != 0 || r[0x188] != 0) {
            for (i, n) in ["ATT", "OK", "HIT", "DIE"].iter().enumerate() {
                add("SNDS", &r[0x179].to_string(), n.to_string(), 0x2D + 4 * k + i);
            }
        }
    }
    for i in 1..=4 {
        add("FX", "ENEMIES", format!("{}_HIT", i), 0x4D + 2 * (i - 1));
        add("FX", "ENEMIES", format!("{}_DIE", i), 0x4E + 2 * (i - 1));
    }
    out
}

impl Sound {
    /// Sound with the game's files: `exe` = JA.EXE (its folder holds FX\ and SNDS\), `iso` = the CD image.
    pub fn has_files(&self) -> bool {
        self.dir.is_some()
    }

    /// The CD image and JA.EXE paths, when the sound has the game's files.
    pub fn files(&self) -> Option<(String, String)> {
        Some((self.iso.clone()?, self.exe.clone()?))
    }

    pub fn with_files(iso: &str, exe: &str) -> Sound {
        let dir = std::path::Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
        Sound { dir: Some(dir), iso: Some(iso.to_string()), exe: Some(exe.to_string()), bank_stale: true, ..Default::default() }
    }

    fn archive(&self, folder: &str, name: &str) -> Option<Vec<u8>> {
        let d = self.dir.as_ref()?;
        // The hard-disk folders are upper case in the GOG install; try the name as given too.
        std::fs::read(d.join(folder).join(name)).ok().or_else(|| std::fs::read(d.join(folder).join(name.to_ascii_lowercase())).ok())
    }

    /// ArcSeek 0x534A4 on a voice archive: the member's bytes; a found member sets word 0x337940.
    fn seek_member(&self, arc: &[u8], name: &str) -> Option<Vec<u8>> {
        let d = archive_member(arc, name).ok()?;
        self.last_size.set(d.len());
        Some(d)
    }

    fn load(&mut self, arc: &Option<Vec<u8>>, member: &str, effect: usize) {
        if self.effects.len() < EFFECTS {
            self.effects.resize(EFFECTS, None);
        }
        // SPEC GAP (sound.md "Files"): a missing member leaves the effect empty.
        self.effects[effect] = arc.as_ref().and_then(|a| self.seek_member(a, &format!("{}.8", member))).map(|d| Sample {
            name: member.to_string(),
            data: Arc::new(d),
        });
    }

    /// sound.md "Tactical bank": OVERHEAD into 0..44, each team soldier's SNDS into 0x2D + 4k .., ENEMIES 0x4D ...
    pub fn load_tactical(&mut self, soldiers: &[u8]) {
        self.bank_stale = false;
        if self.dir.is_none() {
            return;
        }
        let mut open: Option<(&str, String, Option<Vec<u8>>)> = None;
        for e in tactical_plan(soldiers) {
            if !open.as_ref().is_some_and(|o| o.0 == e.folder && o.1 == e.archive) {
                open = Some((e.folder, e.archive.clone(), self.archive(e.folder, &e.archive)));
            }
            let arc = open.as_ref().and_then(|o| o.2.clone());
            self.load(&arc, &e.member, e.slot);
        }
    }

    /// sound.md "Main bank": MAINSND into effects 0..26; nothing stops.
    pub fn load_main(&mut self) {
        if self.dir.is_none() {
            return;
        }
        let m = self.archive("FX", "MAINSND");
        for (k, n) in MAINSND.iter().enumerate() {
            self.load(&m, n, k);
        }
    }

    /// sound.md "Output": `us` microseconds of screen time (end-of-day frames and waits) in 10 ms ticks.
    pub fn pass_us(&mut self, us: u64) {
        self.rest_us += us;
        let ticks = self.rest_us / 10_000;
        self.rest_us %= 10_000;
        if ticks > 0 {
            self.pass(ticks as u32);
        }
    }

    /// "Every sound stops".
    pub fn stop_all(&mut self) {
        self.chans = Default::default();
        self.sunset = None;
        self.cry_chan = None;
        self.speech = None;
    }

    /// SAB op 56 (ending.md): effect slot `slot` holds the raw sample `data` (None: the slot is empty).
    pub fn set_effect(&mut self, slot: usize, name: &str, data: Option<&[u8]>) {
        if self.effects.len() <= slot {
            self.effects.resize(slot + 1, None);
        }
        self.effects[slot] = data.map(|d| Sample { name: name.to_string(), data: Arc::new(d.to_vec()) });
    }

    /// SAB speech (ending.md): the speech channel plays the raw sample `data` once; nothing while Sound FX is off.
    pub fn speech_raw(&mut self, name: &str, rate: i32, vol: i32, data: &[u8]) {
        // SpeechStream 0x5CC6C: nothing plays while Sound FX (g_options 0x1163B8) is off.
        if self.fx_off {
            return;
        }
        self.started.push(format!("speech:{}", name));
        self.lip_view = None;
        self.speech = Some(Chan::new(Sample { name: name.to_string(), data: Arc::new(data.to_vec()) }, rate, vol, 1));
    }

    /// panel.md "The item's voice": the parts in turn on the speech channel (22050 Hz, volume 70), one sample made
    /// of them all; nothing plays while Sound FX is off. Returns the member names that play.
    pub fn item_voice(&mut self, parts: &[crate::iteminfo::VoicePart]) -> Vec<String> {
        let mut data = Vec::new();
        let mut names = Vec::new();
        for p in parts {
            let Some(arc) = self.archive("INV", p.archive) else { continue };
            let Ok(d) = archive_member(&arc, &p.member) else { continue };
            // A VOC member: its data from offset 0x21 to the end minus 1 byte (sound.md "Op 101 voice").
            let d = if p.voc { d.get(0x21..d.len().saturating_sub(1)).unwrap_or(&[]).to_vec() } else { d };
            data.extend_from_slice(&d);
            names.push(format!("{}\\{}", p.archive, p.member));
        }
        if !self.fx_off && !data.is_empty() {
            self.speech_raw(&format!("inv:{}", names.join("+")), RATE as i32, 70, &data);
        }
        names
    }

    /// 0x5C16C: the seek to a speech member. The old speech ends first, also when the archive has no such member or no
    /// archive is open. The member's bytes come back. With no open archive the original seeks on the old handle; every
    /// caller opens an archive first, so here that gives nothing.
    pub fn speech_seek(&mut self, arc: Option<&[u8]>, member: &str) -> Option<Vec<u8>> {
        self.speech = None;
        archive_member(arc?, member).ok()
    }

    /// SpeechStream 0x5CC6C loads one buffer of 0x4000 bytes and ShowMessage? 0x27B98 runs no SpeechRefill? 0x5CCF8: under
    /// a box the voice goes silent at byte 0x4000 and stays busy (live: position 16384, status 4, until the box ends).
    /// Port fix P12 (#71, #2 row 55): at level `all` the voice plays on under the box (no stall).
    pub fn speech_stall(&mut self) {
        if ja1logic::fanfix::port_on(12) {
            return;
        }
        if let Some(c) = self.speech.as_mut() {
            c.stall_at = Some((STREAM_BUFFER as u64) << 16);
        }
    }

    /// The first SpeechRefill? 0x5CCF8 after the box: the rest of the voice plays.
    pub fn speech_resume(&mut self) {
        if let Some(c) = self.speech.as_mut() {
            c.stall_at = None;
        }
    }

    /// True when the speech channel holds a sample (busy word 0x96372).
    pub fn speech_busy(&self) -> bool {
        self.speech.is_some()
    }

    /// inventory.md "inv_extra": member `member` of `INV\EXTRA` on the speech channel (22050 Hz, volume 70, once);
    /// nothing while Sound FX is off. False when nothing plays.
    pub fn inv_extra(&mut self, member: &str) -> bool {
        let Some(arc) = self.archive(INV_EXTRA.0, INV_EXTRA.1) else { return false };
        let Ok(d) = archive_member(&arc, member) else { return false };
        if self.fx_off || d.is_empty() {
            return false;
        }
        self.speech_raw(&format!("inv:EXTRA\\{}", member), crate::invextra::RATE, crate::invextra::VOLUME, &d);
        true
    }

    /// True when `INV\<archive>` holds `member`.
    pub fn inv_has(&self, archive: &str, member: &str) -> bool {
        self.archive("INV", archive).is_some_and(|a| archive_member(&a, member).is_ok())
    }

    /// strategic.md "The battle show": the speech channel plays member `name` of `FX\BATTLE` at rate 11025, volume
    /// 88, once; nothing while Sound FX is off. False when nothing plays.
    pub fn battle(&mut self, name: &str) -> bool {
        if self.fx_off {
            return false;
        }
        let arc = self.archive("FX", "BATTLE");
        let Some(d) = self.speech_seek(arc.as_deref(), name) else { return false };
        // ResolveSectorBattle 0x3E508 runs one SpeechRefill? 0x5CCF8 after each SpeechStream 0x5CC6C, and the next round starts
        // a new stream: a battle sound stops after two buffers (32768 bytes, 2.97 s; all of them are longer).
        self.speech_raw(&format!("battle:{}", name), 0x2B11, 0x58, battle_part(&d));
        true
    }

    /// strategic.md "The day replay": the `FX\REPLAY` member `name`.8 (the replay bank) on the first free effect
    /// channel at 11025 Hz, volume `vol`, once. False: Sound FX off, no such member or no free channel.
    pub fn replay(&mut self, name: &str, vol: i32) -> bool {
        if self.fx_off {
            return false;
        }
        let Some(d) = self.archive("FX", "REPLAY").and_then(|a| archive_member(&a, &format!("{}.8", name)).ok()) else { return false };
        self.start(Sample { name: format!("replay:{}", name), data: Arc::new(d) }, 0x2B11, vol, 1).is_some()
    }

    /// The speech channel stops.
    pub fn speech_stop(&mut self) {
        self.speech = None;
    }

    /// Every effect channel stops (the speech goes on).
    pub fn effects_stop(&mut self) {
        self.chans = Default::default();
    }

    /// Op 60 sfx: the first free effect channel starts the effect.
    pub fn sfx(&mut self, effect: i32, rate: i32, vol: i32, loops: i32) {
        let Some(s) = self.effects.get(effect as usize).cloned().flatten() else { return };
        self.start(s, rate, vol, loops);
    }

    /// As `sfx`; the channel number (None: no free channel or an empty effect).
    pub fn sfx_chan(&mut self, effect: i32, rate: i32, vol: i32, loops: i32) -> Option<usize> {
        let s = self.effects.get(effect as usize).cloned().flatten()?;
        self.start(s, rate, vol, loops)
    }

    /// PlayVocSample 0x5C9C0: the cry sample on its handle; nothing starts while Sound FX is off (gate bytes 0x96399,
    /// g_options 0x1163B8). Every caller goes through this one body: ops 101, 193 and 186.
    fn play_voc(&mut self, rate: i32, vol: i32, loops: i32) {
        if let (false, Some(s)) = (self.fx_off, self.cry.clone()) {
            self.started.push(s.name.clone());
            self.cry_chan = Some(Chan::new(s, rate, vol as u8 as i8 as i32, loops));
        }
    }

    /// Effect channel `k` stops.
    pub fn chan_stop(&mut self, k: usize) {
        if let Some(c) = self.chans.get_mut(k) {
            *c = None;
        }
    }

    /// sound.md "Button sounds": effect `fx` (0 press, 1 release) at 11025, volume 20, once; the channel of the
    /// last button sound stops first when it still plays.
    pub fn button(&mut self, fx: i32) {
        if let Some(k) = self.button.0.take() {
            self.chans[k] = None;
        }
        let Some(s) = self.effects.get(fx as usize).cloned().flatten() else { return };
        self.button = LastButton(self.start(s, 0x2B11, 20, 1));
    }

    /// sfx_diff: 5 ints per op (kind 0 = effect a at rate b, volume c, loops d; 1 = button sound a; 2 = channel a ends;
    /// 3 = sound effects off when a != 0; 4 = every sound stops; 5 = PlayVocSample of the cry sample: rate a, volume b, loops
    /// c). 37 ints per op: the channel (-1 none), then per channel its effect (-1 free), step, volume, loops; the last 4 =
    /// the cry channel.
    pub fn run_ops(ops: &[i32], out: &mut [i32]) {
        let mut snd = Sound { dir: Some(PathBuf::new()), ..Default::default() };
        snd.effects = (0..6).map(|k| Some(Sample { name: k.to_string(), data: Arc::new(vec![128; 100]) })).collect();
        snd.cry = Some(Sample { name: "9".to_string(), data: Arc::new(vec![128; 100]) });
        for (o, r) in ops.chunks(5).zip(out.chunks_mut(37)) {
            r[0] = -1;
            match o[0] {
                0 => r[0] = snd.sfx_chan(o[1], o[2], o[3], o[4]).map_or(-1, |k| k as i32),
                1 => snd.button(o[1]),
                2 => snd.chan_stop(o[1] as usize),
                3 => snd.fx_off = o[1] != 0,
                4 => snd.stop_all(),
                _ => snd.op(101, &o[1..4], &[]),
            }
            for (k, c) in snd.chans.iter().chain(std::iter::once(&snd.cry_chan)).enumerate() {
                let q = &mut r[1 + 4 * k..5 + 4 * k];
                q.copy_from_slice(&match c {
                    Some(c) => [c.s.name.parse().unwrap_or(-2), c.step as i32, c.vol, c.loops],
                    None => [-1, 0, 0, 0],
                });
            }
        }
    }

    /// voice_diff (lib.rs ja1f_voice): the bytes that a sound loader leaves.
    pub fn voice_probe(mut self, kind: i32, a: i32, b: i32) -> Option<Vec<u8>> {
        if kind == 12 {
            // gate_name_quote: the refusal quote of speaker a with name sample b, mixed to the channel's end: the
            // voice's bytes, the samples mixed, and 1 when the channel still plays.
            self.name_voice(a as u8, b as u8);
            self.cat_quote("AIM", a as u8, 6);
            let total = self.speech_bytes();
            let mut buf = vec![0i32; 1024];
            let mut mixed = 0usize;
            while self.speaking() && mixed < 1 << 24 {
                self.render_add(&mut buf);
                mixed += buf.len();
            }
            return Some(format!("{} {} {}", total, mixed, self.speaking() as u8).into_bytes());
        }
        if kind == 3 {
            let mut soldiers = vec![0u8; 512];
            (soldiers[0x179], soldiers[0x182], soldiers[0x138]) = (a as u8, 1, 1);
            self.load_tactical(&soldiers);
            return self.effects.get(0x2D + b as usize).cloned().flatten().map(|s| s.data.to_vec());
        }
        match kind {
            // The whole bank: main, doors, the first 17 overhead effects (op 209), then effect b.
            6 => self.load_main(),
            7 => self.op(209, &[0], &[]),
            8 => self.op(209, &[1], &[]),
            9 => self.load_tactical(&[]),
            _ => {}
        }
        if (6..=9).contains(&kind) {
            return self.effects.get(b as usize).cloned().flatten().map(|s| s.data.to_vec());
        }
        if kind == 10 {
            self.load_main();
            self.op(227, &[], &[]);
            return self.chans[0].as_ref().map(|c| [1, c.step as i32, c.vol, c.loops].iter().flat_map(|v| v.to_le_bytes()).collect());
        }
        if kind == 11 {
            self.speech_raw("x", 0x5622, 0x46, &[128; 4]);
            self.speech_stop();
            return self.speech.map(|_| vec![1]);
        }
        if kind == 4 || kind == 5 {
            self.fx_off = kind == 5;
            self.speech_raw("x", a, b, &[128; 4]);
            return self.speech.map(|c| [c.step as i32, c.vol, c.loops].iter().flat_map(|v| v.to_le_bytes()).collect());
        }
        self.cry = Some(Sample { name: "old".to_string(), data: Arc::new(vec![1, 2, 3]) });
        self.last_size.set(0x7000);
        match kind {
            0 => self.name_voice(a as u8, b as u8),
            1 => self.op(100, &[a, 0, b], &[]),
            _ => self.op(186, &[], &[]),
        }
        self.cry.map(|s| s.data.to_vec())
    }

    /// The first free effect channel starts `s`; its number, None when every channel plays. PlaySfxSlot 0x5CB34 takes the
    /// volume as a signed byte.
    fn start(&mut self, s: Sample, rate: i32, vol: i32, loops: i32) -> Option<usize> {
        if self.fx_off {
            return None;
        }
        let vol = vol as u8 as i8 as i32;
        let k = self.chans.iter().position(|c| c.is_none())?;
        self.started.push(s.name.clone());
        self.chans[k] = Some(Chan::new(s, rate, vol, loops));
        Some(k)
    }

    /// One host call; sound ops start or load sounds, every op answers as before (the caller's result).
    pub fn op(&mut self, op: i32, a: &[i32], soldiers: &[u8]) {
        if self.dir.is_none() {
            return;
        }
        let arg = |i: usize| a.get(i).copied().unwrap_or(0);
        match op {
            60 => self.sfx(arg(0), arg(1), arg(2) as u8 as i8 as i32, arg(3)),
            // op 227 inv_click: sound.md "Click sound" (effect 0 IN, 11025, volume 6, once)
            227 => self.sfx(0, 0x2B11, 6, 1),
            // op 65 effect_slot: the caller (it holds the records) asks effect_slot and sends op 60
            100 => {
                let c = arg(2);
                if let Some(n) = CRIES.get(c as usize) {
                    let p = arg(0) as u8;
                    let v = self.archive("SNDS", &p.to_string()).and_then(|b| self.seek_member(&b, &format!("{}.VOC", n)));
                    self.cry = v.map(|d| {
                        let end = d.len().saturating_sub(1).max(0x21);
                        Sample { name: format!("voc:{}", n), data: Arc::new(d.get(0x21..end).unwrap_or(&[]).to_vec()) }
                    });
                }
            }
            // sound.md "Requests", op 186 death_show: FX\OVERHEAD DEATH.VOC becomes the cry sample and plays.
            186 => {
                let v = self.archive("FX", "OVERHEAD").and_then(|b| self.seek_member(&b, "DEATH.VOC"));
                self.cry = v.map(|d| {
                    let end = d.len().saturating_sub(1).max(0x21);
                    Sample { name: "voc:DEATH".to_string(), data: Arc::new(d.get(0x21..end).unwrap_or(&[]).to_vec()) }
                });
                self.play_voc(0x2B11, 0x6E, 1);
            }
            // PlayVocSample 0x5C9C0: nothing starts while Sound FX is off (gate bytes 0x96399, g_options 0x1163B8).
            101 | 193 => {
                self.play_voc(arg(0), arg(1), arg(2));
            }
            209 => {
                self.stop_all();
                if arg(0) == 0 {
                    let d = self.archive("FX", "DOORS");
                    for (k, n) in DOORS.iter().enumerate() {
                        self.load(&d, n, k);
                    }
                } else {
                    let o = self.archive("FX", "OVERHEAD");
                    for (k, n) in OVERHEAD.iter().enumerate().take(17) {
                        self.load(&o, n, k);
                    }
                }
            }
            210 => self.door_cues(arg(0), arg(1), arg(2)),
            254 => {
                self.load_main();
                self.sunset = None;
                if let Some(smp) = self.effects.get(SUNSET_FX as usize).cloned().flatten() {
                    self.sunset = self.start(smp, 0x2B11, 50, 0);
                }
            }
            258 => {
                if let Some(k) = self.sunset.take() {
                    self.chans[k] = None;
                }
            }
            259 => {
                self.stop_all();
                self.load_tactical(soldiers);
            }
            _ => {}
        }
    }

    /// Op 210 door_anim (big, r, phase, x, y) of tileset `tileset` (PlayDoorFx 0x3B03C): the film member
    /// `<P>_<D><A><n>.FLC` of the CD archive `DAT\<n>.DAT` (FormatSectorDatName 0x2F7C8: n = the tileset, 5 for
    /// tileset 6) and its bytes (None: no film for r, or no such member). Phase 0 keeps the film's play time for the
    /// cues of phase 1: (frames - 1) x frame time (ending.md "Films": frame i shows at i x frame time).
    pub fn door_film(&mut self, big: i32, r: i32, phase: i32, tileset: u8) -> Option<(String, Vec<u8>)> {
        if phase == 0 {
            self.door_ms = 0;
        }
        let n = sector_dat_number(tileset);
        let member = format!("{}{}.FLC", door_base(big, r, phase)?, n);
        let mut cd = crate::iso::Iso::open(self.iso.as_ref()?).ok()?;
        let arc = crate::iso::game_file(self.exe.as_deref(), &mut cd, &sector_dat_name(tileset)).ok()?;
        let data = archive_member(&arc, &member).ok()?;
        if phase == 0 {
            let f = crate::flc::Flc::new(&data)?;
            self.door_ms = (f.frames.saturating_sub(1) as u64) * f.speed_ms as u64;
        }
        Some((member, data))
    }

    /// Op 210 door_anim: the film's cue file; each cue starts later with the effect as it is now (the outcome
    /// film's cues after the try film's play time).
    fn door_cues(&mut self, big: i32, r: i32, phase: i32) {
        let Some(base) = door_base(big, r, phase) else { return };
        let name = format!("{}.BIN", base);
        let after = if phase == 0 { 0 } else { self.door_ms * RATE as u64 / 1000 };
        let Some(b) = self.dir.as_ref().and_then(|d| std::fs::read(d.join("FX").join(&name)).ok()) else { return };
        for i in 0..50 {
            let Some(e) = b.get(4 * i..4 * i + 4) else { break };
            let f = i16::from_le_bytes([e[0], e[1]]) as i32;
            let s = i16::from_le_bytes([e[2], e[3]]) as i32;
            if f == 9999 || !(0..99).contains(&s) {
                continue;
            }
            if let Some(smp) = self.effects.get(s as usize).cloned().flatten() {
                let ms = if name == "I_PDHAN.BIN" { 114 } else { CUE_FRAME_MS } as u64;
                let due = self.made + after + ((f - 1).max(0) as u64 * ms * RATE as u64) / 1000;
                self.cues.push((due, smp));
            }
        }
    }

    /// sound.md "Merc quotes": the voice of quote q of profile p (DAY category) on the speech channel.
    pub fn quote(&mut self, p: u8, q: i32) {
        self.cat_quote("DAY", p, q);
    }

    /// speech.md "Quote categories": the voice of quote q of speaker p in category `cat` (archive CAT\p, member
    /// `<p><CAT><q>.8` or its two halves A and B) on the speech channel.
    pub fn cat_quote(&mut self, cat: &str, p: u8, q: i32) {
        self.cat_quotes(cat, p, &[q]);
    }

    /// strategic.md "Refusal reason" (host op 337 aim_remark): speaker p's sample of merc m's name, archive
    /// `NAMES\<p>` on the CD, member `<m>.VOC` (two digits), becomes the cry sample (the VOC buffer).
    pub fn name_voice(&mut self, p: u8, m: u8) {
        let Some(iso) = self.iso.clone() else { return };
        let Ok(mut cd) = crate::iso::Iso::open(&iso) else { return };
        let Ok(arc) = cd.read(&format!("NAMES\\{}", p)) else { return };
        // A missing member (speaker 29 has no 40.VOC): ArcSeek does not seek and keeps the size, so LoadVocSample 0x5C960 reads
        // the old size of bytes from the place behind the directory read (OpenFileOrCD? 0x53338: 0x50 records of 0x20 B).
        // Port fix P18 (#71, #2 row 51): a missing member gives no name (silence where it goes); the kept size becomes
        // the VOC header alone (0x22), as the fixed original's LoadVocSample.
        let d = self.seek_member(&arc, &format!("{:02}.VOC", m)).unwrap_or_else(|| {
            if ja1logic::fanfix::port_on(18) {
                self.last_size.set(0x22);
                return Vec::new();
            }
            let n = self.last_size.get();
            arc.get(0xA00..).map(|r| r[..n.min(r.len())].to_vec()).unwrap_or_default()
        });
        let end = d.len().saturating_sub(1).max(0x21);
        self.cry = Some(Sample { name: format!("voc:names{}_{:02}", p, m), data: Arc::new(d.get(0x21..end).unwrap_or(&[]).to_vec()) });
    }

    /// The voice of quote `q` (speech.md "Quote categories": the member, or its A half, the cry sample (the VOC
    /// buffer: the name of a merc) at volume 0x68 and its B half, one after the other).
    /// Second result: the bytes of the cry sample inside the voice.
    fn cat_voice(&self, cat: &str, p: u8, q: i32) -> Option<(Vec<u8>, std::ops::Range<usize>)> {
        let iso = self.iso.clone()?;
        let mut cd = crate::iso::Iso::open(&iso).ok()?;
        // speech.md: NAT quotes come from one archive NAT\NAT, members without a speaker prefix (`NAT30.8`).
        let (file, base) = if cat == "NAT" { ("NAT\\NAT".to_string(), format!("NAT{}", q)) } else { (format!("{}\\{}", cat, p), format!("{}{}{}", p, cat, q)) };
        let arc = cd.read(&file).ok()?;
        // A before B: the last size stays the one of the last half the speech wait seeks (SpeechSeek? 0x5C16C for B).
        let whole = self.seek_member(&arc, &format!("{}.8", base));
        let (a, b) = if whole.is_some() {
            (None, None)
        } else {
            (self.seek_member(&arc, &format!("{}A.8", base)), self.seek_member(&arc, &format!("{}B.8", base)))
        };
        let plan = quote_plan(whole.is_some(), a.is_some(), b.is_some());
        if plan.is_empty() {
            return None;
        }
        // No member (#2 row 62): a miss does not seek and keeps the size, so the stream reads the kept size of bytes from
        // the place behind the directory read (0x50 records of 0x20 B), as name_voice; seen live 2026-11-06 (#2 comment 12038).
        let kept = arc.get(0xA00..).map(|r| r[..self.last_size.get().min(r.len())].to_vec()).unwrap_or_default();
        let raw = self.cry.as_ref().map(|c| c.data.to_vec()).unwrap_or_default();
        let mut out = Vec::new();
        let mut name = 0..0;
        for (part, vol) in plan {
            match part {
                QuotePart::Whole => out.extend_from_slice(whole.as_deref().unwrap_or(&[])),
                QuotePart::A => out.extend_from_slice(a.as_deref().unwrap_or(&[])),
                QuotePart::B => out.extend_from_slice(b.as_deref().unwrap_or(&[])),
                QuotePart::Kept => out.extend_from_slice(&kept),
                // The channel plays one sample at QUOTE_VOL: the cry sample is scaled to its own volume.
                QuotePart::Name => {
                    name = out.len()..out.len() + raw.len();
                    out.extend(to_quote_volume(&raw, vol));
                }
            }
        }
        Some((out, name))
    }

    /// The speech channel plays the voices of quotes `qs` (volume QUOTE_VOL) in turn, 1000 ms apart (panel.md "The guide's and
    /// Brenda's balloons", second quote); a quote without a voice is left out.
    pub fn cat_quotes(&mut self, cat: &str, p: u8, qs: &[i32]) {
        let mut data = Vec::new();
        let mut names = Vec::new();
        let mut lip = Vec::new();
        for &q in qs {
            let Some((d, name)) = self.cat_voice(cat, p, q) else { continue };
            if !data.is_empty() {
                data.resize(data.len() + RATE as usize, 128);
            }
            let at = data.len();
            lip.push(at + name.start..at + name.end);
            data.extend_from_slice(&d);
            let base = format!("{}{}{}", p, cat, q);
            self.started.push(format!("quote:{}", base));
            names.push(base);
        }
        if !names.is_empty() {
            self.lip_view = lip_bytes(&data, &lip);
            self.speech = Some(Chan::new(Sample { name: names.join("+"), data: Arc::new(data) }, RATE as i32, QUOTE_VOL, 1));
        }
    }

    /// The effect channels that play now.
    pub fn effects_playing(&self) -> usize {
        self.chans.iter().flatten().count()
    }

    /// The bytes of the sample on the speech channel (0 when it does not play).
    pub fn speech_bytes(&self) -> usize {
        self.speech.as_ref().map_or(0, |c| c.s.data.len())
    }

    /// The time (ms) the speech channel still plays at its rate (0 when it does not play).
    pub fn speech_ms_left(&self) -> u64 {
        match &self.speech {
            Some(c) if c.step > 0 => {
                let left = ((c.s.data.len() as u64) << 16).saturating_sub(c.pos) / c.step;
                left * 1000 / RATE as u64
            }
            _ => 0,
        }
    }

    /// The samples of the voice on the speech channel and its playback time (µs); None when it does not play.
    pub fn speech_lip(&self) -> Option<(Arc<Vec<u8>>, u64)> {
        self.speech.as_ref().map(|c| (self.lip_view.clone().unwrap_or_else(|| c.s.data.clone()), (c.pos >> 16) * 1_000_000 / RATE as u64))
    }

    /// True while the speech channel plays.
    pub fn speaking(&self) -> bool {
        self.speech.is_some()
    }

    /// The time that the speech channel still plays.
    pub fn speech_left(&self) -> std::time::Duration {
        let Some(c) = &self.speech else { return std::time::Duration::ZERO };
        let frames = (((c.s.data.len() as u64) << 16).saturating_sub(c.pos)) / c.step.max(1);
        std::time::Duration::from_millis(frames * 1000 / RATE as u64)
    }

    /// The ticks of sound passed so far.
    pub fn ticks(&self) -> u64 {
        self.passes
    }

    /// `ticks` ticks of the play loop, 10 ms of sound each.
    pub fn pass(&mut self, ticks: u32) {
        self.passes += ticks as u64;
        let target = self.passes * RATE as u64 / 100;
        // music.md "Output": without a window's music device the music joins this mix.
        let mut music = vec![0i32; target.saturating_sub(self.made) as usize];
        if !crate::music::DEVICE.load(std::sync::atomic::Ordering::Relaxed) {
            if self.wav.is_some() || self.live {
                crate::music::with(|m| m.render(&mut music));
            } else {
                crate::music::with(|m| m.advance(music.len()));
            }
        }
        let mut k = 0;
        while self.made < target {
            let mut due = Vec::new();
            self.cues.retain(|(t, s)| {
                if *t <= self.made {
                    due.push(s.clone());
                    false
                } else {
                    true
                }
            });
            for s in due {
                self.start(s, 0x2B11, 50, 1);
            }
            let mut sum = music.get(k).copied().unwrap_or(0);
            k += 1;
            for c in self.chans.iter_mut().chain(std::iter::once(&mut self.cry_chan)).chain(std::iter::once(&mut self.speech)) {
                if let Some(ch) = c.as_mut() {
                    match ch.next() {
                        Some(v) => sum += v,
                        None => *c = None,
                    }
                }
            }
            let v = sum.clamp(-32768, 32767) as i16;
            if self.live {
                self.pending.push(v);
            }
            if let Some(w) = self.wav.as_mut() {
                w.push(v);
            }
            self.made += 1;
        }
    }

    /// Adds the effect channels' next `out.len()` output samples to `out` (the window's audio callback).
    pub fn render_add(&mut self, out: &mut [i32]) {
        for o in out.iter_mut() {
            for c in self.chans.iter_mut().chain(std::iter::once(&mut self.speech)) {
                if let Some(ch) = c.as_mut() {
                    match ch.next() {
                        Some(v) => *o += v,
                        None => *c = None,
                    }
                }
            }
        }
    }

    /// The `sound` script line.
    pub fn line(&mut self) -> String {
        let s = std::mem::take(&mut self.started);
        if s.is_empty() {
            "sound -".to_string()
        } else {
            format!("sound {}", s.join(" "))
        }
    }
}

/// sound.md "Requests", op 65 effect_slot (k, c): the op 60 sfx (effect, rate, volume) of soldier k's own sound,
/// None when nothing plays; `soldiers` = the records from slot 0 (record k at k × 512). A merc's sound sets its
/// voice timer u32 +0xC0 = 400.
/// The door film and cue name without the tileset digit and extension (PlayDoorFx 0x3B03C): P = `I` for phase 0,
/// else `S` when r >= 9 and `F` when r < 9; D = `PD` for a big door, else `DR`; A by r: 6 or 9 `KEY`, 7 or 10 `PCK`,
/// 8 or 11 `HAN` (another r: None).
pub fn door_base(big: i32, r: i32, phase: i32) -> Option<String> {
    let act = match r {
        6 | 9 => "KEY",
        7 | 10 => "PCK",
        8 | 11 => "HAN",
        _ => return None,
    };
    let p = if phase == 0 { "I" } else if r >= 9 { "S" } else { "F" };
    Some(format!("{}_{}{}", p, if big != 0 { "PD" } else { "DR" }, act))
}

pub fn effect_slot(soldiers: &mut [u8], k: i32, c: i32) -> Option<(i32, i32, i32)> {
    let k = k as u8 as usize;
    let c = c as u8 as i32;
    let r = k * 512;
    let rec = soldiers.get_mut(r..r + 512)?;
    let (effect, vol) = if k >= 8 {
        if c < 2 {
            return None;
        }
        (0x4D + 2 * rec[0x1E0] as i32 + (c - 2), 0x58)
    } else {
        let timer = i32::from_le_bytes(rec[0xC0..0xC4].try_into().unwrap());
        if (c < 2 && timer > 0) || rec[0x16C] != 0 {
            return None;
        }
        rec[0xC0..0xC4].copy_from_slice(&400u32.to_le_bytes());
        (0x2D + 4 * k as i32 + c, 0x32)
    };
    let (h, e) = (rec[0x183] as i8 as i32, rec[0x18A] as i8 as i32);
    let rate = if h < 50 || e < 60 { 0x2B11 - 5 * (50 - h) - 5 * (60 - e) } else { 0x2B11 };
    // the effect number is a byte; effect 0 plays nothing
    let effect = effect & 0xFF;
    if effect == 0 {
        return None;
    }
    Some((effect, rate, vol))
}

/// A WAV file (22050 Hz, mono, 16 bits) of `pcm`.
/// The sounds of the screens outside the tactical view (office, menus): one process state with the main bank;
/// the window mixes it in its audio callback beside the music (`render_add`).
/// Tests that change the screen sound (`screen`) and tests that read it back hold this lock (cargo runs tests in parallel).
#[cfg(test)]
pub(crate) static SCREEN_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn screen<R>(f: impl FnOnce(&mut Sound) -> R) -> R {
    static S: OnceLock<Mutex<Sound>> = OnceLock::new();
    let m = S.get_or_init(|| Mutex::new(Sound::default()));
    f(&mut m.lock().unwrap_or_else(|e| e.into_inner()))
}

/// A film screen (intro, end show, credits) gives the screen sound the game's files when it has none yet; a sound
/// that holds files keeps them and its state.
pub fn screen_files(iso: &str, exe: &str) {
    screen(|s| {
        if !s.has_files() {
            *s = Sound::with_files(iso, exe);
        }
    });
}

/// A screen opens with the main bank (sound.md "Main bank"): the screen sound gets the game's files first.
pub fn screen_main(iso: &str, exe: &str) {
    screen(|s| {
        if !s.has_files() {
            *s = Sound::with_files(iso, exe);
        }
        s.load_main();
    });
}

/// speech.md "Quote categories", in the game's order.
/// What the lip clock reads of a voice: the cry sample parts hold no silence. The original scans the stream parts only
/// (SpeechRefill? 0x5CCF8 + LipSyncScan), and ReleaseTacticalTimer? 0x66D94 clears the silence byte 0x337C9F when a stream
/// ends, so the mouth keeps its random steps during the name (FaceMouthStep 0x27FB8). None: no cry sample part.
/// The stream part before a name ends the scan there: LipSyncScan 0x66DB8 records a quiet run only at the next loud
/// sample (0x66E0F), and LipSyncReset 0x66D50 clears the run at the next stream's start, so the quiet run at the end
/// of that part is never a silence (the mouth keeps its random steps). The port's one view would close that run with
/// the name's bytes, so the run is made loud too.
fn lip_bytes(data: &[u8], names: &[std::ops::Range<usize>]) -> Option<Arc<Vec<u8>>> {
    if names.iter().all(|r| r.is_empty()) {
        return None;
    }
    let mut v = data.to_vec();
    for r in names {
        v[r.clone()].fill(0);
        let mut i = r.start;
        while !r.is_empty() && i > 0 && (0x7B..=0x85).contains(&v[i - 1]) {
            i -= 1;
            v[i] = 0;
        }
    }
    Some(Arc::new(v))
}

/// The lip view of one voice with one cry sample part `name` (faceanim_diff `parts` cases).
pub fn lip_view(data: &[u8], name: std::ops::Range<usize>) -> Vec<u8> {
    lip_bytes(data, std::slice::from_ref(&name)).map(|v| v.to_vec()).unwrap_or_else(|| data.to_vec())
}

/// One part of a quote's voice (PlayQuote 0x24F08 mode byte 0x284F23, SpeechWait? 0x280DB and 0x28268..0x2834D).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum QuotePart {
    Whole,
    A,
    Name,
    B,
    /// No member: the stream from the place behind the archive's directory, the size of the last member found.
    Kept,
}

/// The parts of a quote and their volumes, in play order, from the members the archive holds. Whole member: mode 0.
/// A alone: mode 1, A then the cry sample at 0x68. B alone: mode 2, the cry sample at 0x58 then B. Both halves: mode 3,
/// A, the cry sample at 0x68, B. No member: mode 1 with no seek (#2 rows 53 and 62): the stream plays the kept bytes
/// (QuotePart::Kept) in place of A, then the cry sample at 0x68.
pub fn quote_plan(whole: bool, a: bool, b: bool) -> Vec<(QuotePart, i32)> {
    match (whole, a, b) {
        (true, _, _) => vec![(QuotePart::Whole, QUOTE_VOL)],
        (false, true, false) => vec![(QuotePart::A, QUOTE_VOL), (QuotePart::Name, NAME_VOL)],
        (false, false, true) => vec![(QuotePart::Name, QUOTE_VOL), (QuotePart::B, QUOTE_VOL)],
        (false, true, true) => vec![(QuotePart::A, QUOTE_VOL), (QuotePart::Name, NAME_VOL), (QuotePart::B, QUOTE_VOL)],
        (false, false, false) => vec![(QuotePart::Kept, QUOTE_VOL), (QuotePart::Name, NAME_VOL)],
    }
}

pub const CATS: [&str; 11] = ["AIM", "DAY", "PST", "JAC", "INV", "SIG", "QST", "BRE", "NAT", "SAN", "INT"];

/// Host op 302 guide_say (q, c) or 317 brenda_say (q) (panel.md "The guide's and Brenda's balloons"): the speaker,
/// the category and the quotes (the second part when there is one).
pub fn npc_parts(op: i32, a: &[i32]) -> Option<(u8, usize, Vec<i32>)> {
    let arg = |i: usize| a.get(i).copied().unwrap_or(0);
    match op {
        302 => {
            let (q, c) = (arg(0), arg(1));
            let c = usize::try_from(c).ok().filter(|&c| c < CATS.len())?;
            Some((0x40, c, if q < 0x28 && c == 6 { vec![q, q + 1] } else { vec![q] }))
        }
        317 => {
            let q = arg(0);
            Some((0x41, 7, if q < 10 { vec![q, q + 1] } else { vec![q] }))
        }
        _ => None,
    }
}

/// Ops 302 / 317 on the screen sound (the main bank loads first when it has no files yet and `files` gives them):
/// the voices start; returns one `speech C Q TEXT` line per quote (the EDT text record Q of the category).
pub fn npc_say(files: Option<(&str, &str)>, op: i32, a: &[i32]) -> Vec<String> {
    let Some((p, c, qs)) = npc_parts(op, a) else { return Vec::new() };
    screen(|s| {
        if !s.has_files() {
            if let Some((iso, exe)) = files {
                *s = Sound::with_files(iso, exe);
                s.load_main();
            }
        }
        s.cat_quotes(CATS[c], p, &qs);
        let (Some(iso), Some(exe)) = (s.iso.clone(), s.exe.clone()) else { return Vec::new() };
        let edt = crate::aim::edt(&iso, &exe, &format!("{}QUOTE.EDT", CATS[c]));
        qs.iter().map(|&q| {
            let at = 200 * q.max(0) as usize;
            let t = crate::balloon::shared_text(&edt, at);
            format!("speech {} {} {}", c, q, String::from_utf8_lossy(&t))
        }).collect()
    })
}

/// A button sound on the screen sound (sound.md "Button sounds"); the main bank loads first when the screen sound
/// has no files yet. Prints `screen sound NAME` on stderr (the play script reads it).
pub fn screen_button(iso: &str, exe: &str, fx: i32) {
    screen(|s| {
        if !s.has_files() {
            *s = Sound::with_files(iso, exe);
            s.load_main();
        }
        s.button(fx);
    });
    eprintln!("screen sound {}", main_name(fx));
}

/// A button sound on the screen sound when it holds the game's files (a question box over a screen that loaded the
/// main bank); prints `screen sound NAME` on stderr.
pub fn screen_button_loaded(fx: i32) {
    if screen(|s| { let f = s.has_files(); if f { s.button(fx); } f }) {
        eprintln!("screen sound {}", main_name(fx));
    }
}

/// A main-bank effect on the screen sound (effect, rate, volume, times); the main bank loads first when the screen
/// sound has no files yet. Prints `screen sound NAME` on stderr.
pub fn screen_sfx(iso: &str, exe: &str, (fx, rate, vol, n): (i32, i32, i32, i32)) {
    let busy = screen(|s| {
        if !s.has_files() {
            *s = Sound::with_files(iso, exe);
            s.load_main();
        }
        s.sfx(fx, rate, vol, n);
        s.chans.iter().flatten().count()
    });
    eprintln!("screen sound {}", main_name(fx));
    eprintln!("screen chans {}", busy);
}

pub fn wav_bytes(pcm: &[i16]) -> Vec<u8> {
    let n = (pcm.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + n as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + n).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&n.to_le_bytes());
    for v in pcm {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

#[cfg(test)]
mod stall_tests {
    use super::*;

    /// Ledger rows d057aca:ja1front:21456/21457: the time left of a voice = its frames at the mixer rate (the map
    /// screen waits for it, mapui speech wait).
    #[test]
    fn the_time_left_of_a_voice_is_its_length_at_its_rate() {
        let mut s = Sound::default();
        assert_eq!(s.speech_left(), std::time::Duration::ZERO);
        s.speech_raw("v", RATE as i32, 127, &vec![0x80; RATE as usize]);
        assert_eq!(s.speech_left().as_millis(), 1000);
        s.speech_raw("v", RATE as i32 / 2, 127, &vec![0x80; RATE as usize / 4]);
        assert!((499..=500).contains(&s.speech_left().as_millis()), "{:?}", s.speech_left()); // the step rounds down
    }

    #[test]
    fn pass_us_keeps_the_rest_of_a_tick() {
        // The waits of the sunset and the replay (BusyWait 0x67250 in ms) run on the same clock as the sound: the mixer
        // goes 100 ticks per second, and a part of a tick waits for the next call. 4 waits of 15 ms = 6 ticks.
        let mut s = Sound::default();
        for _ in 0..4 {
            s.pass_us(15_000);
        }
        assert_eq!((s.passes, s.rest_us), (6, 0));
    }

    /// SpeechStream 0x5CC6C + ShowMessage? 0x27B98: live in the original, position 16384 and status 4 until the box ended.
    #[test]
    fn a_stalled_voice_stops_at_one_buffer_and_stays_busy() {
        let _lock = crate::sound::TEST_FIX_MASK.lock().unwrap_or_else(|e| e.into_inner());
        let mask = ja1logic::fanfix::port_mask();
        ja1logic::fanfix::port_set(0);
        let mut s = Sound::default();
        s.speech_raw("x", RATE as i32, 70, &vec![200u8; STREAM_BUFFER + 100]);
        s.speech_stall();
        let mut out = vec![0i32; STREAM_BUFFER + 500];
        s.render_add(&mut out);
        assert!(out[STREAM_BUFFER - 1] != 0, "the first buffer plays to its last byte");
        assert!(out[STREAM_BUFFER..].iter().all(|v| *v == 0), "silence after the first buffer");
        assert!(s.speech_busy());
        assert_eq!(s.speech.as_ref().unwrap().pos >> 16, STREAM_BUFFER as u64);
        s.speech_resume();
        let mut out = vec![0i32; 200];
        s.render_add(&mut out);
        assert!(out[99] != 0, "the rest plays after the box");
        assert!(out[100] == 0 && !s.speech_busy(), "and the voice ends at its last byte");
        // #71 P12 (#2 row 55): at level `all` the voice plays on under the box.
        ja1logic::fanfix::port_set(1 << 11);
        let mut s = Sound::default();
        s.speech_raw("x", RATE as i32, 70, &vec![200u8; STREAM_BUFFER + 100]);
        s.speech_stall();
        let mut out = vec![0i32; STREAM_BUFFER + 500];
        s.render_add(&mut out);
        ja1logic::fanfix::port_set(mask);
        assert!(out[STREAM_BUFFER + 99] != 0, "P12: no silence after the first buffer");
        assert!(out[STREAM_BUFFER + 100] == 0 && !s.speech_busy(), "P12: the voice ends at its last byte");
        // #71 P13 (#2 row 56): a battle sound of 40000 bytes stops at 32768 in 1.13b, plays all of it at level `all`.
        let d = vec![1u8; 40000];
        ja1logic::fanfix::port_set(0);
        let off = super::battle_part(&d).len();
        ja1logic::fanfix::port_set(1 << 12);
        let all = super::battle_part(&d).len();
        ja1logic::fanfix::port_set(mask);
        assert_eq!((off, all), (2 * STREAM_BUFFER, 40000), "P13 battle sound length");
    }

    /// ResolveSectorBattle: one refill per stream, so no battle sound goes past two buffers.
    #[test]
    fn a_battle_sound_stops_after_two_buffers() {
        let (iso, exe) = ("../../rom/JA1.gog", "../../build/game113b/JA.EXE");
        if !std::path::Path::new(iso).exists() || !std::path::Path::new(exe).exists() {
            return;
        }
        let _lock = crate::sound::TEST_FIX_MASK.lock().unwrap_or_else(|e| e.into_inner());
        let mask = ja1logic::fanfix::port_mask();
        ja1logic::fanfix::port_set(0);
        let mut s = Sound::with_files(iso, exe);
        let ok = s.battle("DIE0.8");
        let n = s.speech_bytes();
        ja1logic::fanfix::port_set(mask);
        assert!(ok);
        assert_eq!(n, 2 * STREAM_BUFFER, "DIE0.8 holds 54174 bytes");
    }
}

/// The channel plays one sample at QUOTE_VOL: a part with its own volume `vol` (the name cry) is scaled to it around
/// the 8-bit middle 128.
pub fn to_quote_volume(raw: &[u8], vol: i32) -> Vec<u8> {
    raw.iter().map(|&v| (128 + (v as i32 - 128) * vol / QUOTE_VOL).clamp(0, 255) as u8).collect()
}

#[cfg(test)]
mod tests {
    /// Ledger row c2aea50:ja1front:2385: the intro gives a screen sound without files the game's files; a sound with
    /// files keeps them.
    #[test]
    fn a_film_screen_gives_the_sound_its_files() {
        let _g = super::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        super::screen(|s| *s = super::Sound::default());
        super::screen_files("a.gog", "a/JA.EXE");
        assert_eq!(super::screen(|s| s.files()), Some(("a.gog".to_string(), "a/JA.EXE".to_string())));
        super::screen_files("b.gog", "b/JA.EXE");
        assert_eq!(super::screen(|s| s.files()), Some(("a.gog".to_string(), "a/JA.EXE".to_string())));
        super::screen(|s| *s = super::Sound::default());
    }

    /// #70 row 52 (changelog 1.15q, found by DarzaR; 1.15m SetSoundVolume 0x5CEA5 / SetMusicVolume 0x5CEC5 skip the
    /// AIL call when the driver handle 0x337780 / 0x337790 is 0, a crash without a sound card). The port has no driver
    /// handle: with no sound device the volume keys still change the numbers and set the masters, and nothing fails.
    #[test]
    fn volume_keys_work_without_a_sound_device() {
        let old = super::volumes();
        super::set_volumes(super::Volumes { music: 50, sound: 50 });
        assert!(super::press_volume_key(b'.').is_some());
        assert_eq!(super::digital_master(), 51);
        assert!(super::press_volume_key(b',').is_some());
        assert_eq!(super::digital_master(), 50);
        assert!(super::press_volume_key(b'+').is_some());
        assert!(super::press_volume_key(b'-').is_some());
        assert_eq!((super::volumes().music, super::volumes().sound), (50, 50));
        // #71 P09 (#2 row 41), in this test because both share the volume statics: Quick Restore's numbers reach the
        // masters only with the fix.
        let _lock = super::TEST_FIX_MASK.lock().unwrap_or_else(|e| e.into_inner());
        let mask = ja1logic::fanfix::port_mask();
        ja1logic::fanfix::port_set(0);
        super::restore_volumes(super::Volumes { music: 40, sound: 30 });
        assert_eq!(super::digital_master(), 50);
        assert_ne!(crate::music::with(|m| m.master()), 40);
        ja1logic::fanfix::port_set(1 << 8);
        super::restore_volumes(super::Volumes { music: 40, sound: 30 });
        assert_eq!((super::digital_master(), crate::music::with(|m| m.master())), (30, 40));
        assert_eq!((super::volumes().music, super::volumes().sound), (40, 30));
        // #71 P17 (#2 row 40): the numbers start at the masters' 127, so the first louder key changes nothing; in
        // 1.13b they start at 100 / 70 and the first louder key sends 100 / 71 (quieter than 127).
        ja1logic::fanfix::port_set(0);
        super::set_volumes(super::Volumes { music: 100, sound: 70 });
        super::start_volumes();
        assert_eq!(super::press_volume_key(b'+'), Some(super::MasterSet::Music(100)));
        assert_eq!(super::press_volume_key(b'.'), Some(super::MasterSet::Digital(71)));
        ja1logic::fanfix::port_set(1 << 16);
        super::set_volumes(super::Volumes { music: 100, sound: 70 });
        super::start_volumes();
        assert_eq!(super::press_volume_key(b'+'), Some(super::MasterSet::Music(127)));
        assert_eq!(super::press_volume_key(b'.'), Some(super::MasterSet::Digital(127)));
        assert_eq!(super::press_volume_key(b'-'), Some(super::MasterSet::Music(126)));
        ja1logic::fanfix::port_set(mask);
        super::set_volumes(old);
    }

    /// Ledger rows c2aea50:ja1front:21202/21206: the name cry at half the quote volume swings half as far from 128.
    #[test]
    fn the_name_cry_is_scaled_to_its_volume() {
        let q = super::QUOTE_VOL;
        assert_eq!(super::to_quote_volume(&[128, 228, 28, 255, 0], q / 2), vec![128, 178, 78, 191, 64]);
        assert_eq!(super::to_quote_volume(&[200], q), vec![200]);
    }

    /// Ledger row c2aea50:ja1front:21231: one second of 22050 Hz speech has about 1000 ms left at its start.
    #[test]
    fn a_second_of_speech_has_1000_ms_left() {
        let mut s = super::Sound::default();
        s.speech_raw("test", 22050, 70, &vec![128u8; 22050]);
        let ms = s.speech_ms_left();
        assert!((990..=1000).contains(&ms), "{ms}");
    }
    /// Ledger rows ja1front:20313/20314/20337/20338/20339: the speech channel names its byte count, and the lip sync
    /// time of sample 2205 at 22050 Hz is 100 ms (100,000 µs); no speech gives 0 bytes and no lip data.
    #[test]
    fn speech_bytes_and_lip_time() {
        let mut s = super::Sound::default();
        assert_eq!(s.speech_bytes(), 0);
        assert!(s.speech_lip().is_none());
        s.speech_raw("test", 22050, 70, &vec![128u8; 4410]);
        assert_eq!(s.speech_bytes(), 4410);
        s.speech.as_mut().unwrap().pos = 2205 << 16;
        let (data, us) = s.speech_lip().unwrap();
        assert_eq!((data.len(), us), (4410, 100_000));
    }

    /// Ledger row ja1front:20503: GuideSpeech? 0x5F5AC plays a second part q + 1 only for q < 0x28 in category 6
    /// (asm 0x5F649 `cmp esi,0x28; jge`, 0x5F651 `cmp edx,6`); BrendaSpeech? 0x5F76C only for q < 10 (0x5F7FE).
    #[test]
    fn guide_and_brenda_second_part_bounds() {
        use super::npc_parts;
        assert_eq!(npc_parts(302, &[0x27, 6]), Some((0x40, 6, vec![0x27, 0x28])));
        assert_eq!(npc_parts(302, &[0x28, 6]), Some((0x40, 6, vec![0x28])));
        assert_eq!(npc_parts(302, &[0x10, 5]), Some((0x40, 5, vec![0x10])));
        assert_eq!(npc_parts(302, &[0, 11]), None);
        assert_eq!(npc_parts(317, &[9]), Some((0x41, 7, vec![9, 10])));
        assert_eq!(npc_parts(317, &[10]), Some((0x41, 7, vec![10])));
    }

    /// Ledger rows ja1front:20368 and c2aea50:ja1front:21277: 8-bit unsigned samples above 128 (the DAC's middle) mix
    /// as positive output, both in the play loop's own mix (`pass`) and in the window callback (`render_add`).
    #[test]
    fn samples_above_128_mix_positive() {
        let mut s = super::Sound::default();
        s.speech_raw("test", 22050, 70, &vec![228u8; 4410]);
        let mut out = vec![0i32; 8];
        s.render_add(&mut out);
        assert!(out.iter().all(|&v| v > 0), "{out:?}");
        let mut s = super::Sound::default();
        s.live = true;
        s.speech_raw("test", 22050, 70, &vec![228u8; 4410]);
        s.pass(1);
        assert!(!s.pending.is_empty() && s.pending.iter().all(|&v| v > 0));
    }

    /// sound.md "Op 259 overhead_sounds": every sound stops (the tactical bank is then loaded again).
    #[test]
    fn op_259_stops_every_sound() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let mut s = super::Sound::with_files(iso.to_str().unwrap(), exe.to_str().unwrap());
        s.op(186, &[], &[]);
        assert!(s.cry_chan.is_some(), "a death cry plays");
        s.op(259, &[], &[]);
        assert!(s.cry_chan.is_none() && s.chans.iter().all(|c| c.is_none()));
    }

    /// Ledger row c2aea50:ja1front:21052 (inventory.md "inv_extra"): with Sound FX off the item voice does not play,
    /// so the box does not wait for it; with Sound FX on it plays.
    #[test]
    fn the_item_voice_stays_silent_with_sound_fx_off() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        for (off, plays) in [(true, false), (false, true)] {
            let mut s = super::Sound::with_files(iso.to_str().unwrap(), exe.to_str().unwrap());
            s.fx_off = off;
            assert!(s.inv_has("EXTRA", "B0.8"));
            assert_eq!(s.inv_extra("B0.8"), plays);
            assert_eq!(s.speech.is_some(), plays);
        }
    }

    /// The screens ask `files` before they load a voice or an effect: a sound with no game files names none.
    #[test]
    fn a_sound_without_files_names_no_files() {
        assert_eq!(super::Sound::default().files(), None);
        assert_eq!(super::Sound::with_files("a.iso", "g/JA.EXE").files(), Some(("a.iso".to_string(), "g/JA.EXE".to_string())));
    }

    #[test]
    fn death_cry_stays_silent_with_sound_fx_off() {
        // Op 186 (MercDies? 0x3A5C8): PlayVocSample's gate, the same body as op 101 (sfx_diff).
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        for (off, plays) in [(true, false), (false, true)] {
            let mut s = super::Sound::with_files(iso.to_str().unwrap(), exe.to_str().unwrap());
            s.fx_off = off;
            s.op(186, &[], &[]);
            assert!(s.cry.is_some());
            assert_eq!(s.cry_chan.is_some(), plays);
        }
    }

    #[test]
    fn every_bank_member_exists() {
        // Later point "an empty effect": the banks of the target game hold no missing member and no zero-length member,
        // so the original's zero-length start (PlaySfxSlot 0x5CB34) and the port's free channel cannot differ.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let s = super::Sound::with_files(iso.to_str().unwrap(), exe.to_str().unwrap());
        let mut bad = Vec::new();
        let mut check = |folder: &str, arc: &str, member: &str| match s
            .archive(folder, arc)
            .and_then(|a| super::archive_member(&a, &format!("{}.8", member)).ok())
        {
            Some(d) if !d.is_empty() => {}
            _ => bad.push(format!("{}/{}/{}", folder, arc, member)),
        };
        for n in super::MAINSND {
            check("FX", "MAINSND", n);
        }
        for n in super::OVERHEAD {
            check("FX", "OVERHEAD", n);
        }
        for n in super::DOORS {
            check("FX", "DOORS", n);
        }
        for i in 1..=4 {
            check("FX", "ENEMIES", &format!("{}_HIT", i));
            check("FX", "ENEMIES", &format!("{}_DIE", i));
        }
        for p in 0..64 {
            for n in ["ATT", "OK", "HIT", "DIE"] {
                check("SNDS", &p.to_string(), n);
            }
        }
        assert!(bad.is_empty(), "{:?}", bad);
    }

    #[test]
    fn the_lip_clock_counts_the_played_samples() {
        // FaceMouthStep 0x27FB8 reads the lip scan at the place the stream played to: 22050 samples at 22050 Hz = 1 s,
        // 11025 more = 1.5 s (the lip clock is the played sample count / RATE, in µs).
        let mut s = Sound::default();
        s.speech = Some(Chan::new(Sample { name: "T".into(), data: Arc::new(vec![128; 44100]) }, RATE as i32, QUOTE_VOL, 1));
        for _ in 0..22050 {
            s.speech.as_mut().unwrap().next();
        }
        assert_eq!(s.speech_lip().map(|l| l.1), Some(1_000_000));
        for _ in 0..11025 {
            s.speech.as_mut().unwrap().next();
        }
        assert_eq!(s.speech_lip().map(|l| l.1), Some(1_500_000));
    }

    #[test]
    fn the_name_part_of_a_quote_is_the_cry_sample() {
        // speech.md "Quote categories": an A half, then the cry sample (the name) at volume 0x68, then the B half. The
        // second result of cat_voice is the place of the name: the lip view reads it as no silence (lip_bytes).
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let mut s = super::Sound::with_files(iso.to_str().unwrap(), exe.to_str().unwrap());
        let mut seen = 0;
        for p in 0..64u8 {
            s.name_voice(p, 3);
            let cry = s.cry.as_ref().map(|c| c.data.to_vec()).unwrap_or_default();
            if cry.is_empty() {
                continue;
            }
            for q in 0..60 {
                let Some((out, name)) = s.cat_voice("DAY", p, q) else { continue };
                if name.is_empty() {
                    continue;
                }
                assert_eq!(name.len(), cry.len(), "speaker {} quote {}: the name part is not the cry sample", p, q);
                assert!(name.end <= out.len(), "speaker {} quote {}: the name part ends behind the voice", p, q);
                let scaled: Vec<u8> = cry.iter().map(|&v| (128 + (v as i32 - 128) * super::NAME_VOL / super::QUOTE_VOL).clamp(0, 255) as u8).collect();
                assert_eq!(&out[name.clone()], &scaled[..], "speaker {} quote {}: other bytes at the name part", p, q);
                seen += 1;
            }
            if seen >= 3 {
                break;
            }
        }
        assert!(seen >= 3, "no DAY quote with a name part");
    }

    /// cat_quotes: the lip view zeroes the name part of each quote at its place in the joined voice (the second quote
    /// starts one second of silence after the first).
    #[test]
    fn two_quotes_zero_each_name_part() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let mut s = super::Sound::with_files(iso.to_str().unwrap(), exe.to_str().unwrap());
        for p in 0..64u8 {
            s.name_voice(p, 3);
            for q in 0..60 {
                let Some((out, name)) = s.cat_voice("DAY", p, q) else { continue };
                if name.is_empty() || name.start == 0 {
                    continue;
                }
                s.cat_quotes("DAY", p, &[q, q]);
                let v = s.lip_view.clone().expect("a lip view");
                let at = out.len() + super::RATE as usize;
                assert_eq!(v.len(), 2 * out.len() + super::RATE as usize);
                for base in [0, at] {
                    assert!(v[base + name.start..base + name.end].iter().all(|&b| b == 0));
                    assert_eq!(&v[base + name.end..base + out.len()], &out[name.end..]);
                }
                return;
            }
        }
        panic!("no DAY quote with a name part");
    }

    #[test]
    fn the_name_inside_a_quote_holds_no_silence() {
        // A loud half, a quiet name, a loud half: a whole sample scan finds a silence in the name, the lip view does not.
        let mut d = vec![0u8; 22050];
        let name = 22050..44100;
        d.extend(vec![128u8; name.len()]);
        d.extend(vec![0u8; 22050]);
        assert!(!crate::faceanim::silences(&d).is_empty());
        let v = super::lip_bytes(&d, std::slice::from_ref(&name)).unwrap();
        assert!(crate::faceanim::silences(&v).is_empty());
        assert_eq!(v.len(), d.len());
        assert!(super::lip_bytes(&d, &[0..0]).is_none());
    }

    #[test]
    fn the_quiet_end_of_the_part_before_a_name_is_no_silence() {
        // Part A: loud, a closed quiet run (a silence), loud, then a quiet end of 2000 samples; the name; part B loud.
        // LipSyncScan 0x66DB8 records A's inner run only: its end run has no loud sample after it in the stream.
        let mut d = vec![0u8; 5000];
        d.extend(vec![128u8; 1500]);
        d.extend(vec![0u8; 3000]);
        d.extend(vec![130u8; 2000]);
        let name = d.len()..d.len() + 4000;
        d.extend(vec![0u8; 4000]);
        d.extend(vec![0u8; 5000]);
        let v = super::lip_bytes(&d, std::slice::from_ref(&name)).unwrap();
        assert_eq!(crate::faceanim::silences(&v), crate::faceanim::silences(&d[..name.start - 2000]));
        assert_eq!(crate::faceanim::silences(&v).len(), 1);
        assert_eq!(&v[..name.start - 2000], &d[..name.start - 2000]);
    }

    #[test]
    fn speech_stays_silent_with_sound_fx_off() {
        // SpeechStream 0x5CC6C: byte 0x1163B8 = 0 goes straight to the end, also for Walter's quote and the end show.
        let mut s = Sound::default();
        s.fx_off = true;
        s.speech_raw("wp2", 0x2B11, 0x7F, &[1, 2, 3]);
        assert!(s.started.is_empty() && !s.speech_busy());
        s.fx_off = false;
        s.speech_raw("wp2", 0x2B11, 0x7F, &[1, 2, 3]);
        assert_eq!(s.started, ["speech:wp2"]);
        assert!(s.speech_busy());
    }

    use super::*;

    #[test]
    fn queue_delay_falls_back_after_a_stall() {
        // The device plays one 55 ms frame per frame; the game mixes one frame per frame, with 110 ms already waiting
        // (below the 150 ms bound: no clear, no gap in the sound), but once it stalls for 2 s and then mixes those
        // 2 s at once. The queue must be back under 150 ms + one frame at once, and only that one clear may happen.
        // Platform code (no counterpart in the original): the bound is the #20 H design, 150 ms of mono i16.
        assert_eq!(QUEUE_MAX, 6615);
        assert!(!queue_stale(6615) && queue_stale(6616));
        let frame = RATE * 2 * 55 / 1000;
        let mut q = 2 * frame;
        let mut clears = Vec::new();
        for f in 0..200 {
            let part = if f == 50 { RATE * 2 * 2 } else { frame };
            if queue_stale(q) {
                q = 0;
                clears.push(f);
            }
            q += part;
            if f > 51 {
                assert!(q <= 6615 + frame, "frame {}: {} bytes queued", f, q);
            }
            q = q.saturating_sub(frame);
        }
        assert_eq!(clears, [51]);
    }

    #[test]
    fn channel_steps_and_loops() {
        let s = Sample { name: "T".into(), data: Arc::new(vec![128 + 64, 128 - 64]) };
        // 11025 Hz into 22050: each byte twice; volume 127 = full.
        let mut c = Chan::new(s.clone(), 11025, 127, 2);
        let v: Vec<_> = std::iter::from_fn(|| c.next()).collect();
        assert_eq!(v, vec![64 * 256, 64 * 256, -64 * 256, -64 * 256, 64 * 256, 64 * 256, -64 * 256, -64 * 256]);
        let mut snd = Sound { dir: Some(PathBuf::new()), ..Default::default() };
        snd.effects = vec![Some(s); 2];
        for _ in 0..9 {
            snd.sfx(0, 11025, 50, 0);
        }
        // 8 effect channels: the ninth start finds none free.
        assert_eq!(snd.started.len(), 8);
        assert_eq!(snd.line(), format!("sound {}", vec!["T"; 8].join(" ")));
        assert_eq!(snd.line(), "sound -");
    }

    #[test]
    fn door_films_of_the_tileset_archive() {
        assert_eq!(door_base(1, 10, 1).as_deref(), Some("S_PDPCK"));
        assert_eq!(door_base(0, 8, 1).as_deref(), Some("F_DRHAN"));
        assert_eq!(door_base(0, 5, 0), None);
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let (iso, exe) = (format!("{}/rom/JA1.gog", root), format!("{}/build/game113b/JA.EXE", root));
        if !std::path::Path::new(&iso).exists() {
            return;
        }
        let mut snd = Sound::with_files(&iso, &exe);
        // tileset 6 reads archive 5 (FormatSectorDatName 0x2F7C8); the try film's play time delays the outcome cues
        let (m, d) = snd.door_film(0, 8, 0, 6).expect("film");
        let f = crate::flc::Flc::new(&d).unwrap();
        assert_eq!((m.as_str(), snd.door_ms), ("I_DRHAN5.FLC", (f.frames as u64 - 1) * f.speed_ms as u64));
        assert!(snd.door_ms > 0);
        assert_eq!(snd.door_film(0, 8, 1, 0).map(|x| x.0).as_deref(), Some("F_DRHAN0.FLC"));
    }

    /// Ledger rows of `screen_main` and `screen_button`: the bank really loads on the screen sound (LoadMainSound 0x31B8C
    /// reads FX\\MAINSND, voice_diff) and the button routine 0x5C8A0 / 0x5C900 starts effect 0 / 1 of it (sfx_diff). The
    /// `screen sound` lines of the gates print also when no sound starts, so they cannot see this. The one test that
    /// uses the process-wide screen sound.
    #[test]
    fn screen_sound_gets_the_main_bank() {
        let _g = super::SCREEN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let (iso, exe) = (format!("{}/rom/JA1.gog", root), format!("{}/build/game113b/JA.EXE", root));
        let arc = std::fs::read(format!("{}/build/game113b/FX/MAINSND", root));
        if !std::path::Path::new(&iso).exists() || arc.is_err() {
            return;
        }
        let arc = arc.unwrap();
        let bytes = |k: usize| archive_member(&arc, &format!("{}.8", MAINSND[k])).unwrap();
        let playing = |s: &Sound| -> Vec<String> { s.chans.iter().flatten().map(|c| c.s.name.clone()).collect() };
        let bank = |s: &Sound| -> Vec<Vec<u8>> { (0..MAINSND.len()).map(|k| s.effects.get(k).cloned().flatten().map(|e| e.data.to_vec()).unwrap_or_default()).collect() };
        let want: Vec<Vec<u8>> = (0..MAINSND.len()).map(bytes).collect();
        // A button on a screen with no files yet: the bank loads first, then the press sound starts, and it is heard.
        screen(|s| *s = Sound::default());
        screen_button(&iso, &exe, 0);
        screen(|s| {
            assert_eq!(bank(s), want, "button: main bank");
            assert_eq!(playing(s).len(), 1, "button: press sound");
            let mut mix = vec![0i32; 2048];
            s.render_add(&mut mix);
            assert!(mix.iter().any(|v| *v != 0), "button: silent mix");
        });
        // The release sound on the same bank stops the press sound; the files stay.
        screen_button(&iso, &exe, 1);
        screen(|s| assert_eq!(playing(s).len(), 1, "button: release sound"));
        // A screen opens: the bank loads, and the menu sound (Menu::sound, effect 0) starts from it.
        screen(|s| *s = Sound::default());
        screen_main(&iso, &exe);
        screen(|s| {
            assert_eq!(bank(s), want, "screen: main bank");
            s.sfx(0, 0x2B11, 6, 1);
            assert_eq!(playing(s).len(), 1, "screen: menu sound");
        });
        // A second screen keeps a sound that plays (the bank loads again, "nothing stops").
        screen_main(&iso, &exe);
        screen(|s| assert_eq!(playing(s).len(), 1, "second screen: sound stopped"));
        screen(|s| *s = Sound::default());
    }

    #[test]
    fn button_sound_stops_the_last_one() {
        let s = Sample { name: "IN".into(), data: Arc::new(vec![128; 100]) };
        let mut snd = Sound { dir: Some(PathBuf::new()), ..Default::default() };
        snd.effects = vec![Some(s.clone()), Some(Sample { name: "OUT".into(), ..s })];
        snd.button(0);
        snd.sfx(0, 11025, 50, 0);
        // the press still plays on channel 0: the release stops it and takes the first free channel (0 again)
        snd.button(1);
        assert_eq!(snd.chans.iter().filter(|c| c.is_some()).count(), 2);
        assert_eq!(snd.chans[0].as_ref().map(|c| c.s.name.clone()), Some("OUT".into()));
        assert_eq!(snd.line(), "sound IN IN OUT");
    }
}

/// FormatSectorDatName 0x2F7C8: the number of the tileset's film archive (tileset 6 uses the archive of 5).
pub fn sector_dat_number(tileset: u8) -> u8 {
    if tileset == 6 { 5 } else { tileset }
}

/// FormatSectorDatName 0x2F7C8: the name of the tileset's film archive.
pub fn sector_dat_name(tileset: u8) -> String {
    format!("DAT\\{}.DAT", sector_dat_number(tileset))
}

#[cfg(test)]
mod gap_minor_tests {
    use super::*;

    fn smp(n: usize) -> Sample {
        Sample { name: "s".into(), data: Arc::new(vec![200; n]) }
    }

    /// Ledger row ja1front:20140 (panel.md "The item's voice"): an item with no voice parts leaves the voice that
    /// plays alone.
    #[test]
    fn an_item_without_voice_keeps_the_speech() {
        let mut s = Sound::default();
        s.speech_raw("v", RATE as i32, 70, &[200; 100]);
        assert!(s.item_voice(&[]).is_empty());
        assert_eq!(s.speech_bytes(), 100);
        assert_eq!(s.started, vec!["speech:v".to_string()]);
    }

    /// Ledger row 0b4d529:ja1front:21541: effects_stop frees every effect channel.
    #[test]
    fn effects_stop_frees_every_channel() {
        let mut s = Sound::default();
        s.chans[0] = Some(Chan::new(smp(10), RATE as i32, 70, 1));
        s.chans[7] = Some(Chan::new(smp(10), RATE as i32, 70, 1));
        assert_eq!(s.effects_playing(), 2);
        s.effects_stop();
        assert_eq!(s.effects_playing(), 0);
    }

    /// Ledger row c2aea50:ja1front:20954: one stream buffer (16384 bytes) at 22050 Hz plays 743 ms, the time the
    /// map screen takes off a stalled voice.
    #[test]
    fn one_stream_buffer_plays_743_ms() {
        assert_eq!(STREAM_BUFFER_TIME.as_millis(), 743);
        let mut s = Sound::default();
        s.speech_raw("v", RATE as i32, 70, &vec![200; STREAM_BUFFER]);
        assert_eq!(s.speech_left().as_millis(), STREAM_BUFFER_TIME.as_millis());
    }

    /// Ledger row c2aea50:ja1front:21220: a voice with rate 0 does not move; its time left is 0 (no division by 0).
    #[test]
    fn a_rate_0_voice_has_no_time_left() {
        let mut s = Sound::default();
        s.speech_raw("v", 0, 70, &[200; 100]);
        assert_eq!(s.speech_ms_left(), 0);
    }

    /// Ledger rows ja1front:20236, ja1front:20255, ja1front:20260 (op 210 door_anim, PlayDoorFx 0x3B03C): only phase
    /// 0 clears the try film's play time; a cue at frame f starts (f - 1) x frame time later, 114 ms per frame for
    /// I_PDHAN, 71 ms for the other films, rounded down in samples.
    #[test]
    fn door_film_time_and_cue_times() {
        let mut s = Sound::default();
        s.door_ms = 500;
        assert!(s.door_film(1, 8, 1, 0).is_none());
        assert_eq!(s.door_ms, 500);
        assert!(s.door_film(1, 8, 0, 0).is_none());
        assert_eq!(s.door_ms, 0);
        let dir = std::env::temp_dir().join(format!("ja1_doorcues_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("FX")).unwrap();
        let cue = [3i16.to_le_bytes(), 0i16.to_le_bytes()].concat();
        std::fs::write(dir.join("FX").join("I_PDHAN.BIN"), &cue).unwrap();
        std::fs::write(dir.join("FX").join("I_DRHAN.BIN"), &cue).unwrap();
        s.dir = Some(dir.clone());
        s.effects = vec![Some(smp(4))];
        s.door_cues(1, 8, 0);
        s.door_cues(0, 8, 0);
        std::fs::remove_dir_all(&dir).ok();
        let due: Vec<u64> = s.cues.iter().map(|c| c.0).collect();
        assert_eq!(due, vec![2 * 114 * RATE as u64 / 1000, 2 * 71 * RATE as u64 / 1000]);
    }
}
