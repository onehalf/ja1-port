//! turns.md "New turn" step 14 e: the view wipes of host op 164 view_save (0x124FC, the view closes from both sides
//! to black) and op 165 view_restore (0x1259C, the new view opens from the middle row). Each step waits 1 ms
//! (0x67250(1)); the screens below are the screen at each wait.

/// The view window on the 320 x 200 screen: left, top, width, height (SetViewWindow 0x24880: 0x229CB8, 0x229CC4,
/// 0x229CBC, 0x229CC0; the right column 0x229CD8 = left + width - 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub w: i32,
    pub h: i32,
}

/// The wait of one wipe step in ms.
pub const STEP_MS: u64 = 1;

fn col_black(s: &mut [u8], x: i32, top: i32, h: i32) {
    for y in top..top + h {
        if let Some(p) = s.get_mut((y * 320 + x) as usize) {
            if (0..320).contains(&x) {
                *p = 0;
            }
        }
    }
}

/// Op 164 (0x124FC): for i = 0 .. w / 2 - 1 the columns left + i and right - i go black (FillRect 1 x h), then a
/// wait. The result has one screen per wait.
pub fn close(screen: &[u8], r: Rect) -> Vec<Vec<u8>> {
    let mut s = screen.to_vec();
    let right = r.left + r.w - 1;
    let mut out = Vec::new();
    for i in 0..r.w / 2 {
        col_black(&mut s, r.left + i, r.top, r.h);
        col_black(&mut s, right - i, r.top, r.h);
        out.push(s.clone());
    }
    out
}

/// Op 165 (0x1259C): `view` = the new view picture (w x h, row 0 at the top; the original draws it into the back
/// buffer at (32, 28)). With half = h / 2, for d = 0 .. half - 1 view rows half - d and half + d go to screen rows
/// top + half - d and top + half + d (w bytes each), then a wait. View row 0 (and row h - 1 when h is odd) never
/// reaches the screen (original quirk).
pub fn open(screen: &[u8], view: &[u8], r: Rect) -> Vec<Vec<u8>> {
    let mut s = screen.to_vec();
    let half = r.h / 2;
    let mut out = Vec::new();
    let row = |s: &mut Vec<u8>, src: i32, dst: i32| {
        let (a, b) = ((src * r.w) as usize, (dst * 320 + r.left) as usize);
        if let (Some(v), Some(d)) = (view.get(a..a + r.w as usize), s.get_mut(b..b + r.w as usize)) {
            d.copy_from_slice(v);
        }
    };
    for d in 0..half {
        row(&mut s, half - d, r.top + half - d);
        row(&mut s, half + d, r.top + half + d);
        out.push(s.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wipes() {
        let r = Rect { left: 48, top: 14, w: 224, h: 168 };
        let scr = vec![7u8; 64000];
        let c = close(&scr, r);
        assert_eq!(c.len(), 112);
        assert!(c[111][14 * 320 + 48..14 * 320 + 272].iter().all(|&v| v == 0));
        assert_eq!(c[0][14 * 320 + 49], 7);
        let view = vec![5u8; 224 * 168];
        let o = open(&c[111], &view, r);
        assert_eq!(o.len(), 84);
        assert_eq!(o[83][14 * 320 + 48], 0);
        assert_eq!(o[83][15 * 320 + 48], 5);
        assert_eq!(o[83][181 * 320 + 48], 5);
    }
}
