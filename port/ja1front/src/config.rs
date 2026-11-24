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
pub const START: usize = 11;

pub const OPTS: [Opt; 12] = [
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
    Opt { key: "start", env: "",
          values: &[("menu", "", "Menu"), ("continue", "DAY.SAV", "Last day"), ("fight", "DAY_fight48d.SAV", "Fight 48"),
                    ("full-day", "DAY_day3.SAV", "Day 3"), ("grenade", "DAY_grenade.SAV", "Grenade"),
                    ("no-money", "DAY_no_money.SAV", "No money")],
          default: 0, label: "Go", help: "What the lever starts." },
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
        let (k, val) = (k.trim(), val.trim());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_file_reads_back_the_same() {
        let mut v = OPTS.each_ref().map(|o| o.default);
        v[MOUSE] = 0;
        v[3] = 0;
        v[START] = 2;
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
        assert!(!pairs.iter().any(|(k, _)| k.is_empty()), "start has no variable");
    }

    #[test]
    fn every_default_is_a_value() {
        for o in &OPTS {
            assert!(o.default < o.values.len(), "{}", o.key);
            assert!(o.label.len() <= 13, "{} label too long for a switch row", o.key);
        }
    }
}
