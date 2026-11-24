//! The game clock (docs/spec/world_load.md "Game speed", The game clock): the DOS time of day, which moves in steps
//! of one BIOS tick (65536 / 1193180 s, ~54.9 ms). Clock waits ("until at least t ms passed") end on such a step.
//! The original's TimerStart 0x47020 / TimerElapsedMs 0x47040 (ftime_ 0x76253) are replaced by this clock.
use std::sync::OnceLock;
use std::time::{Duration, Instant};

static START: OnceLock<Instant> = OnceLock::new();

/// The clock in ms (1000 × seconds + 10 × hundredths) for a BIOS tick count.
pub fn tick_ms(tick: u64) -> i64 {
    // hundredths since the start = floor(tick × 65536 / 1193180 s × 100)
    (10 * (tick as u128 * 6_553_600 / 1_193_180)) as i64
}

/// The BIOS tick count at a moment (ticks since the program start).
pub fn tick_at(t: Instant) -> u64 {
    let start = *START.get_or_init(Instant::now);
    (t.saturating_duration_since(start).as_nanos() * 1_193_180 / 65_536 / 1_000_000_000) as u64
}

/// world_load.md "Game time": every 10 ms of real time is one game tick, the original's 10 ms timer (the soldier
/// timer +0xB0 and ripple_timer count it: a merc walks 166 ms per tile, the view slides 50 ms per 5-tick step).
pub fn game_ticks(elapsed: Duration) -> u64 {
    elapsed.as_millis() as u64 / 10
}

/// FadePalette? 0x52908 writes one level per VGA vertical retrace (70 Hz, 14.286 ms): the moment that level `k`
/// (0-based) of a fade ends, from the fade's start.
pub fn retrace_due(k: usize) -> Duration {
    Duration::from_micros(14_286 * (k as u64 + 1))
}

/// The end of the next VGA vertical retrace (70 Hz) after `last`: a day-end fade level shows no earlier.
pub fn next_retrace(last: Instant) -> Instant {
    last + Duration::from_micros(1_000_000 / 70)
}

/// The clock now, in ms.
pub fn now_ms() -> i64 {
    tick_ms(tick_at(Instant::now()))
}

/// Wait until the clock reads at least `ms` more than `from` (a clock reading in ms). Returns the clock at the end.
pub fn wait_from(from: i64, ms: i64) -> i64 {
    loop {
        let now = now_ms();
        if now - from >= ms {
            return now;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// A clock wait that starts now (the original's TimerStart, then TimerElapsedMs >= `ms`).
pub fn sleep(ms: i64) {
    wait_from(now_ms(), ms);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_of_one_tick() {
        assert_eq!(tick_ms(0), 0);
        // 1 tick = 0.0549 s: 5 hundredths; 2 ticks = 0.1099 s: 10 hundredths; 18 ticks = 0.9887 s; 19 = 1.0436 s
        assert_eq!(tick_ms(1), 50);
        assert_eq!(tick_ms(2), 100);
        assert_eq!(tick_ms(18), 980);
        assert_eq!(tick_ms(19), 1040);
        // the clock never goes back and moves at most 60 ms per tick
        for k in 0..2000 {
            let d = tick_ms(k + 1) - tick_ms(k);
            assert!((50..=60).contains(&d), "tick {k}: {d}");
        }
    }

    #[test]
    fn the_clock_runs_at_the_rate_of_the_pc_timer() {
        // The PC timer runs at 1193180 Hz and the BIOS tick comes each 65536 counts: 18.2 ticks per second.
        let start = *START.get_or_init(Instant::now);
        assert_eq!(tick_at(start + Duration::from_secs(10)), 182);
    }

    /// Ledger rows c2aea50:ja1front:1059/1060/995/1381/1383/1594/1768 (the window's game time).
    #[test]
    fn one_game_tick_per_10_ms() {
        assert_eq!(game_ticks(Duration::from_millis(0)), 0);
        assert_eq!(game_ticks(Duration::from_millis(9)), 0);
        assert_eq!(game_ticks(Duration::from_millis(10)), 1);
        // the walk of 7 tile steps in window_speed: 1160 ms = 116 ticks
        assert_eq!(game_ticks(Duration::from_millis(1160)), 116);
        assert_eq!(game_ticks(Duration::from_millis(1234)), 123);
    }

    /// Ledger row c2aea50:ja1front:805 (fade_show): 56 fade steps take 800 ms, one 70 Hz retrace each.
    #[test]
    fn a_fade_level_per_retrace() {
        assert_eq!(retrace_due(0), Duration::from_micros(14_286));
        assert_eq!(retrace_due(55).as_millis(), 800);
        assert_eq!(retrace_due(23).as_millis(), 342);
    }

    /// Ledger row c2aea50:ja1front:1958 (WinEod::show): a day-end fade level waits for the next 70 Hz retrace after
    /// the last one.
    #[test]
    fn the_next_retrace_is_1_70_s_later() {
        let t = Instant::now();
        assert_eq!(next_retrace(t) - t, Duration::from_micros(14_285));
    }

    #[test]
    fn now_ms_moves_with_the_clock() {
        let a = now_ms();
        std::thread::sleep(Duration::from_millis(130));
        assert!(now_ms() - a >= 100);
    }

    #[test]
    fn a_short_wait_ends_on_the_next_step() {
        // the sunset's "2 ms" waits: 4 in a row take 3 clock steps after the first (~165 ms)
        let mut c = wait_from(now_ms(), 2);
        let t = Instant::now();
        for _ in 0..3 {
            c = wait_from(c, 2);
        }
        let ms = t.elapsed().as_millis();
        assert!((150..=200).contains(&ms), "{ms} ms");
    }
}
