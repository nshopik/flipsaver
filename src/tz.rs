//! Windows timezone resolution and per-tick local-time conversion. Win32
//! only; the DST decision defers to the host-tested board::dst_active.

use windows::core::*;
use windows::Win32::Foundation::{FILETIME, SYSTEMTIME, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
use windows::Win32::System::Time::*;

/// A configured city plus its resolved zone. `info == None` means the INI
/// key did not match any installed Windows zone (renders as `--:--`).
pub struct Zone {
    pub label: String,
    pub info: Option<DYNAMIC_TIME_ZONE_INFORMATION>,
}

fn key_to_string(key: &[u16; 128]) -> String {
    let end = key.iter().position(|&c| c == 0).unwrap_or(key.len());
    String::from_utf16_lossy(&key[..end])
}

/// Find the zone whose `TimeZoneKeyName` matches `id` case-insensitively.
/// Loops EnumDynamicTimeZoneInformation until ERROR_NO_MORE_ITEMS (259).
/// `UTC` is a real Windows key and resolves through the same path.
pub fn resolve(id: &str) -> Option<DYNAMIC_TIME_ZONE_INFORMATION> {
    unsafe {
        let mut i = 0u32;
        loop {
            let mut dtzi = DYNAMIC_TIME_ZONE_INFORMATION::default();
            let rc = EnumDynamicTimeZoneInformation(i, &mut dtzi);
            if rc == ERROR_NO_MORE_ITEMS.0 {
                return None;
            }
            if rc == ERROR_SUCCESS.0 && key_to_string(&dtzi.TimeZoneKeyName).eq_ignore_ascii_case(id) {
                return Some(dtzi);
            }
            i += 1;
        }
    }
}

/// Resolve every configured city; unmatched keys are logged and kept as
/// unresolved rows. Order is preserved.
pub fn resolve_all(world_clocks: &[(String, String)]) -> Vec<Zone> {
    world_clocks
        .iter()
        .map(|(label, key)| {
            let info = resolve(key);
            if info.is_none() {
                crate::screensaver::debug_log(&format!("flipsaver: unknown timezone '{key}'"));
            }
            Zone { label: label.clone(), info }
        })
        .collect()
}

/// Applied UTC-minus-local bias in minutes, via FILETIME subtraction (robust
/// across day boundaries, unlike field-wise SYSTEMTIME math).
unsafe fn applied_bias(utc: &SYSTEMTIME, local: &SYSTEMTIME) -> Option<i32> {
    let mut fu = FILETIME::default();
    let mut fl = FILETIME::default();
    SystemTimeToFileTime(utc, &mut fu).ok()?;
    SystemTimeToFileTime(local, &mut fl).ok()?;
    let u = ((fu.dwHighDateTime as i64) << 32) | fu.dwLowDateTime as i64;
    let l = ((fl.dwHighDateTime as i64) << 32) | fl.dwLowDateTime as i64;
    // 100 ns units -> minutes.
    Some(((u - l) / 600_000_000) as i32)
}

/// Standard-season bias for `year` (Bias + StandardBias). The zone's *local*
/// year is used because UTC and local year can differ around New Year.
unsafe fn standard_bias(info: &DYNAMIC_TIME_ZONE_INFORMATION, year: u16) -> Option<i32> {
    let mut tzi = TIME_ZONE_INFORMATION::default();
    GetTimeZoneInformationForYear(year, Some(info), &mut tzi).ok()?;
    Some(tzi.Bias + tzi.StandardBias)
}

/// Convert `utc` to this zone's local time, deriving the DST flag and the
/// date-differs flag (vs the machine-local `local_now`). Returns None if the
/// Win32 conversion fails.
pub fn zone_time(
    info: &DYNAMIC_TIME_ZONE_INFORMATION,
    utc: &SYSTEMTIME,
    local_now: &SYSTEMTIME,
) -> Option<crate::board::TimeParts> {
    unsafe {
        let mut local = SYSTEMTIME::default();
        SystemTimeToTzSpecificLocalTimeEx(Some(info), utc, &mut local).ok()?;
        let applied = applied_bias(utc, &local)?;
        let standard = standard_bias(info, local.wYear)?;
        let date_differs = (local.wYear, local.wMonth, local.wDay)
            != (local_now.wYear, local_now.wMonth, local_now.wDay);
        Some(crate::board::TimeParts {
            hour: local.wHour as u32,
            minute: local.wMinute as u32,
            is_dst: crate::board::dst_active(applied, standard),
            date_differs,
            weekday: local.wDayOfWeek as u8,
        })
    }
}
