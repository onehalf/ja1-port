//! The play settings file `ja1.cfg` (port option, not in the original): `key = value` lines, `#` comments. The game
//! reads it at the start and gives each value to its environment variable; a variable that the environment already
//! sets wins (the tests set them). `--setup` (setup.rs) shows the same options as switches and writes the file.

/// One option: the file key, the environment variable (empty = only the setup reads it), the values as
/// (file value, variable value, setup label), the default value index, the setup row label and its help line.
pub struct Opt {
    pub key: &'static str,
    pub env: &'static str,
    pub values: &'static [(&'static str, &'static str, &'static str)],
    pub default: usize,
    pub label: &'static str,
    pub help: &'static str,
}

impl Opt {
    /// A lever switch: two values, off then on.
    pub fn lever(&self) -> bool {
        self.values.len() == 2 && self.values[0].0 == "off" && self.values[1].0 == "on"
    }
}

const ONOFF: &[(&str, &str, &str)] = &[("off", "0", "Off"), ("on", "1", "On")];

pub const MOUSE: usize = 0;
pub const FIXES: usize = 11;
pub const AIM_BROWSER: usize = 12;

pub const OPTS: [Opt; 13] = [
    Opt { key: "capture_mouse", env: "JA1_MOUSE", values: &[("off", "desktop", "Off"), ("on", "dos", "On")], default: 1,
          label: "Grab mouse", help: "Ctrl+F10 frees the mouse." },
    Opt { key: "pointer_speed", env: "JA1_POINTER_SPEED",
          values: &[("slow", "0.5", "Slow"), ("original", "1", "DOS"), ("fast", "2", "Fast")], default: 1,
          label: "Speed", help: "Pointer speed with Grab mouse on." },
    Opt { key: "fullscreen", env: "JA1_FULLSCREEN", values: ONOFF, default: 0,
          label: "Fullscreen", help: "Black bars at the sides." },
    Opt { key: "window_size", env: "JA1_WINDOW_SCALE",
          values: &[("2", "2", "2x"), ("3", "3", "3x"), ("4", "4", "4x"), ("5", "5", "5x")], default: 2,
          label: "Size", help: "2x  640 x 400\n3x  960 x 600\n4x  1280 x 800\n5x  1600 x 1000" },
    Opt { key: "soft_pixels", env: "JA1_PIXELS", values: &[("off", "sharp", "Off"), ("on", "smooth", "On")],
          default: 0, label: "Soft pixels", help: "Soft edges, not square pixels." },
    Opt { key: "scanlines", env: "JA1_SCANLINES", values: ONOFF, default: 0,
          label: "Scanlines", help: "Dark lines, as an old monitor." },
    Opt { key: "music", env: "JA1_MUSIC", values: ONOFF, default: 1,
          label: "Music", help: "Off: no music at all." },
    Opt { key: "mute_when_away", env: "JA1_MUTE_UNFOCUSED", values: ONOFF, default: 1,
          label: "Mute away", help: "No sound while another window has the focus." },
    Opt { key: "subtitles", env: "JA1_SUBTITLES", values: ONOFF, default: 0,
          label: "Subtitles", help: "Always show the text of speech." },
    Opt { key: "title_films", env: "JA1_TITLE_FILMS", values: ONOFF, default: 1,
          label: "Title films", help: "Sir-tech logo, title and credits at the start." },
    Opt { key: "intro", env: "JA1_INTRO", values: ONOFF, default: 1,
          label: "Intro film", help: "The island film at a new game only." },
    // #71 (was #70 fan_fixes): off = 1.13b as Sir-tech made it, 1.15m = the fan patch fixes (ja1logic::fanfix F ids),
    // all = 1.15m + our own port fixes (P ids). An old `fan_fixes = on` line reads as 1.15m.
    Opt { key: "fixes", env: "JA1_FIXES", values: &[("off", "off", "Off"), ("1.15m", "1.15m", "1.15m"), ("all", "all", "All")],
          default: 0, label: "Fixes",
          help: "Off: the game as Sir-tech made it. 1.15m: fixes of the fan patch by DarzaR. All: 1.15m + port fixes." },
    // #42 (Insert #13454), the first mod of the port: A.I.M. opens on a grid of all mercs (aimgrid.rs).
    Opt { key: "aim_browser", env: "JA1_AIM_BROWSER", values: ONOFF, default: 0,
          label: "A.I.M. grid", help: "A.I.M. shows all mercs on one page, with sort and filter." },
];

/// The file name, beside the launchers (the working folder); JA1_CONFIG names another file.
pub fn path() -> std::path::PathBuf {
    std::env::var_os("JA1_CONFIG").map(Into::into).unwrap_or_else(|| "ja1.cfg".into())
}

