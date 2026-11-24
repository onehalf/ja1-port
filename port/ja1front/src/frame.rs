//! The game window keeps the shape of its picture (the maintainer, sticky #1 Inserts 11519 + 11524): 8:5 for the game, on every
//! platform. `Frame` wraps the SDL canvas: the draw sites still write `copy(tex, None, None)` + `present()`, and
//! these two calls do the work. A maximized or full-screen window shows the picture with black bars; mouse positions
//! go through `screen_pixel` / `warp_frame`. No SDL logical size: it would scale mouse events but not `mouse_state`.
use sdl2::rect::Rect;
use sdl2::sys;
use sdl2::render::{Canvas, Texture};
use sdl2::video::{Window, WindowBuilder};
use ja1front::mouselimit::{clamp, Limit, FULL};
use ja1front::dosmouse;

const SW: u32 = 320;
const SH: u32 = 200;

/// Rounding room of `shape_fix`, in `w * sh - h * sw` units of the reduced shape: about one pixel.
fn near_shape(w: u32, h: u32, sw: u32, sh: u32) -> bool {
    (w as i64 * sh as i64 - h as i64 * sw as i64).abs() <= sw as i64
}

fn reduced(sw: u32, sh: u32) -> (u32, u32) {
    let (mut a, mut b) = (sw, sh);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    (sw / a, sh / a)
}

/// The size to ask for after the window got `w` x `h`: same area, the shape `sw`:`sh`. Stateless, because a window
/// manager can apply its own size again at the mouse release, and SDL sends two events per resize. `None`: leave it.
pub fn shape_fix(w: u32, h: u32, sw: u32, sh: u32) -> Option<(u32, u32)> {
    let (sw, sh) = reduced(sw, sh);
    if w == 0 || h == 0 || near_shape(w, h, sw, sh) {
        return None;
    }
    let nw = (((w as f64) * (h as f64) * (sw as f64) / (sh as f64)).sqrt() + 0.5) as u32;
    let nh = (nw * sh + sw / 2) / sw;
    Some((nw, nh)).filter(|&s| s != (w, h))
}

/// The part of a `w` x `h` window that shows a `sw` x `sh` picture (x, y, w, h).
pub fn frame_rect(w: u32, h: u32, sw: u32, sh: u32) -> (i32, i32, u32, u32) {
    let (rw, rh) = reduced(sw, sh);
    if near_shape(w, h, rw, rh) {
        (0, 0, w, h)
    } else if w as u64 * rh as u64 > h as u64 * rw as u64 {
        let fw = (h as u64 * rw as u64 / rh as u64) as u32;
        (((w - fw) / 2) as i32, 0, fw, h)
    } else {
        let fh = (w as u64 * rh as u64 / rw as u64) as u32;
        (0, ((h - fh) / 2) as i32, w, fh)
    }
}

/// Window pixel -> game screen pixel in a `w` x `h` window. A point on a black bar counts as the nearest screen
/// pixel: the mouse of the original cannot leave the screen. (-1, -1) = left or above the window.
pub fn pixel_of(w: u32, h: u32, x: i32, y: i32) -> (i32, i32) {
    if x < 0 || y < 0 || w == 0 || h == 0 {
        return (-1, -1);
    }
    let (fx, fy, fw, fh) = frame_rect(w, h, SW, SH);
    let px = (x - fx) as i64 * SW as i64 / fw.max(1) as i64;
    let py = (y - fy) as i64 * SH as i64 / fh.max(1) as i64;
    let px = if x < w as i32 { px.clamp(0, SW as i64 - 1) } else { px };
    let py = if y < h as i32 { py.clamp(0, SH as i64 - 1) } else { py };
    (px as i32, py as i32)
}

/// `pixel_of` under the mouse limit: what the game reads as the mouse position.
pub fn limited_pixel(w: u32, h: u32, x: i32, y: i32) -> (i32, i32) {
    let (px, py) = pixel_of(w, h, x, y);
    if px < 0 {
        return (px, py);
    }
    clamp(LIMIT.with(|c| c.get()), px, py)
}

/// Game screen pixel -> the window pixel at its middle, so that `pixel_of` gives the same pixel back at any size.
pub fn window_of(w: u32, h: u32, x: i32, y: i32) -> (i32, i32) {
    let (fx, fy, fw, fh) = frame_rect(w, h, SW, SH);
    (fx + ((x as i64 * 2 + 1) * fw as i64 / (2 * SW as i64)) as i32,
     fy + ((y as i64 * 2 + 1) * fh as i64 / (2 * SH as i64)) as i32)
}

thread_local! { static LIMIT: std::cell::Cell<Limit> = const { std::cell::Cell::new(FULL) }; }

/// The mouse limit of the original, 0x682AC. It stays until the next call, as in the mouse driver.
pub fn set_limit(l: Limit) {
    LIMIT.with(|c| c.set(l));
}

pub struct Frame {
    cv: Canvas<Window>,
    shape: (u32, u32),
    /// The last window size that `present` looked at. A size is corrected once: no repeat if the desktop refuses.
    seen: (u32, u32),
    /// The last picture (a render target of the picture size), or null if the renderer has none. Some wait loops
    /// draw only on a change: the event watch below shows this picture again after a resize.
    last: *mut sys::SDL_Texture,
}

impl std::ops::Deref for Frame {
    type Target = Canvas<Window>;
    fn deref(&self) -> &Canvas<Window> {
        &self.cv
    }
}

impl std::ops::DerefMut for Frame {
    fn deref_mut(&mut self) -> &mut Canvas<Window> {
        &mut self.cv
    }
}

/// Shows the last picture in the frame rectangle, black around it. The caller presents.
unsafe fn show_last(r: *mut sys::SDL_Renderer, win: *mut sys::SDL_Window, last: *mut sys::SDL_Texture, shape: (u32, u32)) {
    let (mut w, mut h) = (0, 0);
    sys::SDL_GetWindowSize(win, &mut w, &mut h);
    let (fx, fy, fw, fh) = frame_rect(w as u32, h as u32, shape.0, shape.1);
    if (fw, fh) != (w as u32, h as u32) {
        sys::SDL_RenderClear(r);
    }
    let dst = sys::SDL_Rect { x: fx, y: fy, w: fw.max(1) as i32, h: fh.max(1) as i32 };
    sys::SDL_RenderCopy(r, last, std::ptr::null(), &dst);
    if look().scanlines {
        let rows = scanline_rects(fx, fy, fw, fh, shape.1);
        if !rows.is_empty() {
            sys::SDL_SetRenderDrawBlendMode(r, sys::SDL_BlendMode::SDL_BLENDMODE_BLEND);
            sys::SDL_SetRenderDrawColor(r, 0, 0, 0, 96);
            sys::SDL_RenderFillRects(r, rows.as_ptr(), rows.len() as i32);
            sys::SDL_SetRenderDrawBlendMode(r, sys::SDL_BlendMode::SDL_BLENDMODE_NONE);
            sys::SDL_SetRenderDrawColor(r, 0, 0, 0, 255);
        }
    }
    draw_pointer(r, win);
}

