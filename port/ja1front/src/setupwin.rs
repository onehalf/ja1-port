//! The window of `ja1front ISO EXE --setup` (setup.rs, config.rs; port option, not in the original). The big lever
//! starts the game as a new process with the chosen start, so every option takes effect from ja1.cfg.

use ja1front::config;
use ja1front::setup::{Act, Setup};
use ja1front::switchbox::Step;

/// The setup screen until Esc (exit 0) or the lever (the game runs, then its exit code).
pub fn run(iso: &str, exe: &str) -> ! {
    let path = config::path();
    let mut bad = vec![];
    let v = config::parse(&std::fs::read_to_string(&path).unwrap_or_default(), &mut bad);
    for b in &bad {
        eprintln!("{}", b);
    }
    let saves = std::path::Path::new("saves");
    let mut s = Setup::new(iso, exe, v, |f| saves.join(f).exists()).unwrap_or_else(|c| super::fail("setup", c));
    if std::env::var_os("JA1_SETUP_WIDTHS").is_some() {
        // Test lines: the pixel width of every row label with every value.
        for line in s.label_widths() {
            println!("{}", line);
        }
        std::process::exit(0);
    }
    if let Ok(png) = std::env::var("JA1_SETUP_PNG") {
        // Test picture: JA1_SETUP_HOVER=i lights row i.
        if let Some(i) = std::env::var("JA1_SETUP_HOVER").ok().and_then(|h| h.parse().ok()) {
            s.hover = Some(i);
            s.draw();
        }
        let pal = ja1front::eod::scale_palette(&s.pal);
        super::write_png_wh(&png, &s.buf, &pal, 320, 200).unwrap_or_else(|m| super::die(format!("{}: {}", png, m)));
        std::process::exit(0);
    }
    // The setup window uses the desktop pointer; the game process gets the environment as it was.
    let mouse_env = std::env::var_os("JA1_MOUSE");
    std::env::set_var("JA1_MOUSE", "desktop");
    let act = window(iso, exe, &mut s, &path);
    if act != Act::Play {
        std::process::exit(0);
    }
    let me = std::env::current_exe().unwrap_or_else(|e| super::die(format!("setup: {}", e)));
    let mut cmd = std::process::Command::new(me);
    cmd.arg(iso).arg(exe).args(s.start_args());
    match mouse_env {
        Some(m) => cmd.env("JA1_MOUSE", m),
        None => cmd.env_remove("JA1_MOUSE"),
    };
    eprintln!("setup start {:?}", s.start_args());
    let code = cmd.status().map(|st| st.code().unwrap_or(1)).unwrap_or_else(|e| super::die(format!("setup: game: {}", e)));
    std::process::exit(code)
}

fn save(s: &mut Setup, path: &std::path::Path) {
    match std::fs::write(path, config::write(&s.v)) {
        Ok(()) => eprintln!("setup saved {}", path.display()),
        Err(e) => {
            eprintln!("setup: {}: {}", path.display(), e);
            s.note = Some(format!("Cannot save {}.", path.display()));
            s.draw();
        }
    }
}

fn window(iso: &str, exe: &str, s: &mut Setup, path: &std::path::Path) -> Act {
    use sdl2::event::Event;
    use sdl2::mouse::MouseButton;
    let mut g = super::gfx("JA1 Setup", iso, exe);
    let tc = g.canvas.texture_creator();
    let mut tex = tc.create_texture_streaming(sdl2::pixels::PixelFormatEnum::RGB24, 320, 200).expect("texture");
    let pal = ja1front::eod::scale_palette(&s.pal);
    let show = |g: &mut super::Gfx, tex: &mut sdl2::render::Texture, img: &[u8]| {
        super::blit(tex, img, &pal, 320, 200);
        g.canvas.copy(tex, None, None).expect("copy");
        g.canvas.present();
    };
    show(&mut g, &mut tex, &s.buf);
    loop {
        let Some(ev) = g.events.wait_event_timeout(100) else {
            show(&mut g, &mut tex, &s.buf);
            continue;
        };
        let mut steps: Vec<Step> = vec![];
        let act = match ev {
            Event::Quit { .. } => Act::Quit,
            Event::MouseMotion { x, y, .. } => {
                let (px, py) = super::screen_pixel(&g.canvas, x, y);
                if s.hover_at(px, py) {
                    steps.push(Step::Show(s.buf.clone(), 0));
                }
                Act::None
            }
            Event::MouseButtonDown { x, y, mouse_btn, .. } if matches!(mouse_btn, MouseButton::Left | MouseButton::Right) => {
                let (px, py) = super::screen_pixel(&g.canvas, x, y);
                s.press(px, py, mouse_btn == MouseButton::Right, &mut steps)
            }
            Event::KeyDown { keycode: Some(k), repeat: false, .. } => s.key(&k.name(), &mut steps),
            _ => Act::None,
        };
        for st in &steps {
            match st {
                Step::Show(img, ms) => {
                    show(&mut g, &mut tex, img);
                    if *ms > 0 {
                        std::thread::sleep(std::time::Duration::from_millis(*ms as u64));
                    }
                }
                Step::Sfx(fx, rate, vol, _) => ja1front::sound::screen(|x| x.sfx(*fx, *rate, *vol, 1)),
                Step::Music(..) => {}
            }
        }
        match act {
            Act::Changed => save(s, path),
            Act::Play => {
                save(s, path);
                // the lever sound ends before the window closes
                std::thread::sleep(std::time::Duration::from_millis(250));
                return Act::Play;
            }
            Act::Quit => return Act::Quit,
            Act::None => {}
        }
    }
}
