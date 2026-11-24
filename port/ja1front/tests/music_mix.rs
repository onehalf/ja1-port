//! Ledger row c2aea50:ja1front:21270 (Insert #12730): with no music device of its own (music.md "Output": the
//! window's music device did not open, or a play script) the music joins the sound mix sample by sample, so the
//! mix of a pass is the music's own rendering, one sample after the other. A test of its own (own process): the
//! music player is one static. Skipped when rom/ is missing.
use std::path::PathBuf;

#[test]
fn the_music_joins_the_mix_sample_by_sample() {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../rom");
    let exe = d.join("JA.EXE");
    if !exe.exists() {
        eprintln!("rom/ missing: skipped");
        return;
    }
    assert!(!ja1front::music::DEVICE.load(std::sync::atomic::Ordering::Relaxed));
    ja1front::music::set_exe(&exe.to_string_lossy());
    // 1 s of the office theme through the sound mix (no effect plays).
    ja1front::music::screen("MENUS", 1);
    let mut s = ja1front::sound::Sound::default();
    s.wav = Some(Vec::new());
    s.pass(100);
    let mixed = s.wav.take().unwrap();
    assert_eq!(mixed.len(), ja1front::sound::RATE as usize);
    // Each sample takes the music's next sample: the wave moves (one held sample would give a flat line).
    let steps = mixed.windows(2).filter(|p| p[0] != p[1]).count();
    assert!(steps > mixed.len() / 4, "only {} changes in {} samples", steps, mixed.len());
}