/// The window options of ja1.cfg (config.rs; port options, not in the original), read once from the environment.
pub struct Look {
    /// Window size in picture pixels per game pixel (JA1_WINDOW_SCALE, 1..8, default 4).
    pub scale: u32,
    /// JA1_FULLSCREEN=1: the desktop's full screen.
    pub fullscreen: bool,
    /// JA1_PIXELS=smooth: linear scaling (default sharp, nearest).
    pub smooth: bool,
    /// JA1_SCANLINES=1: a dark band over the lower half of each game pixel row.
    pub scanlines: bool,
    /// JA1_POINTER_SPEED: a factor on the DOS mouse speed (0.25..4, default 1).
    pub pointer: f32,
}

pub fn look() -> &'static Look {
    static L: std::sync::OnceLock<Look> = std::sync::OnceLock::new();
    L.get_or_init(|| {
        let var = |k: &str| std::env::var(k).ok();
        Look {
            scale: var("JA1_WINDOW_SCALE").and_then(|v| v.parse().ok()).filter(|s| (1..=8).contains(s)).unwrap_or(4),
            fullscreen: var("JA1_FULLSCREEN").is_some_and(|v| v == "1"),
            smooth: var("JA1_PIXELS").is_some_and(|v| v == "smooth"),
            scanlines: var("JA1_SCANLINES").is_some_and(|v| v == "1"),
            pointer: var("JA1_POINTER_SPEED").and_then(|v| v.parse().ok()).filter(|f: &f32| (0.25..=4.0).contains(f)).unwrap_or(1.0),
        }
    })
}

/// The scanline bands of a picture `rows` game pixels high in the frame (fx, fy, fw, fh): the lower half of each
/// row. None when a row is less than 2 window pixels high (the bands would hide the picture).
pub fn scanline_rects(fx: i32, fy: i32, fw: u32, fh: u32, rows: u32) -> Vec<sys::SDL_Rect> {
    if rows == 0 || fh < 2 * rows {
        return vec![];
    }
    (0..rows as i64).filter_map(|y| {
        let top = fy as i64 + (2 * y + 1) * fh as i64 / (2 * rows as i64);
        let bottom = fy as i64 + (y + 1) * fh as i64 / rows as i64;
        (bottom > top).then(|| sys::SDL_Rect { x: fx, y: top as i32, w: fw as i32, h: (bottom - top) as i32 })
    }).collect()
}

/// The DOS mouse mode of #64 (captured mouse, half a game pixel per count, the pointer drawn into the picture) is the
/// game window's default; JA1_MOUSE=desktop gives the desktop pointer (the window tests, which move it with XTEST).
pub fn dos_mode() -> bool {
    static M: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *M.get_or_init(|| std::env::var("JA1_MOUSE").map_or(true, |v| v != "desktop")) || DOS_TEST.load(std::sync::atomic::Ordering::Relaxed)
}

/// The game window starts the DOS mouse mode (after its pointer shapes): the mouse is captured.
pub fn dos_start() {
    if !dos_mode() {
        return;
    }
    let Some((_, win, ..)) = SHOWN.with(|s| s.get()) else { return };
    unsafe {
        sys::SDL_AddEventWatch(Some(dos_watch), std::ptr::null_mut());
        dos_capture(win, true);
    }
}

/// Unit tests turn the DOS mouse mode on without the environment.
static DOS_TEST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

struct DosPtr {
    tex: *mut sys::SDL_Texture,
    /// The shape's colour indexes (0 = not drawn); the texture gets the colours of the shown palette.
    pix: Vec<u8>,
    w: i32,
    h: i32,
    hx: i32,
    hy: i32,
}

thread_local! {
    static DOS_PTRS: std::cell::RefCell<Vec<DosPtr>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The shape index and the pointer on or off, as the game asks.
    static DOS_CUR: std::cell::Cell<(usize, bool)> = const { std::cell::Cell::new((0, true)) };
    static DOS_DRAWN: std::cell::Cell<Option<dosmouse::Drawn>> = const { std::cell::Cell::new(None) };
    static DOS_CAPTURED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// A capture change that an event asked for; `Frame::present` does it, outside the event pump.
    static DOS_WANT: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    /// The palette of the shown picture, and true when the pointer textures do not have its colours yet.
    static DOS_PAL: std::cell::Cell<([u8; 768], bool)> = const { std::cell::Cell::new(([0; 768], true)) };
}

/// The palette of the picture that goes to the screen: the DOS pointer is drawn with it (MousePoll 0x678E8 draws
/// into the 8-bit picture).
pub fn dos_palette(pal: &[u8; 768]) {
    if !dos_mode() {
        return;
    }
    DOS_PAL.with(|c| {
        let (old, dirty) = c.get();
        if dirty || old != *pal {
            c.set((*pal, true));
        }
    });
}

/// True while the DOS mouse mode holds the mouse (the desktop pointer is off).
pub fn dos_captured() -> bool {
    DOS_CAPTURED.with(|c| c.get())
}

/// The pointer shapes of the DOS mouse mode (RGBA, width, height, hot spot), at game size.
/// The DOS pointer shapes: (colour indexes, w, h, hot spot x, y); `dos_palette` gives their colours.
pub fn dos_pointers(list: Vec<(Vec<u8>, u32, u32, i32, i32)>) {
    let Some((r, ..)) = SHOWN.with(|s| s.get()) else { return };
    let ptrs = list.into_iter().filter_map(|(pix, w, h, hx, hy)| unsafe {
        let t = sys::SDL_CreateTexture(r, sys::SDL_PixelFormatEnum::SDL_PIXELFORMAT_ABGR8888 as u32,
                                       sys::SDL_TextureAccess::SDL_TEXTUREACCESS_STATIC as i32, w as i32, h as i32);
        if t.is_null() {
            return None;
        }
        sys::SDL_SetTextureBlendMode(t, sys::SDL_BlendMode::SDL_BLENDMODE_BLEND);
        Some(DosPtr { tex: t, pix, w: w as i32, h: h as i32, hx, hy })
    }).collect();
    DOS_PTRS.with(|p| *p.borrow_mut() = ptrs);
    DOS_PAL.with(|c| c.set((c.get().0, true)));
}

/// The pointer textures get the colours of the shown palette after a palette change.
unsafe fn pointer_colours() {
    let (pal, dirty) = DOS_PAL.with(|c| c.get());
    if !dirty {
        return;
    }
    DOS_PAL.with(|c| c.set((pal, false)));
    DOS_PTRS.with(|p| {
        for d in p.borrow().iter() {
            let px = dosmouse::pointer_rgba(&d.pix, &pal);
            sys::SDL_UpdateTexture(d.tex, std::ptr::null(), px.as_ptr() as *const _, d.w * 4);
        }
    });
}

