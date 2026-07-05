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

#[cfg(windows)]
pub mod draw {
    use super::{compute, format_time, BoxLayout, Layout, Marker, Rect};
    use crate::screensaver::Gfx;
    use crate::settings::Settings;
    use windows::core::*;
    use windows::Win32::Graphics::Direct2D::Common::*;
    use windows::Win32::Graphics::Direct2D::*;
    use windows::Win32::Graphics::DirectWrite::*;

    fn color(rgb: u32) -> D2D1_COLOR_F {
        D2D1_COLOR_F {
            r: ((rgb >> 16) & 0xFF) as f32 / 255.0,
            g: ((rgb >> 8) & 0xFF) as f32 / 255.0,
            b: (rgb & 0xFF) as f32 / 255.0,
            a: 1.0,
        }
    }

    fn rectf(r: Rect) -> D2D_RECT_F {
        D2D_RECT_F {
            left: r.x as f32,
            top: r.y as f32,
            right: (r.x + r.w) as f32,
            bottom: (r.y + r.h) as f32,
        }
    }

    pub struct FaceCache {
        pub layout: Layout,
        pub is_24h: bool,
        digits: ID2D1SolidColorBrush,
        black: ID2D1SolidColorBrush,
        gradient: ID2D1GradientStopCollection,
        large_format: IDWriteTextFormat,
        small_format: IDWriteTextFormat,
        dwrite: IDWriteFactory5,
    }

    impl FaceCache {
        pub fn new(
            rt: &ID2D1HwndRenderTarget,
            gfx: &Gfx,
            width: i32,
            height: i32,
            settings: Settings,
            is_preview: bool,
        ) -> Result<FaceCache> {
            unsafe {
                let layout =
                    compute(width, height, settings.scale, settings.display_24hr, is_preview);
                let digits = rt.CreateSolidColorBrush(&color(0xB7B7B7), None)?;
                let black = rt.CreateSolidColorBrush(&color(0x000000), None)?;
                let stops = [
                    D2D1_GRADIENT_STOP { position: 0.0, color: color(0x121212) },
                    D2D1_GRADIENT_STOP { position: 1.0, color: color(0x0A0A0A) },
                ];
                let gradient = rt.CreateGradientStopCollection(
                    &stops,
                    D2D1_GAMMA_2_2,
                    D2D1_EXTEND_MODE_CLAMP,
                )?;
                let mk_format = |px: i32| -> Result<IDWriteTextFormat> {
                    let f = gfx.dwrite.CreateTextFormat(
                        &HSTRING::from(gfx.family),
                        gfx.fonts.as_ref().map(|c| c.cast::<IDWriteFontCollection>()).transpose()?.as_ref(),
                        DWRITE_FONT_WEIGHT_BOLD,
                        DWRITE_FONT_STYLE_NORMAL,
                        DWRITE_FONT_STRETCH_NORMAL,
                        px as f32,
                        w!("en-us"),
                    )?;
                    Ok(f)
                };
                let large_format = mk_format(layout.large_font_px)?;
                // Digits center in the (already offset) text rect, matching
                // the original's StringFormat Center/Center.
                large_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
                large_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
                let small_format = mk_format(layout.small_font_px)?;
                Ok(FaceCache {
                    layout,
                    is_24h: settings.display_24hr,
                    digits,
                    black,
                    gradient,
                    large_format,
                    small_format,
                    dwrite: gfx.dwrite.clone(),
                })
            }
        }

        unsafe fn draw_box(
            &self,
            rt: &ID2D1HwndRenderTarget,
            bl: &BoxLayout,
            text: &str,
            marker: Option<Marker>,
        ) -> Result<()> {
            let r = rectf(bl.rect);
            // Vertical gradient per box (original: LinearGradientMode.Vertical).
            let brush = rt.CreateLinearGradientBrush(
                &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                    startPoint: windows_numerics::Vector2 { X: r.left, Y: r.top },
                    endPoint: windows_numerics::Vector2 { X: r.left, Y: r.bottom },
                },
                None,
                &self.gradient,
            )?;
            let rounded = D2D1_ROUNDED_RECT {
                rect: r,
                radiusX: self.layout.corner_radius as f32,
                radiusY: self.layout.corner_radius as f32,
            };
            rt.FillRoundedRectangle(&rounded, &brush);

            let wide: Vec<u16> = text.encode_utf16().collect();
            let tl = self.dwrite.CreateTextLayout(
                &wide,
                &self.large_format,
                bl.text.w as f32,
                bl.text.h as f32,
            )?;
            rt.DrawTextLayout(
                windows_numerics::Vector2 { X: bl.text.x as f32, Y: bl.text.y as f32 },
                &tl,
                &self.digits,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );

            if let Some(m) = marker {
                let s: Vec<u16> = (if m == Marker::Am { "AM" } else { "PM" }).encode_utf16().collect();
                let ml = self.dwrite.CreateTextLayout(&s, &self.small_format, 4096.0, 4096.0)?;
                match m {
                    Marker::Am => {
                        let (x, y) = bl.marker_top.unwrap();
                        rt.DrawTextLayout(
                            windows_numerics::Vector2 { X: x as f32, Y: y as f32 },
                            &ml,
                            &self.digits,
                            D2D1_DRAW_TEXT_OPTIONS_NONE,
                        );
                    }
                    Marker::Pm => {
                        // Anchor is the text bottom edge (original subtracts
                        // Font.Height); use the measured line height.
                        let mut metrics = DWRITE_TEXT_METRICS::default();
                        ml.GetMetrics(&mut metrics)?;
                        let (x, y_bottom) = bl.marker_bottom.unwrap();
                        rt.DrawTextLayout(
                            windows_numerics::Vector2 {
                                X: x as f32,
                                Y: y_bottom as f32 - metrics.height,
                            },
                            &ml,
                            &self.digits,
                            D2D1_DRAW_TEXT_OPTIONS_NONE,
                        );
                    }
                }
            }

            // Split line per box, never spanning the face.
            rt.DrawLine(
                windows_numerics::Vector2 { X: r.left, Y: bl.split_y as f32 },
                windows_numerics::Vector2 { X: r.right, Y: bl.split_y as f32 },
                &self.black,
                self.layout.split_stroke,
                None,
            );
            Ok(())
        }
    }

    pub fn draw_face(
        rt: &ID2D1HwndRenderTarget,
        cache: &FaceCache,
        hour: u32,
        minute: u32,
    ) -> Result<()> {
        unsafe {
            rt.Clear(Some(&color(0x000000)));
            let (h, m, marker) = format_time(hour, minute, cache.is_24h);
            // One marker only: AM top corner before noon, PM bottom after.
            cache.draw_box(rt, &cache.layout.hours, &h, marker)?;
            cache.draw_box(rt, &cache.layout.minutes, &m, None)?;
            Ok(())
        }
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
