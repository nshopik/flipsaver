//! Layout math for the flip-clock face. Pure data, no Win32 types, so it
//! unit-tests on the Linux host. Constants replicate CurrentTimeScreen.cs.

const SPLIT_WIDTH: i32 = 4;
const BOX_SEPARATION_PERCENT: f64 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxLayout {
    pub rect: Rect,
    pub text: Rect,
    pub split_y: i32,
    /// Top-left anchor of the AM marker (12h mode, hours box only).
    pub marker_top: Option<(i32, i32)>,
    /// (x, bottom-edge y) anchor of the PM marker; drawer subtracts the
    /// measured small-text line height (original: bottom - diff - Font.Height).
    pub marker_bottom: Option<(i32, i32)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub hours: BoxLayout,
    pub minutes: BoxLayout,
    pub corner_radius: i32,
    pub large_font_px: i32,
    pub small_font_px: i32,
    pub split_stroke: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    Am,
    Pm,
}

pub fn border_percent(scale_percent: i32) -> i32 {
    (100 - scale_percent) / 4 + 5
}

fn calc_box_size(total: i32, border_percent: i32, box_count: i32) -> i32 {
    let border = total * border_percent / 100; // truncating, like Percent(int)
    let remaining = (total - border * 2) as f64;
    let parts = (1.0 + BOX_SEPARATION_PERCENT) * box_count as f64 - BOX_SEPARATION_PERCENT;
    (remaining / parts).round() as i32
}

fn calc_offset(total: i32, box_count: i32, box_size: i32, separator: i32) -> i32 {
    let all = (box_size + separator) * box_count - separator;
    (total - all) / 2
}

fn box_layout(rect: Rect, with_markers: bool, is_preview: bool) -> BoxLayout {
    let diff = rect.w / 10;
    let x_off = rect.w * 1 / 100;
    let y_off = rect.h * 4 / 100;
    let text = Rect { x: rect.x - diff + x_off, y: rect.y + y_off, w: rect.w + diff * 2, h: rect.h };
    let split_y = if is_preview {
        rect.y + rect.h / 2
    } else {
        rect.y + rect.h / 2 - SPLIT_WIDTH / 2
    };
    let (marker_top, marker_bottom) = if with_markers {
        let left = rect.x + diff / 2;
        (Some((left, rect.y + diff)), Some((left, rect.y + rect.h - diff)))
    } else {
        (None, None)
    };
    BoxLayout { rect, text, split_y, marker_top, marker_bottom }
}

pub fn compute(width: i32, height: i32, scale_percent: i32, is_24h: bool, is_preview: bool) -> Layout {
    let bp = border_percent(scale_percent);
    let box_size = calc_box_size(width, bp, 2).min(calc_box_size(height, bp, 1));
    let sep = (box_size as f64 * BOX_SEPARATION_PERCENT).round() as i32;
    let start_x = calc_offset(width, 2, box_size, sep);
    let start_y = calc_offset(height, 1, box_size, 0);
    let hours_rect = Rect { x: start_x, y: start_y, w: box_size, h: box_size };
    let minutes_rect = Rect { x: start_x + box_size + sep, ..hours_rect };
    Layout {
        hours: box_layout(hours_rect, !is_24h, is_preview),
        minutes: box_layout(minutes_rect, false, is_preview),
        corner_radius: box_size / 20,
        large_font_px: box_size * 85 / 100,
        small_font_px: box_size * 9 / 100,
        split_stroke: if is_preview { 1.0 } else { SPLIT_WIDTH as f32 },
    }
}

pub fn format_time(hour: u32, minute: u32, is_24h: bool) -> (String, String, Option<Marker>) {
    let minute_s = format!("{minute:02}");
    if is_24h {
        (format!("{hour:02}"), minute_s, None)
    } else {
        let h12 = match hour % 12 {
            0 => 12,
            h => h,
        };
        let marker = if hour >= 12 { Marker::Pm } else { Marker::Am };
        (h12.to_string(), minute_s, Some(marker))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_percent_range() {
        assert_eq!(border_percent(100), 5);
        assert_eq!(border_percent(70), 12);
        assert_eq!(border_percent(0), 30);
    }

    #[test]
    fn layout_1080p_scale70_fullscreen_12h() {
        let l = compute(1920, 1080, 70, false, false);
        assert_eq!(l.hours.rect, Rect { x: 230, y: 184, w: 712, h: 712 });
        assert_eq!(l.minutes.rect, Rect { x: 978, y: 184, w: 712, h: 712 });
        assert_eq!(l.corner_radius, 35);
        assert_eq!(l.large_font_px, 605);
        assert_eq!(l.small_font_px, 64);
        assert_eq!(l.hours.text, Rect { x: 166, y: 212, w: 854, h: 712 });
        assert_eq!(l.hours.split_y, 538);
        assert_eq!(l.split_stroke, 4.0);
        assert_eq!(l.hours.marker_top, Some((265, 255)));
        assert_eq!(l.hours.marker_bottom, Some((265, 825)));
        // markers only ever on the hours box
        assert_eq!(l.minutes.marker_top, None);
        assert_eq!(l.minutes.marker_bottom, None);
    }

    #[test]
    fn layout_24h_has_no_markers() {
        let l = compute(1920, 1080, 70, true, false);
        assert_eq!(l.hours.marker_top, None);
        assert_eq!(l.hours.marker_bottom, None);
    }

    #[test]
    fn layout_preview_320x240_scale70() {
        let l = compute(320, 240, 70, false, true);
        assert_eq!(l.hours.rect, Rect { x: 38, y: 60, w: 119, h: 119 });
        assert_eq!(l.minutes.rect, Rect { x: 163, y: 60, w: 119, h: 119 });
        // preview split: 1 px hairline at exact box center
        assert_eq!(l.hours.split_y, 119);
        assert_eq!(l.split_stroke, 1.0);
    }

    #[test]
    fn format_12h() {
        assert_eq!(format_time(0, 5, false), ("12".into(), "05".into(), Some(Marker::Am)));
        assert_eq!(format_time(11, 59, false), ("11".into(), "59".into(), Some(Marker::Am)));
        assert_eq!(format_time(12, 0, false), ("12".into(), "00".into(), Some(Marker::Pm)));
        assert_eq!(format_time(13, 7, false), ("1".into(), "07".into(), Some(Marker::Pm)));
    }

    #[test]
    fn format_24h() {
        assert_eq!(format_time(0, 5, true), ("00".into(), "05".into(), None));
        assert_eq!(format_time(23, 5, true), ("23".into(), "05".into(), None));
    }
}