/// The game sets the pointer shape (`shape`) or turns it on or off (`on`); the picture shows it at once.
pub fn dos_pointer(shape: Option<usize>, on: Option<bool>) {
    if !dos_mode() {
        return;
    }
    DOS_CUR.with(|c| {
        let (s0, o0) = c.get();
        c.set((shape.unwrap_or(s0), on.unwrap_or(o0)));
    });
    unsafe { redraw_if_moved() };
}

/// The pointer's game pixel, shape and on/off now.
unsafe fn pointer_now(win: *mut sys::SDL_Window) -> dosmouse::Drawn {
    let (mut w, mut h, mut x, mut y) = (0, 0, 0, 0);
    sys::SDL_GetWindowSize(win, &mut w, &mut h);
    sys::SDL_GetMouseState(&mut x, &mut y);
    let (gx, gy) = pixel_of(w as u32, h as u32, x, y);
    let (k, on) = DOS_CUR.with(|c| c.get());
    (gx, gy, k, on && dos_captured())
}

/// MousePoll 0x678E8: the pointer is drawn into the picture at its game pixel (game size, scaled with the picture).
unsafe fn draw_pointer(r: *mut sys::SDL_Renderer, win: *mut sys::SDL_Window) {
    if !dos_mode() {
        return;
    }
    let now = pointer_now(win);
    DOS_DRAWN.with(|d| d.set(Some(now)));
    record_pointer(now);
    let (gx, gy, k, on) = now;
    if !on {
        return;
    }
    pointer_colours();
    let (mut w, mut h) = (0, 0);
    sys::SDL_GetWindowSize(win, &mut w, &mut h);
    let (fx, fy, fw, fh) = frame_rect(w as u32, h as u32, SW, SH);
    DOS_PTRS.with(|p| {
        let p = p.borrow();
        let Some(d) = p.get(k).or(p.first()) else { return };
        let sx = |v: i32| v * fw as i32 / SW as i32;
        let sy = |v: i32| v * fh as i32 / SH as i32;
        let dst = sys::SDL_Rect { x: fx + sx(gx - d.hx), y: fy + sy(gy - d.hy), w: sx(d.w).max(1), h: sy(d.h).max(1) };
        let clip = sys::SDL_Rect { x: fx, y: fy, w: fw.max(1) as i32, h: fh.max(1) as i32 };
        sys::SDL_RenderSetClipRect(r, &clip);
        sys::SDL_RenderCopy(r, d.tex, std::ptr::null(), &dst);
        sys::SDL_RenderSetClipRect(r, std::ptr::null());
    });
}

/// `JA1_FRAMES=PATH`: each change of the drawn pointer goes as one pointer line to PATH.pointer (#64 step 2), so the
/// recorded frames plus these lines give the pictures with the pointer, also for a pointer-only redraw.
fn record_pointer(now: dosmouse::Drawn) {
    use std::io::Write;
    static OUT: std::sync::Mutex<(Option<Option<std::fs::File>>, Option<dosmouse::Drawn>)> = std::sync::Mutex::new((None, None));
    let mut g = OUT.lock().unwrap_or_else(|e| e.into_inner());
    let (out, last) = &mut *g;
    let out = out.get_or_insert_with(|| {
        let path = std::env::var("JA1_FRAMES").ok()?;
        std::fs::File::create(format!("{}.pointer", path)).ok()
    });
    let Some(f) = out else { return };
    if let Some(line) = dosmouse::pointer_record(last, record_ms(), now) {
        f.write_all(line.as_bytes()).ok();
    }
}

/// The last picture again with the pointer, when the pointer's place, shape or on/off changed.
unsafe fn redraw_if_moved() {
    let Some((r, win, last, shape)) = SHOWN.with(|s| s.get()) else { return };
    if dosmouse::needs_draw(DOS_DRAWN.with(|d| d.get()), pointer_now(win)) {
        show_last(r, win, last, shape);
        sys::SDL_RenderPresent(r);
    }
}

/// The DOS mouse mode's events: the edge pin, Ctrl+F10 (release), a click (capture again), the pointer at a move.
unsafe extern "C" fn dos_watch(_: *mut std::ffi::c_void, ev: *mut sys::SDL_Event) -> i32 {
    use sys::SDL_EventType as T;
    let t = (*ev).type_;
    if t == T::SDL_KEYDOWN as u32 {
        let k = (*ev).key.keysym;
        let ctrl = k.mod_ & (sys::SDL_Keymod::KMOD_LCTRL as u16 | sys::SDL_Keymod::KMOD_RCTRL as u16) != 0;
        if dos_captured() && dosmouse::release_key(k.sym == sys::SDL_KeyCode::SDLK_F10 as i32, ctrl) {
            DOS_WANT.with(|c| c.set(Some(false)));
        }
    } else if t == T::SDL_MOUSEBUTTONDOWN as u32 && !dos_captured() {
        DOS_WANT.with(|c| c.set(Some(true)));
    } else if t == T::SDL_MOUSEMOTION as u32 && dos_captured() {
        let m = (*ev).motion;
        let win = sys::SDL_GetWindowFromID(m.windowID);
        if !win.is_null() {
            let (mut w, mut h) = (0, 0);
            sys::SDL_GetWindowSize(win, &mut w, &mut h);
            let (px, py) = dosmouse::pin(frame_rect(w as u32, h as u32, SW, SH), m.x, m.y);
            if (px, py) != (m.x, m.y) {
                sys::SDL_WarpMouseInWindow(win, px, py);
                return 1;
            }
        }
        redraw_if_moved();
    }
    1
}

/// Captures the mouse (relative mode, half a game pixel per count) or gives it back to the desktop.
unsafe fn dos_capture(win: *mut sys::SDL_Window, on: bool) {
    let (mut w, mut h) = (0, 0);
    sys::SDL_GetWindowSize(win, &mut w, &mut h);
    let (_, _, fw, _) = frame_rect(w as u32, h as u32, SW, SH);
    let scale = std::ffi::CString::new(format!("{}", dosmouse::speed_scale(fw, SW) * look().pointer)).unwrap_or_default();
    sys::SDL_SetHint(c"SDL_MOUSE_RELATIVE_SPEED_SCALE".as_ptr(), scale.as_ptr());
    if sys::SDL_SetRelativeMouseMode(if on { sys::SDL_bool::SDL_TRUE } else { sys::SDL_bool::SDL_FALSE }) == 0 {
        DOS_CAPTURED.with(|c| c.set(on));
        eprintln!("dos mouse {}", if on { "captured" } else { "released" });
    }
    sys::SDL_ShowCursor((!on) as i32);
}

type Shown = (*mut sys::SDL_Renderer, *mut sys::SDL_Window, *mut sys::SDL_Texture, (u32, u32));
thread_local! { static SHOWN: std::cell::Cell<Option<Shown>> = const { std::cell::Cell::new(None) }; }

