//! The morning (docs/spec/strategic.md "The morning"; C interface in docs/spec/port_api.md "The morning entry
//! point"): `ja1_morning` runs the morning steps in their order. A child module of `ai`; host op 301 goes through `Ai`.
use super::{Ai, Ja1Host, Ja1World};

const PAYROLL_SHOW: i32 = 301;

// SPEC GAP: a null world or host does nothing (result 0).
#[no_mangle]
pub extern "C" fn ja1_morning(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    if w.is_null() {
        return 0;
    }
    let ai = match Ai::new(w, h, 0) {
        Some(ai) => ai,
        None => return 0,
    };
    ai.call(PAYROLL_SHOW, &[]);
    super::pay::ja1_pay_mercs(w, h);
    super::pay::ja1_collect_cash(w, h);
    super::duties::ja1_morning_duties(w, h);
    if super::payday::ja1_payday_dialog(w, h) == 1 {
        return 1;
    }
    super::stats::ja1_stat_changes(w, h);
    super::morning::ja1_duty_wake(w, h);
    super::corpse::ja1_corpse_export(w, h);
    super::raise::ja1_raise_salary(w, h);
    super::complain::ja1_hated_check(w, h);
    super::complain::ja1_complain_pick(w, h);
    super::morning::ja1_profile_sync(w, h);
    0
}