/// The value index of each option in `text`; a missing key or an unknown value gives the default (one line in
/// `bad` for each unknown key or value).
pub fn parse(text: &str, bad: &mut Vec<String>) -> [usize; OPTS.len()] {
    let mut v = OPTS.each_ref().map(|o| o.default);
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((k, val)) = line.split_once('=') else {
            bad.push(format!("config: no '=' in {:?}", line));
            continue;
        };
        let (k, mut val) = (k.trim(), val.trim());
        // #71: the old fan_fixes switch is the fixes setting: on = 1.15m, off = off.
        let k = if k == "fan_fixes" {
            val = match val.to_ascii_lowercase().as_str() { "on" | "1" => "1.15m", "off" | "0" => "off", _ => val };
            "fixes"
        } else {
            k
        };
        // The old start switch (test saves on the lever) left the setup for the release: its line is skipped.
        if k == "start" {
            continue;
        }
        match OPTS.iter().position(|o| o.key == k) {
            Some(i) => match OPTS[i].values.iter().position(|x| x.0.eq_ignore_ascii_case(val)) {
                Some(j) => v[i] = j,
                None => bad.push(format!("config: {} = {:?} is not a value (default {})", k, val,
                                         OPTS[i].values[OPTS[i].default].0)),
            },
            None => bad.push(format!("config: unknown key {:?}", k)),
        }
    }
    v
}

/// The file text for the value indexes `v`.
pub fn write(v: &[usize; OPTS.len()]) -> String {
    let mut s = String::from("# JA1 port play settings. ja1front --setup writes this file; you can edit it by hand.\n\
                              # An environment variable of the same option wins over the file.\n");
    for (o, &i) in OPTS.iter().zip(v) {
        let all: Vec<&str> = o.values.iter().map(|x| x.0).collect();
        s += &format!("\n# {} ({})\n{} = {}\n", o.help.replace('\n', ", "), all.join(" | "), o.key, o.values[i.min(o.values.len() - 1)].0);
    }
    s
}

/// The environment pairs for `v`: each option with a variable that `is_set` does not report.
pub fn env_pairs(v: &[usize; OPTS.len()], is_set: impl Fn(&str) -> bool) -> Vec<(&'static str, &'static str)> {
    OPTS.iter().zip(v).filter(|(o, _)| !o.env.is_empty() && !is_set(o.env)).map(|(o, &i)| (o.env, o.values[i].1)).collect()
}

/// The game start: reads the file (if any) and sets the variables it gives; stderr gets one line per bad entry.
pub fn load_into_env() {
    let p = path();
    let Ok(text) = std::fs::read_to_string(&p) else { return };
    let mut bad = vec![];
    let v = parse(&text, &mut bad);
    for b in bad {
        eprintln!("{}", b);
    }
    for (k, val) in env_pairs(&v, |k| std::env::var_os(k).is_some()) {
        std::env::set_var(k, val);
    }
    eprintln!("config {} read", p.display());
}

/// The readers of the option variables (Inserts #13553, #13554): each takes the variable's value (None = not set) and
/// gives what the program uses. With no variable set a reader gives what the option's default value gives (test
/// readers_default_to_the_config_default), so a run with no ja1.cfg behaves as the setup screen says.
pub mod read {
    pub fn var(env: &str) -> Option<String> {
        std::env::var(env).ok()
    }
    /// capture_mouse (JA1_MOUSE): true = the DOS mouse (grabbed).
    pub fn capture_mouse(v: Option<&str>) -> bool { v.map_or(true, |v| v != "desktop") }
    /// pointer_speed (JA1_POINTER_SPEED): the factor, 0.25..4.
    pub fn pointer_speed(v: Option<&str>) -> f32 {
        v.and_then(|v| v.parse().ok()).filter(|f: &f32| (0.25..=4.0).contains(f)).unwrap_or(1.0)
    }
    pub fn fullscreen(v: Option<&str>) -> bool { v == Some("1") }
    /// window_size (JA1_WINDOW_SCALE): 1..8.
    pub fn window_size(v: Option<&str>) -> u32 { v.and_then(|v| v.parse().ok()).filter(|s| (1..=8).contains(s)).unwrap_or(4) }
    pub fn soft_pixels(v: Option<&str>) -> bool { v == Some("smooth") }
    pub fn scanlines(v: Option<&str>) -> bool { v == Some("1") }
    pub fn music(v: Option<&str>) -> bool { v != Some("0") }
    /// mute_when_away (JA1_MUTE_UNFOCUSED): default on (Insert #13554 item 2; the old reader `== "1"` was off).
    pub fn mute_when_away(v: Option<&str>) -> bool { v != Some("0") }
    pub fn subtitles(v: Option<&str>) -> bool { v == Some("1") }
    pub fn title_films(v: Option<&str>) -> bool { v != Some("0") }
    /// intro (JA1_INTRO) in the window. A play script plays the intro only with JA1_INTRO=1 (main.rs; its runs would
    /// take ~5 s more each), so both paths agree whenever the variable is set (ja1.cfg sets it).
    pub fn intro(v: Option<&str>) -> bool { v != Some("0") }
    pub fn fixes(v: Option<&str>) -> String { v.unwrap_or("off").to_string() }
    pub fn aim_browser(v: Option<&str>) -> bool { v == Some("1") }