/// A resize or an uncovered window shows the last picture at once, also in a wait loop that draws only on a change.
/// Only the picture: `Frame::present` does the shape correction, outside the event pump.
unsafe extern "C" fn watch(_: *mut std::ffi::c_void, ev: *mut sys::SDL_Event) -> i32 {
    if (*ev).type_ == sys::SDL_EventType::SDL_WINDOWEVENT as u32 {
        let e = (*ev).window.event as u32;
        if e == sys::SDL_WindowEventID::SDL_WINDOWEVENT_SIZE_CHANGED as u32
            || e == sys::SDL_WindowEventID::SDL_WINDOWEVENT_EXPOSED as u32 {
            if let Some((r, win, last, shape)) = SHOWN.with(|s| s.get()) {
                show_last(r, win, last, shape);
                sys::SDL_RenderPresent(r);
            }
        }
    }
    1
}

/// The clock of the `JA1_FRAMES` record: the frames and the input lines count their ms from the same start.
pub fn record_ms() -> u32 {
    static T0: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    T0.get_or_init(std::time::Instant::now).elapsed().as_millis() as u32
}

/// One input line of the `JA1_FRAMES` record (#64 step 2): ms on the frame clock, the game pixel x y (-1 = off the
/// picture), the buttons after the event (1 left, 2 right, 4 middle), the relative move dx dy of the event.
pub fn input_line(ms: u32, (x, y): (i32, i32), buttons: u32, (dx, dy): (i32, i32)) -> String {
    format!("{} {} {} {} {} {}\n", ms, x, y, buttons, dx, dy)
}

/// The SDL button state bits as the input line writes them: 1 left, 2 right, 4 middle.
pub fn input_buttons(sdl_state: u32) -> u32 {
    (sdl_state & 1) | ((sdl_state >> 1) & 2) | ((sdl_state << 1) & 4)
}

/// `JA1_FRAMES=PATH`: every mouse event goes as one input line to PATH.input (the frames stay in PATH).
unsafe extern "C" fn input_watch(_: *mut std::ffi::c_void, ev: *mut sys::SDL_Event) -> i32 {
    use std::io::Write;
    use sys::SDL_EventType as T;
    static OUT: std::sync::Mutex<Option<Option<std::fs::File>>> = std::sync::Mutex::new(None);
    let t = (*ev).type_;
    let (wid, x, y, state, rel) = if t == T::SDL_MOUSEMOTION as u32 {
        let m = (*ev).motion;
        (m.windowID, m.x, m.y, m.state, (m.xrel, m.yrel))
    } else if t == T::SDL_MOUSEBUTTONDOWN as u32 || t == T::SDL_MOUSEBUTTONUP as u32 {
        let b = (*ev).button;
        // The state after the event: the other buttons from SDL, this button from the event itself.
        let bit = 1u32 << (b.button as u32).saturating_sub(1).min(31);
        let mut st = sys::SDL_GetMouseState(std::ptr::null_mut(), std::ptr::null_mut()) & !bit;
        if t == T::SDL_MOUSEBUTTONDOWN as u32 {
            st |= bit;
        }
        (b.windowID, b.x, b.y, st, (0, 0))
    } else {
        return 1;
    };
    let win = sys::SDL_GetWindowFromID(wid);
    let (mut w, mut h) = (0, 0);
    if !win.is_null() {
        sys::SDL_GetWindowSize(win, &mut w, &mut h);
    }
    let line = input_line(record_ms(), pixel_of(w as u32, h as u32, x, y), input_buttons(state), rel);
    let mut g = OUT.lock().unwrap_or_else(|e| e.into_inner());
    let out = g.get_or_insert_with(|| {
        let path = std::env::var("JA1_FRAMES").ok()?;
        std::fs::File::create(format!("{}.input", path)).ok()
    });
    if let Some(f) = out {
        f.write_all(line.as_bytes()).ok();
    }
    1
}

impl Frame {
    /// A resizable window of a `sw` x `sh` picture. The X11 hint comes after the renderer, because SDL can make the
    /// X window again for its renderer.
    pub fn new(b: &mut WindowBuilder, sw: u32, sh: u32) -> Frame {
        b.resizable();
        let window = b.build().expect("window");
        let size = window.size();
        let mut cv = window.into_canvas().build().expect("canvas");
        cv.window_mut().set_minimum_size(sw, sh).ok();
        cv.set_draw_color(sdl2::pixels::Color::RGB(0, 0, 0));
        let (rw, rh) = reduced(sw, sh);
        let hint = aspect_hint(cv.window(), rw, rh);
        eprintln!("window aspect hint {}", if hint { "set" } else { "off" });
        let last = unsafe {
            let t = sys::SDL_CreateTexture(cv.raw(), sys::SDL_PixelFormatEnum::SDL_PIXELFORMAT_RGB888 as u32,
                                           sys::SDL_TextureAccess::SDL_TEXTUREACCESS_TARGET as i32, sw as i32, sh as i32);
            if !t.is_null() && sys::SDL_SetRenderTarget(cv.raw(), t) == 0 {
                sys::SDL_RenderClear(cv.raw());
                sys::SDL_SetRenderTarget(cv.raw(), std::ptr::null_mut());
                SHOWN.with(|s| s.set(Some((cv.raw(), cv.window().raw(), t, (sw, sh)))));
                sys::SDL_AddEventWatch(Some(watch), std::ptr::null_mut());
                t
            } else {
                std::ptr::null_mut()
            }
        };
        if std::env::var_os("JA1_FRAMES").is_some() {
            unsafe { sys::SDL_AddEventWatch(Some(input_watch), std::ptr::null_mut()) };
        }
        Frame { cv, shape: (sw, sh), seen: size, last }
    }

    /// Every draw site shows a whole picture: it fills the frame rectangle, and the rest of the window is black.
    pub fn copy(&mut self, tex: &Texture, src: Option<Rect>, dst: Option<Rect>) -> Result<(), String> {
        debug_assert!(src.is_none() && dst.is_none());
        if !self.last.is_null() {
            unsafe {
                let r = self.cv.raw();
                sys::SDL_SetRenderTarget(r, self.last);
                sys::SDL_RenderCopy(r, tex.raw(), std::ptr::null(), std::ptr::null());
                sys::SDL_SetRenderTarget(r, std::ptr::null_mut());
                show_last(r, self.cv.window().raw(), self.last, self.shape);
            }
            return Ok(());
        }
        let (w, h) = self.cv.window().size();
        let (fx, fy, fw, fh) = frame_rect(w, h, self.shape.0, self.shape.1);
        if (fw, fh) != (w, h) {
            self.cv.clear();
        }
        self.cv.copy(tex, None, Rect::new(fx, fy, fw.max(1), fh.max(1)))
    }

