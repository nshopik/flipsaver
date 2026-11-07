//! Windows timezone resolution and per-tick local-time conversion. Win32
//! only.

use windows::Win32::Foundation::{SYSTEMTIME, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
use windows::Win32::System::Time::*;

/// A configured city plus its resolved zone. `info == None` means the INI
/// key did not match any installed Windows zone (renders as `--:--`).
pub struct Zone {
    pub label: String,
    pub info: Option<DYNAMIC_TIME_ZONE_INFORMATION>,
}

/// Find the zone whose `TimeZoneKeyName` matches `id` case-insensitively.
/// Enumerates until ERROR_NO_MORE_ITEMS; an index that fails to read is
/// logged and skipped. `UTC` is a real Windows key and resolves through
/// the same path.
pub fn resolve(id: &str) -> Option<DYNAMIC_TIME_ZONE_INFORMATION> {
    // Windows ships ~140 zones; the cap ends the loop if the end marker
    // never comes.
    for i in 0..1024u32 {
        let mut dtzi = DYNAMIC_TIME_ZONE_INFORMATION::default();
        let rc = unsafe { EnumDynamicTimeZoneInformation(i, &mut dtzi) };
        if rc == ERROR_NO_MORE_ITEMS.0 {
            return None;
        }
        if rc != ERROR_SUCCESS.0 {
            crate::screensaver::debug_log(&format!("flipsaver: timezone index {i} unreadable ({rc})"));
            continue;
        }
        if crate::screensaver::wide_to_string(&dtzi.TimeZoneKeyName).eq_ignore_ascii_case(id) {
            return Some(dtzi);
        }
    }
    None
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

/// Convert `utc` to this zone's local time, deriving the date-differs flag
/// (vs the machine-local `local_now`). Returns None if the Win32 conversion
/// fails.
pub fn zone_time(
    info: &DYNAMIC_TIME_ZONE_INFORMATION,
    utc: &SYSTEMTIME,
    local_now: &SYSTEMTIME,
) -> Option<crate::board::TimeParts> {
    unsafe {
        let mut local = SYSTEMTIME::default();
        SystemTimeToTzSpecificLocalTimeEx(Some(info), utc, &mut local).ok()?;
        let date_differs = (local.wYear, local.wMonth, local.wDay)
            != (local_now.wYear, local_now.wMonth, local_now.wDay);
        Some(crate::board::TimeParts {
            hour: local.wHour as u32,
            minute: local.wMinute as u32,
            date_differs,
            weekday: local.wDayOfWeek as u8,
        })
    }
}