    /// The reader of option `key` as text (for the guard test).
    pub fn show(key: &str, v: Option<&str>) -> String {
        match key {
            "capture_mouse" => capture_mouse(v).to_string(),
            "pointer_speed" => pointer_speed(v).to_string(),
            "fullscreen" => fullscreen(v).to_string(),
            "window_size" => window_size(v).to_string(),
            "soft_pixels" => soft_pixels(v).to_string(),
            "scanlines" => scanlines(v).to_string(),
            "music" => music(v).to_string(),
            "mute_when_away" => mute_when_away(v).to_string(),
            "subtitles" => subtitles(v).to_string(),
            "title_films" => title_films(v).to_string(),
            "intro" => intro(v).to_string(),
            "fixes" => fixes(v),
            "aim_browser" => aim_browser(v).to_string(),
            _ => panic!("no reader for option {key}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_file_reads_back_the_same() {
        let mut v = OPTS.each_ref().map(|o| o.default);
        v[MOUSE] = 0;
        v[3] = 0;
        v[FIXES] = 2;
        let mut bad = vec![];
        assert_eq!(parse(&write(&v), &mut bad), v);
        assert!(bad.is_empty(), "{:?}", bad);
    }

    #[test]
    fn bad_lines_give_the_default_and_a_note() {
        let mut bad = vec![];
        let v = parse("capture_mouse = maybe\nwindow_size=3 # three\nnope = 1\njunk\n", &mut bad);
        assert_eq!(v[MOUSE], 1);
        assert_eq!(v[3], 1);
        assert_eq!(bad.len(), 3, "{:?}", bad);
    }

    #[test]
    fn the_environment_wins_over_the_file() {
        let v = OPTS.each_ref().map(|o| o.default);
        let pairs = env_pairs(&v, |k| k == "JA1_MOUSE");
        assert!(!pairs.iter().any(|(k, _)| *k == "JA1_MOUSE"));
        assert!(pairs.contains(&("JA1_MUSIC", "1")));
        assert!(!pairs.iter().any(|(k, _)| k.is_empty()), "an option with no variable");
    }

    #[test]
    fn the_old_fan_fixes_line_reads_as_fixes() {
        let mut bad = vec![];
        assert_eq!(parse("fan_fixes = on\n", &mut bad)[FIXES], 1, "on = 1.15m");
        assert_eq!(parse("fan_fixes = off\n", &mut bad)[FIXES], 0);
        assert_eq!(parse("fixes = all\n", &mut bad)[FIXES], 2);
        assert!(bad.is_empty(), "{:?}", bad);
        assert_eq!(OPTS[FIXES].values[1].1, "1.15m");
    }

    #[test]
    fn the_old_start_line_is_skipped() {
        let mut bad = vec![];
        assert_eq!(parse("start = fight\n", &mut bad), OPTS.each_ref().map(|o| o.default));
        assert!(bad.is_empty(), "{:?}", bad);
        assert!(!OPTS.iter().any(|o| o.key == "start"));
    }

    #[test]
    fn the_aim_grid_is_off_by_default() {
        let mut bad = vec![];
        assert_eq!(OPTS[AIM_BROWSER].key, "aim_browser");
        assert_eq!(parse("", &mut bad)[AIM_BROWSER], 0);
        assert_eq!(parse("aim_browser = on\n", &mut bad)[AIM_BROWSER], 1);
        assert!(bad.is_empty(), "{:?}", bad);
        assert!(OPTS[AIM_BROWSER].lever());
    }

    #[test]
    fn every_default_is_a_value() {
        for o in &OPTS {
            assert!(o.default < o.values.len(), "{}", o.key);
            assert!(o.label.len() <= 13, "{} label too long for a switch row", o.key);
        }
    }

    /// Insert #13554 item 3: for each option with a variable, its reader with no variable set gives what the option's
    /// default value gives.
    #[test]
    fn readers_default_to_the_config_default() {
        for o in OPTS.iter().filter(|o| !o.env.is_empty()) {
            assert_eq!(read::show(o.key, None), read::show(o.key, Some(o.values[o.default].1)), "{}", o.key);
            let got: std::collections::BTreeSet<String> = o.values.iter().map(|(_, v, _)| read::show(o.key, Some(v))).collect();
            assert_eq!(got.len(), o.values.len(), "{}: two values read the same", o.key);
        }
    }
}