    /// The desktop pointer goes back into the mouse limit, as the driver of the original keeps it there.
    fn hold_mouse(&mut self) {
        let l = LIMIT.with(|c| c.get());
        if l == FULL {
            return;
        }
        let win = self.cv.window();
        if !has_input_focus(win.window_flags()) {
            return;
        }
        let (mut x, mut y) = (0, 0);
        unsafe {
            if sys::SDL_GetMouseFocus() != win.raw() {
                return;
            }
            sys::SDL_GetMouseState(&mut x, &mut y);
        }
        let (w, h) = win.size();
        let (px, py) = pixel_of(w, h, x, y);
        let (cx, cy) = clamp(l, px, py);
        if (cx, cy) != (px, py) {
            let (wx, wy) = window_of(w, h, cx, cy);
            unsafe { sys::SDL_WarpMouseInWindow(win.raw(), wx, wy) };
        }
    }

    /// Shows the frame, then corrects a window of a wrong shape that the desktop gave (the hint keeps most drags right).
    pub fn present(&mut self) {
        if dos_mode() {
            if let Some(on) = DOS_WANT.with(|c| c.take()) {
                unsafe { dos_capture(self.cv.window().raw(), on) };
            }
        }
        self.cv.present();
        self.hold_mouse();
        let size = self.cv.window().size();
        if size == self.seen {
            return;
        }
        self.seen = size;
        if dos_captured() {
            unsafe { dos_capture(self.cv.window().raw(), true) };
        }
        use sys::SDL_WindowFlags as F;
        let flags = self.cv.window().window_flags();
        if flags & (F::SDL_WINDOW_MAXIMIZED as u32 | F::SDL_WINDOW_FULLSCREEN as u32) != 0 {
            return;
        }
        if let Some((w, h)) = shape_fix(size.0, size.1, self.shape.0, self.shape.1) {
            eprintln!("window shape set {} {}", w, h);
            self.cv.window_mut().set_size(w, h).ok();
        }
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        // The renderer frees the texture.
        SHOWN.with(|s| s.set(None));
        unsafe { sys::SDL_DelEventWatch(Some(watch), std::ptr::null_mut()) };
        unsafe { sys::SDL_DelEventWatch(Some(input_watch), std::ptr::null_mut()) };
        unsafe { sys::SDL_DelEventWatch(Some(dos_watch), std::ptr::null_mut()) };
    }
}

/// X11 only: asks the window manager for a fixed shape (`WM_NORMAL_HINTS`, PAspect). libX11 comes through SDL's loader:
/// the program must start with no libX11. Any failure leaves the hint out, and `Frame::present` still corrects.
/// JA1_NO_ASPECT_HINT=1 (tests): the correction alone.
#[cfg(target_os = "linux")]
fn aspect_hint(window: &Window, rw: u32, rh: u32) -> bool {
    use sys as s;
    use std::ffi::c_void;
    #[repr(C)]
    #[derive(Default)]
    struct SizeHints {
        flags: std::ffi::c_long,
        geometry: [i32; 4],
        min_size: [i32; 2],
        max_size: [i32; 2],
        inc: [i32; 2],
        min_aspect: [i32; 2],
        max_aspect: [i32; 2],
        base: [i32; 2],
        win_gravity: i32,
    }
    const P_ASPECT: std::ffi::c_long = 1 << 7;
    type Get = unsafe extern "C" fn(*mut c_void, std::ffi::c_ulong, *mut SizeHints, *mut std::ffi::c_long) -> i32;
    type Set = unsafe extern "C" fn(*mut c_void, std::ffi::c_ulong, *const SizeHints) -> i32;
    type Flush = unsafe extern "C" fn(*mut c_void) -> i32;
    if std::env::var_os("JA1_NO_ASPECT_HINT").is_some() {
        return false;
    }
    unsafe {
        let mut info: s::SDL_SysWMinfo = std::mem::zeroed();
        s::SDL_GetVersion(&mut info.version);
        if s::SDL_GetWindowWMInfo(window.raw(), &mut info) != s::SDL_bool::SDL_TRUE
            || info.subsystem != s::SDL_SYSWM_TYPE::SDL_SYSWM_X11 {
            return false;
        }
        let lib = s::SDL_LoadObject(c"libX11.so.6".as_ptr());
        if lib.is_null() {
            return false;
        }
        let (get, set, flush) = (s::SDL_LoadFunction(lib, c"XGetWMNormalHints".as_ptr()),
                                 s::SDL_LoadFunction(lib, c"XSetWMNormalHints".as_ptr()),
                                 s::SDL_LoadFunction(lib, c"XFlush".as_ptr()));
        if get.is_null() || set.is_null() || flush.is_null() {
            return false;
        }
        let (get, set, flush): (Get, Set, Flush) = (std::mem::transmute(get), std::mem::transmute(set), std::mem::transmute(flush));
        let (dpy, win) = (info.info.x11.display as *mut c_void, info.info.x11.window as std::ffi::c_ulong);
        let (mut h, mut supplied) = (SizeHints::default(), 0);
        if get(dpy, win, &mut h, &mut supplied) == 0 {
            h = SizeHints::default();
        }
        h.flags |= P_ASPECT;
        h.min_aspect = [rw as i32, rh as i32];
        h.max_aspect = [rw as i32, rh as i32];
        set(dpy, win, &h);
        flush(dpy);
    }
    true
}

#[cfg(not(target_os = "linux"))]
fn aspect_hint(_: &Window, _: u32, _: u32) -> bool {
    false
}

/// The keyboard interrupt handler 0x74CD5 queues no key for a shift, Ctrl, Alt or lock key alone: their table entries are 0
/// (0x9B6B2, scans 0x1D, 0x2A, 0x36, 0x38, 0x3A, 0x45, 0x46), so a wait for "any key" goes on. The Windows and menu keys have no
/// DOS code here. SDL still tracks the modifier state: only the key-down event goes.
pub fn is_lone_modifier(sym: i32) -> bool {
    use sys::SDL_KeyCode::*;
    [SDLK_LSHIFT, SDLK_RSHIFT, SDLK_LCTRL, SDLK_RCTRL, SDLK_LALT, SDLK_RALT, SDLK_CAPSLOCK, SDLK_NUMLOCKCLEAR, SDLK_SCROLLLOCK,
     SDLK_LGUI, SDLK_RGUI, SDLK_MODE, SDLK_APPLICATION]
        .iter()
        .any(|&k| k as i32 == sym)
}

/// The key Ctrl+[ with no Alt.
fn ctrl_bracket(sym: i32, keymod: u16) -> bool {
    let (ctrl, alt) = ((sys::SDL_Keymod::KMOD_LCTRL as u16) | (sys::SDL_Keymod::KMOD_RCTRL as u16), (sys::SDL_Keymod::KMOD_LALT as u16) | (sys::SDL_Keymod::KMOD_RALT as u16));
    sym == b'[' as i32 && keymod & ctrl != 0 && keymod & alt == 0
}

/// The upper case letter of a Ctrl+letter key with no Alt.
fn ctrl_letter(sym: i32, keymod: u16) -> Option<u8> {
    let (ctrl, alt) = ((sys::SDL_Keymod::KMOD_LCTRL as u16) | (sys::SDL_Keymod::KMOD_RCTRL as u16), (sys::SDL_Keymod::KMOD_LALT as u16) | (sys::SDL_Keymod::KMOD_RALT as u16));
    ((b'a' as i32..=b'z' as i32).contains(&sym) && keymod & ctrl != 0 && keymod & alt == 0).then(|| (sym as u8).to_ascii_uppercase())
}

