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

pub fn compute(
    width: i32,
    height: i32,
    scale_percent: i32,
    is_24h: bool,
    vertical: bool,
    is_preview: bool,
) -> Layout {
    let bp = border_percent(scale_percent);
    let (hours_rect, minutes_rect, box_size) = if vertical {
        // One box across, two stacked; separation applied on the Y axis.
        let box_size = calc_box_size(width, bp, 1).min(calc_box_size(height, bp, 2));
        let sep = (box_size as f64 * BOX_SEPARATION_PERCENT).round() as i32;
        let start_x = calc_offset(width, 1, box_size, 0);
        let start_y = calc_offset(height, 2, box_size, sep);
        let hours = Rect { x: start_x, y: start_y, w: box_size, h: box_size };
        let minutes = Rect { y: start_y + box_size + sep, ..hours };
        (hours, minutes, box_size)
    } else {
        let box_size = calc_box_size(width, bp, 2).min(calc_box_size(height, bp, 1));
        let sep = (box_size as f64 * BOX_SEPARATION_PERCENT).round() as i32;
        let start_x = calc_offset(width, 2, box_size, sep);
        let start_y = calc_offset(height, 1, box_size, 0);
        let hours = Rect { x: start_x, y: start_y, w: box_size, h: box_size };
        let minutes = Rect { x: start_x + box_size + sep, ..hours };
        (hours, minutes, box_size)
    };
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

/// Total flip duration; `progress = elapsed_ms / FLIP_MS`.
pub const FLIP_MS: f64 = 600.0;
const MAX_SHADE: f32 = 0.55;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    UpperFold, // t < 0.5: old value folds down to the hinge
    LowerFall, // t >= 0.5: new value falls open from the hinge
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlipFrame {
    pub phase: Phase,
    /// Y-scale of the folding leaf, 0 (edge-on) .. 1 (flat).
    pub leaf_scale: f32,
    /// Translucent-black overlay alpha, 0 .. MAX_SHADE.
    pub shade_alpha: f32,
}

fn ease_in_out_cubic(p: f64) -> f64 {
    if p < 0.5 {
        4.0 * p * p * p
    } else {
        let q = -2.0 * p + 2.0;
        1.0 - q * q * q / 2.0
    }
}

/// Split-flap fold geometry for `progress` in [0,1] (clamped). One flap read
/// as two leaves: phase 1 the upper leaf (old) folds 0->pi/2, phase 2 the
/// lower leaf (new) falls pi/2->0. `leaf_scale = cos(theta)` is the projected
/// foreshortening; using cosine in both phases keeps the two leaves continuous
/// (a linear phase-2 ramp would visibly clash). Shade peaks edge-on.
pub fn flip_frame(progress: f64) -> FlipFrame {
    let t = ease_in_out_cubic(progress.clamp(0.0, 1.0));
    let (phase, theta) = if t < 0.5 {
        (Phase::UpperFold, t * std::f64::consts::PI)
    } else {
        (Phase::LowerFall, (1.0 - t) * std::f64::consts::PI)
    };
    let leaf_scale = theta.cos() as f32;
    let shade_alpha = MAX_SHADE * (1.0 - leaf_scale);
    FlipFrame { phase, leaf_scale, shade_alpha }
}

#[cfg(windows)]
pub mod draw {
    use super::{compute, flip_frame, format_time, BoxLayout, FlipFrame, Layout, Marker, Phase, Rect, FLIP_MS};
    use crate::screensaver::Gfx;
    use crate::settings::Settings;
    use windows::core::*;
    use windows::Win32::Graphics::Direct2D::Common::*;
    use windows::Win32::Graphics::Direct2D::*;
    use windows::Win32::Graphics::DirectWrite::*;
    use windows_numerics::Matrix3x2;

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
        gradient_brush: ID2D1LinearGradientBrush,
        shade: ID2D1SolidColorBrush,
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
            settings: &Settings,
            vertical: bool,
            is_preview: bool,
        ) -> Result<FaceCache> {
            unsafe {
                let layout = compute(
                    width,
                    height,
                    settings.scale,
                    settings.display_24hr,
                    vertical,
                    is_preview,
                );
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
                // Cached once; start/end points are repositioned per box in
                // draw_panel_digit. Rebuilt only when the whole cache is.
                let gradient_brush = rt.CreateLinearGradientBrush(
                    &D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES {
                        startPoint: windows_numerics::Vector2 { X: 0.0, Y: 0.0 },
                        endPoint: windows_numerics::Vector2 { X: 0.0, Y: 1.0 },
                    },
                    None,
                    &gradient,
                )?;
                // Opaque black; the fold sets per-frame opacity for the shade.
                let shade = rt.CreateSolidColorBrush(&color(0x000000), None)?;
                let mk_format = |px: i32| -> Result<IDWriteTextFormat> {
                    let f = gfx.dwrite.CreateTextFormat(
                        &HSTRING::from(gfx.font.family),
                        gfx.font.collection.as_ref().map(|c| c.cast::<IDWriteFontCollection>()).transpose()?.as_ref(),
                        DWRITE_FONT_WEIGHT_BOLD,
                        DWRITE_FONT_STYLE_NORMAL,
                        gfx.font.stretch,
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
                    gradient_brush,
                    shade,
                    large_format,
                    small_format,
                    dwrite: gfx.dwrite.clone(),
                })
            }
        }

        /// Panel gradient + digit + optional marker, under the caller's
        /// current transform and clip. No split line (drawn once on top).
        unsafe fn draw_panel_digit(
            &self,
            rt: &ID2D1HwndRenderTarget,
            bl: &BoxLayout,
            text: &str,
            marker: Option<Marker>,
        ) -> Result<()> {
            let r = rectf(bl.rect);
            // Reposition the cached vertical gradient onto this box.
            self.gradient_brush
                .SetStartPoint(windows_numerics::Vector2 { X: r.left, Y: r.top });
            self.gradient_brush
                .SetEndPoint(windows_numerics::Vector2 { X: r.left, Y: r.bottom });
            let rounded = D2D1_ROUNDED_RECT {
                rect: r,
                radiusX: self.layout.corner_radius as f32,
                radiusY: self.layout.corner_radius as f32,
            };
            rt.FillRoundedRectangle(&rounded, &self.gradient_brush);

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
                let s: Vec<u16> =
                    (if m == Marker::Am { "AM" } else { "PM" }).encode_utf16().collect();
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
            Ok(())
        }

        unsafe fn draw_box(
            &self,
            rt: &ID2D1HwndRenderTarget,
            bl: &BoxLayout,
            text: &str,
            marker: Option<Marker>,
        ) -> Result<()> {
            self.draw_panel_digit(rt, bl, text, marker)?;
            let r = rectf(bl.rect);
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

        /// panel+digit clipped to `clip` under identity (no fold).
        unsafe fn draw_clipped(
            &self,
            rt: &ID2D1HwndRenderTarget,
            bl: &BoxLayout,
            clip: D2D_RECT_F,
            text: &str,
            marker: Option<Marker>,
        ) -> Result<()> {
            rt.SetTransform(&Matrix3x2::identity());
            rt.PushAxisAlignedClip(&clip, D2D1_ANTIALIAS_MODE_ALIASED);
            self.draw_panel_digit(rt, bl, text, marker)?;
            rt.PopAxisAlignedClip();
            Ok(())
        }

        /// One folding leaf: panel+digit clipped to `clip`, squashed to
        /// `leaf_scale` about the hinge, then a shade overlay.
        ///
        /// D2D quirk (do NOT reorder): PushAxisAlignedClip bakes its rect into
        /// device space using the transform active *at push time*; a later
        /// SetTransform does not re-warp an already-pushed clip. So push the
        /// clip under identity (screen coords), THEN set the fold transform.
        /// The pivot sits on the hinge (a fixed point of the scale), and the
        /// squashed content is always a subset of the un-scaled half-rect, so
        /// this order clips correctly in both phases.
        unsafe fn draw_leaf(
            &self,
            rt: &ID2D1HwndRenderTarget,
            bl: &BoxLayout,
            clip: D2D_RECT_F,
            hinge: f32,
            leaf_scale: f32,
            text: &str,
            marker: Option<Marker>,
            shade_alpha: f32,
        ) -> Result<()> {
            rt.SetTransform(&Matrix3x2::identity());
            rt.PushAxisAlignedClip(&clip, D2D1_ANTIALIAS_MODE_ALIASED);
            // scale(1, s) about (x, hinge): windows-numerics uses D2D's
            // row-vector convention, so the hinge line stays static as s
            // varies (a reversed multiply would smear instead of squash -
            // the manual-matrix hinge check catches that).
            rt.SetTransform(&Matrix3x2::scale_around(
                1.0,
                leaf_scale,
                windows_numerics::Vector2 { X: 0.0, Y: hinge },
            ));
            self.draw_panel_digit(rt, bl, text, marker)?;
            self.shade.SetOpacity(shade_alpha);
            let rounded = D2D1_ROUNDED_RECT {
                rect: rectf(bl.rect),
                radiusX: self.layout.corner_radius as f32,
                radiusY: self.layout.corner_radius as f32,
            };
            rt.FillRoundedRectangle(&rounded, &self.shade);
            rt.SetTransform(&Matrix3x2::identity());
            rt.PopAxisAlignedClip();
            Ok(())
        }

        /// Full fold frame for one box: static backgrounds + the moving leaf,
        /// then the split line on top at the hinge.
        unsafe fn draw_fold(
            &self,
            rt: &ID2D1HwndRenderTarget,
            bl: &BoxLayout,
            f: FlipFrame,
            old_text: &str,
            old_marker: Option<Marker>,
            new_text: &str,
            new_marker: Option<Marker>,
        ) -> Result<()> {
            let r = rectf(bl.rect);
            let hinge = bl.split_y as f32;
            let top = D2D_RECT_F { left: r.left, top: r.top, right: r.right, bottom: hinge };
            let bottom = D2D_RECT_F { left: r.left, top: hinge, right: r.right, bottom: r.bottom };
            match f.phase {
                Phase::UpperFold => {
                    // reveal new top behind; old bottom static; old leaf folds down
                    self.draw_clipped(rt, bl, top, new_text, new_marker)?;
                    self.draw_clipped(rt, bl, bottom, old_text, old_marker)?;
                    self.draw_leaf(rt, bl, top, hinge, f.leaf_scale, old_text, old_marker, f.shade_alpha)?;
                }
                Phase::LowerFall => {
                    // new top static; old bottom behind; new leaf falls open
                    self.draw_clipped(rt, bl, top, new_text, new_marker)?;
                    self.draw_clipped(rt, bl, bottom, old_text, old_marker)?;
                    self.draw_leaf(rt, bl, bottom, hinge, f.leaf_scale, new_text, new_marker, f.shade_alpha)?;
                }
            }
            rt.SetTransform(&Matrix3x2::identity());
            rt.DrawLine(
                windows_numerics::Vector2 { X: r.left, Y: hinge },
                windows_numerics::Vector2 { X: r.right, Y: hinge },
                &self.black,
                self.layout.split_stroke,
                None,
            );
            Ok(())
        }

        /// (digit string, marker) for a box given its value. Markers ride the
        /// hours box only; format_time derives them from the hour.
        fn box_text(&self, is_hours: bool, value: u32) -> (String, Option<Marker>) {
            if is_hours {
                let (h, _m, marker) = format_time(value, 0, self.is_24h);
                (h, marker)
            } else {
                (format_time(0, value, self.is_24h).1, None)
            }
        }

        /// Render one box: static when no anim (or finished), else the fold.
        unsafe fn draw_box_animated(
            &self,
            rt: &ID2D1HwndRenderTarget,
            bl: &BoxLayout,
            is_hours: bool,
            settled: u32,
            anim: Option<&crate::screensaver::Anim>,
            now_ms: u64,
        ) -> Result<()> {
            match anim {
                None => {
                    let (t, m) = self.box_text(is_hours, settled);
                    self.draw_box(rt, bl, &t, m)
                }
                Some(a) => {
                    let progress = now_ms.saturating_sub(a.start_ms) as f64 / FLIP_MS;
                    let (new_t, new_m) = self.box_text(is_hours, a.to);
                    if progress >= 1.0 {
                        self.draw_box(rt, bl, &new_t, new_m)
                    } else {
                        let (old_t, old_m) = self.box_text(is_hours, a.from);
                        self.draw_fold(rt, bl, flip_frame(progress), &old_t, old_m, &new_t, new_m)
                    }
                }
            }
        }
    }

    pub fn draw_face(
        rt: &ID2D1HwndRenderTarget,
        cache: &FaceCache,
        shown: (u32, u32),
        hours_anim: Option<&crate::screensaver::Anim>,
        minutes_anim: Option<&crate::screensaver::Anim>,
        now_ms: u64,
    ) -> Result<()> {
        unsafe {
            rt.Clear(Some(&color(0x000000)));
            cache.draw_box_animated(rt, &cache.layout.hours, true, shown.0, hours_anim, now_ms)?;
            cache.draw_box_animated(rt, &cache.layout.minutes, false, shown.1, minutes_anim, now_ms)?;
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
        let l = compute(1920, 1080, 70, false, false, false);
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
        let l = compute(1920, 1080, 70, true, false, false);
        assert_eq!(l.hours.marker_top, None);
        assert_eq!(l.hours.marker_bottom, None);
    }

    #[test]
    fn layout_preview_320x240_scale70() {
        let l = compute(320, 240, 70, false, false, true);
        assert_eq!(l.hours.rect, Rect { x: 38, y: 60, w: 119, h: 119 });
        assert_eq!(l.minutes.rect, Rect { x: 163, y: 60, w: 119, h: 119 });
        // preview split: 1 px hairline at exact box center
        assert_eq!(l.hours.split_y, 119);
        assert_eq!(l.split_stroke, 1.0);
    }

    #[test]
    fn layout_portrait_1080x1920_scale70_vertical() {
        let l = compute(1080, 1920, 70, false, true, false);
        assert_eq!(l.hours.rect, Rect { x: 184, y: 230, w: 712, h: 712 });
        assert_eq!(l.minutes.rect, Rect { x: 184, y: 978, w: 712, h: 712 });
        assert_eq!(l.corner_radius, 35);
        assert_eq!(l.large_font_px, 605);
        assert_eq!(l.small_font_px, 64);
        assert_eq!(l.hours.text, Rect { x: 120, y: 258, w: 854, h: 712 });
        assert_eq!(l.hours.split_y, 584);
        assert_eq!(l.hours.marker_top, Some((219, 301)));
        assert_eq!(l.hours.marker_bottom, Some((219, 871)));
        // markers only ever on the hours box
        assert_eq!(l.minutes.marker_top, None);
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

    #[test]
    fn flip_frame_endpoints() {
        // progress 0: flat, no shade
        let f = flip_frame(0.0);
        assert_eq!(f.phase, Phase::UpperFold);
        assert!((f.leaf_scale - 1.0).abs() < 1e-4);
        assert!(f.shade_alpha.abs() < 1e-4);
        // progress 1: flat again (leaf fully fallen), no shade
        let f = flip_frame(1.0);
        assert_eq!(f.phase, Phase::LowerFall);
        assert!((f.leaf_scale - 1.0).abs() < 1e-4);
        assert!(f.shade_alpha.abs() < 1e-4);
    }

    #[test]
    fn flip_frame_midpoint_is_edge_on() {
        // eased(0.5)==0.5 -> boundary: leaf edge-on (scale 0), shade at peak
        let f = flip_frame(0.5);
        assert_eq!(f.phase, Phase::LowerFall);
        assert!(f.leaf_scale.abs() < 1e-4);
        assert!((f.shade_alpha - 0.55).abs() < 1e-4);
    }

    #[test]
    fn flip_frame_locks_cosine_curve() {
        // A linear phase-2 ramp would give leaf_scale ~0.875 at progress 0.75;
        // the cosine gives ~0.981. Pin both mid-phase points so the shape,
        // not just the ends, is locked.
        let up = flip_frame(0.25);
        assert_eq!(up.phase, Phase::UpperFold);
        assert!((up.leaf_scale - 0.98079).abs() < 1e-3);
        let down = flip_frame(0.75);
        assert_eq!(down.phase, Phase::LowerFall);
        assert!((down.leaf_scale - 0.98079).abs() < 1e-3);
    }

    #[test]
    fn flip_frame_clamps_past_one() {
        let f = flip_frame(1.7);
        assert_eq!(f.phase, Phase::LowerFall);
        assert!((f.leaf_scale - 1.0).abs() < 1e-4);
    }
}