/// The character of a Ctrl key that keeps its plain character in the original (live, issue #31): digits but 6, and these signs.
/// Ctrl+6 and Ctrl+- give control bytes, Ctrl+= gives none, Ctrl+Shift gives none. X11 changes such text input, so the
/// window takes it from the key.
pub fn ctrl_plain_char(sym: i32, keymod: u16) -> Option<u8> {
    let (ctrl, other) = ((sys::SDL_Keymod::KMOD_LCTRL as u16) | (sys::SDL_Keymod::KMOD_RCTRL as u16), (sys::SDL_Keymod::KMOD_LALT as u16) | (sys::SDL_Keymod::KMOD_RALT as u16) | (sys::SDL_Keymod::KMOD_LSHIFT as u16) | (sys::SDL_Keymod::KMOD_RSHIFT as u16));
    let c = u8::try_from(sym).ok()?;
    (keymod & ctrl != 0 && keymod & other == 0 && matches!(c, b'0'..=b'5' | b'7'..=b'9' | b' ' | b',' | b'.' | b'/' | b';' | b'\'')).then_some(c)
}

/// The bytes of a key that the save name line refuses (live in the original, issue #31): the key reader of `saveslots` gives 0
/// for it, and the next key goes to the raw read and types nothing. Empty for the other keys.
pub fn name_line_refused(sym: i32, keymod: u16) -> &'static [u8] {
    let ctrl = (sys::SDL_Keymod::KMOD_LCTRL as u16) | (sys::SDL_Keymod::KMOD_RCTRL as u16);
    let lalt = sys::SDL_Keymod::KMOD_LALT as u16;
    if sym == sys::SDL_KeyCode::SDLK_TAB as i32 {
        return &[9];
    }
    const SCAN: [(sys::SDL_KeyCode, u8); 18] = [
        (sys::SDL_KeyCode::SDLK_F1, 0x3B), (sys::SDL_KeyCode::SDLK_F2, 0x3C), (sys::SDL_KeyCode::SDLK_F3, 0x3D),
        (sys::SDL_KeyCode::SDLK_F4, 0x3E), (sys::SDL_KeyCode::SDLK_F5, 0x3F), (sys::SDL_KeyCode::SDLK_F6, 0x40),
        (sys::SDL_KeyCode::SDLK_F7, 0x41), (sys::SDL_KeyCode::SDLK_F8, 0x42), (sys::SDL_KeyCode::SDLK_F9, 0x43),
        (sys::SDL_KeyCode::SDLK_F10, 0x44), (sys::SDL_KeyCode::SDLK_F11, 0x85), (sys::SDL_KeyCode::SDLK_F12, 0x86),
        (sys::SDL_KeyCode::SDLK_UP, 0x48), (sys::SDL_KeyCode::SDLK_DOWN, 0x50), (sys::SDL_KeyCode::SDLK_PAGEUP, 0x49),
        (sys::SDL_KeyCode::SDLK_PAGEDOWN, 0x51), (sys::SDL_KeyCode::SDLK_INSERT, 0x52), (sys::SDL_KeyCode::SDLK_KP_5, 0x4C),
    ];
    const BYTES: [[u8; 2]; 18] = {
        let mut b = [[0u8; 2]; 18];
        let mut i = 0;
        while i < 18 {
            b[i][1] = SCAN[i].1;
            i += 1;
        }
        b
    };
    static STORE: [[u8; 2]; 18] = BYTES;
    if let Some(i) = SCAN.iter().position(|&(k, _)| k as i32 == sym) {
        return &STORE[i];
    }
    let Ok(c) = u8::try_from(sym) else { return &[] };
    if keymod & lalt != 0 && (0x20..0x7F).contains(&c) {
        return &[0, 0x10];
    }
    let shift = (sys::SDL_Keymod::KMOD_LSHIFT as u16) | (sys::SDL_Keymod::KMOD_RSHIFT as u16);
    if keymod & ctrl != 0 && keymod & shift == 0 && matches!(c, b'6' | b'-' | b'=' | b'`' | b'\\' | b']') {
        return &[0x1F];
    }
    &[]
}

/// The BIOS byte (1..26) of the last Ctrl+letter key: `key_filter` turns that key into F24, so that no screen reads it as
/// its letter. Readers of DOS key bytes take the byte from here.
pub static CTRL_LETTER: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

unsafe extern "C" fn key_filter(_: *mut std::os::raw::c_void, ev: *mut sys::SDL_Event) -> std::os::raw::c_int {
    if (*ev).type_ == sys::SDL_EventType::SDL_KEYDOWN as u32 && is_lone_modifier((*ev).key.keysym.sym) {
        return 0;
    }
    if (*ev).type_ == sys::SDL_EventType::SDL_TEXTINPUT as u32 {
        // The name line takes a Ctrl key from `ctrl_plain_char`; AltGr (Ctrl + Alt on some systems) keeps its text.
        let m = sys::SDL_GetModState() as u16;
        let (ctrl, alt) = ((sys::SDL_Keymod::KMOD_LCTRL as u16) | (sys::SDL_Keymod::KMOD_RCTRL as u16), (sys::SDL_Keymod::KMOD_LALT as u16) | (sys::SDL_Keymod::KMOD_RALT as u16));
        // Ctrl + Shift + sign is the shifted sign (live, issue #31).
        let shift = (sys::SDL_Keymod::KMOD_LSHIFT as u16) | (sys::SDL_Keymod::KMOD_RSHIFT as u16);
        if m & ctrl != 0 && m & alt == 0 && m & shift == 0 {
            return 0;
        }
        // Left Alt + key is an extended key on the name line (`name_line_refused`), not its text.
        if m & (sys::SDL_Keymod::KMOD_LALT as u16) != 0 {
            return 0;
        }
    }
    if (*ev).type_ == sys::SDL_EventType::SDL_KEYDOWN as u32 {
        // BIOS: Ctrl+[ gives byte 27, the byte of Escape, on every screen (live in the original, issue #31).
        let k = &mut (*ev).key.keysym;
        if ctrl_bracket(k.sym, k.mod_) {
            k.sym = sys::SDL_KeyCode::SDLK_ESCAPE as i32;
            k.mod_ = 0;
        } else if let Some(c) = ctrl_letter(k.sym, k.mod_) {
            // BIOS: Ctrl+letter gives byte 1..26, not the letter. Ctrl+M is the byte of Return, Ctrl+H of Backspace and
            // Ctrl+I of Tab on every screen; the other bytes answer no box (live in the original, issue #31).
            match c {
                b'M' | b'H' | b'I' => {
                    k.sym = (if c == b'M' { sys::SDL_KeyCode::SDLK_RETURN } else if c == b'H' { sys::SDL_KeyCode::SDLK_BACKSPACE } else { sys::SDL_KeyCode::SDLK_TAB }) as i32;
                    k.mod_ = 0;
                }
                _ => {
                    CTRL_LETTER.store(c - b'A' + 1, std::sync::atomic::Ordering::Relaxed);
                    k.sym = sys::SDL_KeyCode::SDLK_F24 as i32;
                }
            }
        }
    }
    hold_still(ev)
}

/// PressPanelButton? 0x35D44 and the other button holds: the move does not count and the pointer stands still (the original
/// polls no pointer during MouseQueueNext? 0x6782C). A move during a hold goes back to the press; the left button's release ends it.
unsafe fn hold_still(ev: *mut sys::SDL_Event) -> std::os::raw::c_int {
    use std::sync::atomic::Ordering::Relaxed;
    let hold = ja1front::pointer::HOLD_AT.load(Relaxed);
    if hold < 0 {
        return 1;
    }
    let t = (*ev).type_;
    if t == sys::SDL_EventType::SDL_MOUSEBUTTONUP as u32 && (*ev).button.button == 1 {
        ja1front::pointer::HOLD_AT.store(-1, Relaxed);
    } else if t == sys::SDL_EventType::SDL_MOUSEMOTION as u32 {
        let m = (*ev).motion;
        if m.state & 1 == 0 {
            ja1front::pointer::HOLD_AT.store(-1, Relaxed);
            return 1;
        }
        let win = sys::SDL_GetWindowFromID(m.windowID);
        if !win.is_null() {
            let (mut w, mut h) = (0, 0);
            sys::SDL_GetWindowSize(win, &mut w, &mut h);
            let (wx, wy) = window_of(w as u32, h as u32, (hold >> 16) as i32, (hold & 0xFFFF) as i32);
            if (m.x, m.y) != (wx, wy) {
                sys::SDL_WarpMouseInWindow(win, wx, wy);
                return 0;
            }
        }
    }
    1
}

/// Call once after SDL starts: the event queue drops the key-down of a lone modifier.
pub fn drop_lone_modifiers() {
    unsafe { sys::SDL_SetEventFilter(Some(key_filter), std::ptr::null_mut()) };
}

/// The window holds the desktop pointer only while it has the keyboard focus (another window's pointer stays free).
fn has_input_focus(flags: u32) -> bool {
    flags & (sys::SDL_WindowFlags::SDL_WINDOW_INPUT_FOCUS as u32) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ledger row d057aca:ja1front:23749: the pointer hold runs only with the input focus flag, whatever other window
    /// flags are set.
    #[test]
    fn the_pointer_hold_needs_the_input_focus() {
        use sys::SDL_WindowFlags as F;
        let (focus, shown) = (F::SDL_WINDOW_INPUT_FOCUS as u32, F::SDL_WINDOW_SHOWN as u32);
        assert!(has_input_focus(focus | shown) && has_input_focus(focus));
        assert!(!has_input_focus(shown) && !has_input_focus(0));
    }

    /// Ledger row c2aea50:ja1front:23443: the mouse limit of 0x682AC holds the game's mouse inside its box; a point left
    /// of the window stays (-1, -1).
    #[test]
    fn the_mouse_limit_holds_the_mouse() {
        let (px, py) = pixel_of(640, 400, 600, 380);
        assert!(px > 100 && py > 50);
        set_limit((0, 0, 100, 50));
        let got = (limited_pixel(640, 400, 600, 380), limited_pixel(640, 400, -3, 10));
        set_limit(FULL);
        assert_eq!(got, ((100, 50), (-1, -1)));
    }

    #[test]
    fn name_line_refuses_the_live_list() {
        let c = sys::SDL_Keymod::KMOD_LCTRL as u16;
        for k in *b"6-=`\\]" {
            assert_eq!(name_line_refused(k as i32, c), &[0x1F]);
            assert!(name_line_refused(k as i32, 0).is_empty());
            assert!(name_line_refused(k as i32, c | sys::SDL_Keymod::KMOD_LSHIFT as u16).is_empty());
        }
        assert_eq!(name_line_refused(sys::SDL_KeyCode::SDLK_TAB as i32, 0), &[9]);
        assert_eq!(name_line_refused(sys::SDL_KeyCode::SDLK_F1 as i32, 0), &[0, 0x3B]);
        assert_eq!(name_line_refused(sys::SDL_KeyCode::SDLK_UP as i32, 0), &[0, 0x48]);
        assert_eq!(name_line_refused(b'q' as i32, sys::SDL_Keymod::KMOD_LALT as u16), &[0, 0x10]);
        assert!(name_line_refused(b'q' as i32, sys::SDL_Keymod::KMOD_RALT as u16).is_empty());
        assert!(name_line_refused(b'q' as i32, 0).is_empty());
    }

    #[test]
    fn ctrl_plain_char_follows_the_live_list() {
        let c = sys::SDL_Keymod::KMOD_LCTRL as u16;
        let s = sys::SDL_Keymod::KMOD_LSHIFT as u16;
        for k in *b"012345789 ,./;'" {
            assert_eq!(ctrl_plain_char(k as i32, c), Some(k));
            assert_eq!(ctrl_plain_char(k as i32, 0), None);
            assert_eq!(ctrl_plain_char(k as i32, c | s), None);
        }
        for k in *b"6-=`a" {
            assert_eq!(ctrl_plain_char(k as i32, c), None);
        }
        assert_eq!(ctrl_plain_char(0x4000_0050, c), None);
    }

    #[test]
    fn ctrl_letter_only_with_ctrl_and_no_alt() {
        let (c, a) = (sys::SDL_Keymod::KMOD_LCTRL as u16, sys::SDL_Keymod::KMOD_LALT as u16);
        assert_eq!(ctrl_letter(b'y' as i32, c), Some(b'Y'));
        assert_eq!(ctrl_letter(b'y' as i32, 0), None);
        assert_eq!(ctrl_letter(b'y' as i32, c | a), None);
        assert_eq!(ctrl_letter(b'1' as i32, c), None);
    }

    #[test]
    fn ctrl_bracket_is_escape() {
        let (c, a) = (sys::SDL_Keymod::KMOD_LCTRL as u16, sys::SDL_Keymod::KMOD_LALT as u16);
        assert!(ctrl_bracket(b'[' as i32, c));
        assert!(!ctrl_bracket(b'[' as i32, 0));
        assert!(!ctrl_bracket(b'[' as i32, c | a));
        assert!(!ctrl_bracket(b']' as i32, c));
    }

    #[test]
    fn near_shape_has_one_pixel_of_room() {
        // 826x516 is the size where the rule of Insert 11524 ends: it counts as the shape, and fills the window.
        assert!(near_shape(826, 516, 8, 5));
        assert!(near_shape(1280, 800, 8, 5));
        assert!(!near_shape(830, 516, 8, 5));
        assert!(!near_shape(826, 520, 8, 5));
        assert_eq!(frame_rect(826, 516, SW, SH), (0, 0, 826, 516));
    }

    #[test]
    fn a_window_with_no_width_or_no_height_keeps_its_size() {
        assert_eq!(shape_fix(0, 100, SW, SH), None);
        assert_eq!(shape_fix(100, 0, SW, SH), None);
    }

    #[test]
    fn a_point_below_the_window_stays_below_the_screen() {
        // Inside the window the point clamps to the last row; at the window height and below it stays outside.
        assert_eq!(pixel_of(640, 400, 10, 399), (5, 199));
        assert_eq!(pixel_of(640, 400, 10, 400), (5, 200));
    }

    #[test]
    fn wm_release_size_settles() {
        // Insert 11524: the openbox drag with no hint gives 1040x410; the rule ends at 826x516 and stays there.
        assert_eq!(shape_fix(1040, 410, SW, SH), Some((826, 516)));
        assert_eq!(shape_fix(826, 516, SW, SH), None);
        assert_eq!(shape_fix(1280, 800, SW, SH), None);
        for w in 320..2000 {
            for h in [200, 317, 410, 777, 1080] {
                if let Some((a, b)) = shape_fix(w, h, SW, SH) {
                    assert_eq!(shape_fix(a, b, SW, SH), None, "{} {} -> {} {}", w, h, a, b);
                }
            }
        }
    }

    #[test]
    fn corners_map_back_at_any_size() {
        for (w, h) in [(1280, 800), (320, 200), (826, 516), (1920, 1080), (1000, 1000), (1040, 410), (1280, 801)] {
            for (x, y) in [(0, 0), (319, 0), (0, 199), (319, 199), (160, 100)] {
                let (wx, wy) = window_of(w, h, x, y);
                assert!(wx >= 0 && wy >= 0 && wx < w as i32 && wy < h as i32);
                assert_eq!(pixel_of(w, h, wx, wy), (x, y), "{}x{}", w, h);
            }
        }
    }

    #[test]
    fn bars_count_as_the_nearest_pixel() {
        assert_eq!(frame_rect(1920, 1080, SW, SH), (96, 0, 1728, 1080));
        assert_eq!(pixel_of(1920, 1080, 0, 500).0, 0);
        assert_eq!(pixel_of(1920, 1080, 1919, 500).0, 319);
        assert_eq!(frame_rect(1000, 1000, SW, SH), (0, 187, 1000, 625));
        assert_eq!(pixel_of(1000, 1000, 500, 0).1, 0);
        assert_eq!(pixel_of(1000, 1000, 500, 999).1, 199);
        assert_eq!(pixel_of(1280, 800, 1279, 799), (319, 199));
    }
}

#[cfg(test)]
pub(crate) mod dos_draw_tests {
    use super::*;

    /// #64 step 3: the DOS mouse mode draws the pointer into the picture at the mouse's game pixel, hot spot first,
    /// at the picture's scale; with the pointer off, the picture stays clean.
    pub(crate) fn the_pointer_is_drawn_into_the_picture(video: &sdl2::VideoSubsystem) {
        DOS_TEST.store(true, std::sync::atomic::Ordering::Relaxed);
        let mut cv = Frame::new(video.window("dos", SW * 2, SH * 2).hidden(), SW, SH);
        let tc = cv.texture_creator();
        let mut tex = tc.create_texture_streaming(sdl2::pixels::PixelFormatEnum::RGB24, SW, SH).expect("texture");
        tex.with_lock(None, |b: &mut [u8], _| b.fill(0)).expect("lock");
        // A 3 x 3 red pointer (colour 1 of the shown palette) with its hot spot in the middle.
        dos_pointers(vec![(vec![1; 9], 3, 3, 1, 1)]);
        let mut pal = [0u8; 768];
        pal[3..6].copy_from_slice(&[255, 0, 0]);
        dos_palette(&pal);
        DOS_CAPTURED.with(|c| c.set(true));
        unsafe { sys::SDL_WarpMouseInWindow(cv.window().raw(), 100 * 2 + 1, 50 * 2 + 1) };
        let at = |cv: &Frame, x: i32, y: i32| {
            let p = cv.read_pixels(Some(Rect::new(x, y, 1, 1)), sdl2::pixels::PixelFormatEnum::RGB24).expect("pixels");
            (p[0], p[1], p[2])
        };
        DOS_CUR.with(|c| c.set((0, true)));
        cv.copy(&tex, None, None).expect("copy");
        let (w, h) = cv.window().size();
        let (mut mx, mut my) = (0, 0);
        unsafe { sys::SDL_GetMouseState(&mut mx, &mut my) };
        assert_eq!(pixel_of(w, h, mx, my), (100, 50), "mouse place");
        // Game pixels 99..101 x 49..51 are red, 102 is not.
        assert_eq!(at(&cv, 99 * 2, 49 * 2), (255, 0, 0));
        assert_eq!(at(&cv, 101 * 2 + 1, 51 * 2 + 1), (255, 0, 0));
        assert_eq!(at(&cv, 102 * 2, 50 * 2), (0, 0, 0));
        // A fade step (fade_show -> blit -> dos_palette) gives the picture a darker palette: the pointer takes it too,
        // as in the original, where the pointer is part of the 8-bit picture under the DAC fade (#64 held point).
        pal[3..6].copy_from_slice(&[128, 0, 0]);
        dos_palette(&pal);
        cv.copy(&tex, None, None).expect("copy");
        assert_eq!(at(&cv, 100 * 2, 50 * 2), (128, 0, 0), "pointer colour after a fade step");
        DOS_CUR.with(|c| c.set((0, false)));
        cv.copy(&tex, None, None).expect("copy");
        assert_eq!(at(&cv, 100 * 2, 50 * 2), (0, 0, 0));
        DOS_CAPTURED.with(|c| c.set(false));
        DOS_PTRS.with(|p| p.borrow_mut().clear());
        DOS_TEST.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
mod input_line_tests {
    /// #64 step 2: an input line is "ms x y buttons dx dy", one event per line.
    #[test]
    fn an_input_line_holds_time_place_buttons_and_move() {
        assert_eq!(super::input_line(1234, (160, 99), 3, (-7, 2)), "1234 160 99 3 -7 2\n");
        assert_eq!(super::input_line(0, (-1, -1), 0, (0, 0)), "0 -1 -1 0 0 0\n");
    }

    /// SDL's left, middle, right bits (1, 2, 4) become left 1, right 2, middle 4.
    #[test]
    fn input_buttons_are_left_right_middle() {
        assert_eq!(super::input_buttons(1), 1);
        assert_eq!(super::input_buttons(4), 2);
        assert_eq!(super::input_buttons(2), 4);
        assert_eq!(super::input_buttons(7), 7);
        assert_eq!(super::input_buttons(0), 0);
    }

    /// The frames and the input lines read one clock: it never goes back.
    #[test]
    fn the_record_clock_runs_forward() {
        let a = super::record_ms();
        std::thread::sleep(std::time::Duration::from_millis(15));
        assert!(super::record_ms() >= a + 10);
    }
}
