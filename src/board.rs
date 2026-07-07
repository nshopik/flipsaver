//! World-clock board: pure layout and formatting. No Win32 types, so it
//! unit-tests on the host. The Direct2D render lives in the `draw`
//! submodule (Win32-only).

/// Cells reserved for the city label (left-aligned, uppercase).
pub const LABEL_CELLS: usize = 16;

/// Precomputed, Win32-free inputs for one row. `weekday` is 0=Sun..6=Sat
/// (Win32 SYSTEMTIME.wDayOfWeek convention).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeParts {
    pub hour: u32,
    pub minute: u32,
    pub is_dst: bool,
    pub date_differs: bool,
    pub weekday: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grid {
    pub cols: usize,
    pub rows: usize,
    pub cell: i32,
    pub origin_x: i32,
    pub origin_y: i32,
}

/// One breathing row top and bottom so the block never touches the edges.
const MARGIN_ROWS: i32 = 2;

/// Right-hand block width: time(5) + gap + [AM/PM(2) + gap] + day(3) + gap
/// + dst(1). All fields reserved unconditionally so the grid never reflows
/// at midnight or a DST transition.
fn right_block_width(is_24h: bool) -> usize {
    if is_24h {
        5 + 1 + 3 + 1 + 1
    } else {
        5 + 1 + 2 + 1 + 3 + 1 + 1
    }
}

/// Total columns in one row: label + gap + right block.
pub fn row_width(is_24h: bool) -> usize {
    LABEL_CELLS + 1 + right_block_width(is_24h)
}

pub fn weekday_abbr(dow: u8) -> &'static str {
    match dow {
        0 => "SUN",
        1 => "MON",
        2 => "TUE",
        3 => "WED",
        4 => "THU",
        5 => "FRI",
        6 => "SAT",
        _ => "???",
    }
}

/// A zone is in DST when the actually-applied bias differs from its
/// standard-season bias (both UTC-minus-local minutes, Win32 sign). Kept
/// pure so it tests without Win32; the biases are derived in `tz.rs`.
pub fn dst_active(actual_bias: i32, standard_bias: i32) -> bool {
    actual_bias != standard_bias
}

/// The changed cell indices between two equal-length rows. City-name cells
/// never change after startup, so steady state is a handful of indices.
pub fn diff_cells(old: &[char], new: &[char]) -> Vec<usize> {
    old.iter()
        .zip(new.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect()
}

fn push_str_field(row: &mut Vec<char>, s: &str, width: usize, right: bool) {
    let chars: Vec<char> = s.chars().take(width).collect();
    let pad = width - chars.len();
    if right {
        for _ in 0..pad {
            row.push(' ');
        }
    }
    row.extend(chars);
    if !right {
        for _ in 0..pad {
            row.push(' ');
        }
    }
}

/// 12h: `H:MM` (hour 1..12, no leading zero); 24h: `HH:MM`. Right-justified
/// in 5 cells either way. `None` renders the unresolved-zone placeholder.
fn time_field(t: Option<TimeParts>, is_24h: bool) -> String {
    match t {
        None => "--:--".to_string(),
        Some(t) if is_24h => format!("{:02}:{:02}", t.hour, t.minute),
        Some(t) => {
            let h12 = match t.hour % 12 {
                0 => 12,
                h => h,
            };
            format!("{}:{:02}", h12, t.minute)
        }
    }
}

/// One full row as a fixed-width `Vec<char>` (length == `row_width`). Label
/// left, everything else in the right block; blank fields are spaces so the
/// grid geometry is stable. `time == None` is an unresolved timezone:
/// label + `--:--`, all other fields blank.
pub fn format_row(label: &str, time: Option<TimeParts>, is_24h: bool) -> Vec<char> {
    let mut row: Vec<char> = Vec::with_capacity(row_width(is_24h));
    push_str_field(&mut row, &label.to_uppercase(), LABEL_CELLS, false);
    row.push(' ');
    push_str_field(&mut row, &time_field(time, is_24h), 5, true);
    if !is_24h {
        row.push(' ');
        let ampm = match time {
            Some(t) if t.hour >= 12 => "PM",
            Some(_) => "AM",
            None => "",
        };
        push_str_field(&mut row, ampm, 2, false);
    }
    row.push(' ');
    let day = match time {
        Some(t) if t.date_differs => weekday_abbr(t.weekday),
        _ => "",
    };
    push_str_field(&mut row, day, 3, false);
    row.push(' ');
    let dst = match time {
        Some(t) if t.is_dst => "*",
        _ => "",
    };
    push_str_field(&mut row, dst, 1, false);
    debug_assert_eq!(row.len(), row_width(is_24h));
    row
}

/// Cell size = min of the horizontal fit (widest possible row) and the
/// vertical fit (all rows + margin), scaled through the clock's border
/// curve so slider 0 keeps the board at 40% of the fit (matching the
/// clock's floor) rather than shrinking linearly to nothing.
/// Content block is centered on the screen.
pub fn compute_grid(width: i32, height: i32, scale_percent: i32, city_count: usize, is_24h: bool) -> Grid {
    let cols = row_width(is_24h) as i32;
    let rows = city_count.max(1) as i32;
    let by_w = width / cols;
    let by_h = height / (rows + MARGIN_ROWS);
    let base = by_w.min(by_h).max(1);
    let frac = 100 - 2 * crate::clock::border_percent(scale_percent);
    let cell = (base * frac / 100).max(1);
    let grid_w = cols * cell;
    let grid_h = rows * cell;
    Grid {
        cols: cols as usize,
        rows: city_count,
        cell,
        origin_x: (width - grid_w) / 2,
        origin_y: (height - grid_h) / 2,
    }
}

#[cfg(windows)]
pub mod draw {
    use super::Grid;
    use crate::screensaver::Gfx;
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

    /// Cached device-dependent board resources. Brushes and the cell text
    /// format survive across frames; only the glyph text layouts are built
    /// per drawn cell (a handful change per minute).
    pub struct BoardCache {
        pub grid: Grid,
        cell_bg: ID2D1SolidColorBrush,
        outline: ID2D1SolidColorBrush,
        glyph: ID2D1SolidColorBrush,
        pub black: ID2D1SolidColorBrush,
        pub shade: ID2D1SolidColorBrush,
        cell_format: IDWriteTextFormat,
        corner_radius: f32,
        dwrite: IDWriteFactory5,
    }

    impl BoardCache {
        pub fn new(
            rt: &ID2D1HwndRenderTarget,
            gfx: &Gfx,
            grid: Grid,
        ) -> Result<BoardCache> {
            unsafe {
                let cell_bg = rt.CreateSolidColorBrush(&color(0x121212), None)?;
                let outline = rt.CreateSolidColorBrush(&color(0x2A2A2A), None)?;
                let glyph = rt.CreateSolidColorBrush(&color(0xB7B7B7), None)?;
                let black = rt.CreateSolidColorBrush(&color(0x000000), None)?;
                let shade = rt.CreateSolidColorBrush(&color(0x000000), None)?;
                let px = (grid.cell as f32 * 0.82).max(1.0);
                let cell_format = gfx.dwrite.CreateTextFormat(
                    &HSTRING::from(gfx.font.family),
                    gfx.font
                        .collection
                        .as_ref()
                        .map(|c| c.cast::<IDWriteFontCollection>())
                        .transpose()?
                        .as_ref(),
                    DWRITE_FONT_WEIGHT_BOLD,
                    DWRITE_FONT_STYLE_NORMAL,
                    gfx.font.stretch,
                    px,
                    w!("en-us"),
                )?;
                cell_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
                cell_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
                Ok(BoardCache {
                    corner_radius: grid.cell as f32 / 12.0,
                    grid,
                    cell_bg,
                    outline,
                    glyph,
                    black,
                    shade,
                    cell_format,
                    dwrite: gfx.dwrite.clone(),
                })
            }
        }

        fn cell_rect(&self, col: usize, row: usize) -> D2D_RECT_F {
            let g = &self.grid;
            let x = (g.origin_x + col as i32 * g.cell) as f32;
            let y = (g.origin_y + row as i32 * g.cell) as f32;
            D2D_RECT_F { left: x, top: y, right: x + g.cell as f32, bottom: y + g.cell as f32 }
        }

        fn rounded(&self, r: D2D_RECT_F) -> D2D1_ROUNDED_RECT {
            D2D1_ROUNDED_RECT { rect: r, radiusX: self.corner_radius, radiusY: self.corner_radius }
        }

        /// One cell: dark rounded background, faint outline, centered glyph.
        /// A space glyph draws the empty cell (background only).
        pub(crate) unsafe fn draw_cell(
            &self,
            rt: &ID2D1HwndRenderTarget,
            col: usize,
            row: usize,
            glyph: char,
        ) -> Result<()> {
            let r = self.cell_rect(col, row);
            let rounded = self.rounded(r);
            rt.FillRoundedRectangle(&rounded, &self.cell_bg);
            rt.DrawRoundedRectangle(&rounded, &self.outline, 1.0, None);
            if glyph != ' ' {
                let wide: Vec<u16> = [glyph].iter().map(|c| *c as u16).collect();
                let tl = self.dwrite.CreateTextLayout(
                    &wide,
                    &self.cell_format,
                    self.grid.cell as f32,
                    self.grid.cell as f32,
                )?;
                rt.DrawTextLayout(
                    windows_numerics::Vector2 { X: r.left, Y: r.top },
                    &tl,
                    &self.glyph,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                );
            }
            Ok(())
        }

        /// A cell mid-flip: reuse the clock's fold geometry. `progress` in
        /// [0,1); at >= 1 the caller draws the settled glyph instead.
        pub(crate) unsafe fn draw_cell_fold(
            &self,
            rt: &ID2D1HwndRenderTarget,
            col: usize,
            row: usize,
            old_glyph: char,
            new_glyph: char,
            progress: f64,
        ) -> Result<()> {
            let r = self.cell_rect(col, row);
            let hinge = (r.top + r.bottom) / 2.0;
            let shade_rect = self.rounded(r);
            crate::clock::draw::fold_frame(
                rt,
                r,
                hinge,
                crate::clock::flip_frame(progress),
                &self.shade,
                &shade_rect,
                &self.black,
                1.0,
                &|| self.draw_cell(rt, col, row, old_glyph),
                &|| self.draw_cell(rt, col, row, new_glyph),
            )
        }
    }

    /// Paint the whole board. Cells with an active flip render the fold;
    /// the rest draw their settled glyph. `now_ms` drives progress.
    pub fn draw_board(
        rt: &ID2D1HwndRenderTarget,
        cache: &BoardCache,
        cells: &[crate::screensaver::CellState],
        now_ms: u64,
    ) -> Result<()> {
        unsafe {
            rt.Clear(Some(&color(0x000000)));
            let cols = cache.grid.cols;
            for row in 0..cache.grid.rows {
                for col in 0..cols {
                    let idx = row * cols + col;
                    let Some(cell) = cells.get(idx) else {
                        cache.draw_cell(rt, col, row, ' ')?;
                        continue;
                    };
                    match &cell.anim {
                        Some(a) => {
                            let progress =
                                now_ms.saturating_sub(a.start_ms) as f64 / crate::clock::FLIP_MS;
                            if progress >= 1.0 {
                                cache.draw_cell(rt, col, row, a.to)?;
                            } else {
                                cache.draw_cell_fold(rt, col, row, a.from, a.to, progress)?;
                            }
                        }
                        None => cache.draw_cell(rt, col, row, cell.glyph)?,
                    }
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(row: &[char]) -> String {
        row.iter().collect()
    }

    #[test]
    fn row_width_reserves_all_fields() {
        assert_eq!(row_width(false), 16 + 1 + (5 + 1 + 2 + 1 + 3 + 1 + 1)); // 31
        assert_eq!(row_width(true), 16 + 1 + (5 + 1 + 3 + 1 + 1)); // 28
    }

    #[test]
    fn format_12h_pm_no_dst_same_date() {
        let t = TimeParts { hour: 15, minute: 7, is_dst: false, date_differs: false, weekday: 3 };
        let row = format_row("Los Angeles", Some(t), false);
        assert_eq!(row.len(), row_width(false));
        // "LOS ANGELES" (11) padded to 16, gap, " 3:07" right-just in 5,
        // gap, "PM", gap, blank day (3), gap, blank dst (1).
        assert_eq!(s(&row), "LOS ANGELES       3:07 PM      ");
    }

    #[test]
    fn format_12h_am_dst_and_day_shown() {
        let t = TimeParts { hour: 0, minute: 5, is_dst: true, date_differs: true, weekday: 3 };
        let row = format_row("Tokyo", Some(t), false);
        assert_eq!(s(&row), "TOKYO            12:05 AM WED *");
    }

    #[test]
    fn format_24h_has_no_ampm_column() {
        let t = TimeParts { hour: 9, minute: 4, is_dst: false, date_differs: false, weekday: 1 };
        let row = format_row("London", Some(t), true);
        assert_eq!(row.len(), row_width(true));
        assert_eq!(s(&row), "LONDON           09:04      ");
    }

    #[test]
    fn day_only_shown_when_date_differs() {
        let same = TimeParts { hour: 9, minute: 0, is_dst: false, date_differs: false, weekday: 2 };
        let diff = TimeParts { hour: 9, minute: 0, is_dst: false, date_differs: true, weekday: 2 };
        assert!(!s(&format_row("X", Some(same), true)).contains("TUE"));
        assert!(s(&format_row("X", Some(diff), true)).contains("TUE"));
    }

    #[test]
    fn unresolved_zone_renders_dashes() {
        let row = format_row("Nowhere", None, false);
        assert_eq!(row.len(), row_width(false));
        assert_eq!(s(&row), "NOWHERE          --:--         ");
    }

    #[test]
    fn label_truncates_and_uppercases() {
        let t = TimeParts { hour: 1, minute: 0, is_dst: false, date_differs: false, weekday: 0 };
        let row = format_row("an extremely long name", Some(t), true);
        let label: String = row[..LABEL_CELLS].iter().collect();
        assert_eq!(label, "AN EXTREMELY LON");
    }

    #[test]
    fn dst_active_compares_bias() {
        assert!(dst_active(-420, -480)); // PDT vs PST
        assert!(!dst_active(-480, -480));
    }

    #[test]
    fn diff_cells_reports_changed_indices() {
        let a: Vec<char> = "12:00".chars().collect();
        let b: Vec<char> = "13:01".chars().collect();
        assert_eq!(diff_cells(&a, &b), vec![1, 4]);
        assert!(diff_cells(&a, &a).is_empty());
    }

    #[test]
    fn weekday_abbr_maps_win32_dow() {
        assert_eq!(weekday_abbr(0), "SUN");
        assert_eq!(weekday_abbr(6), "SAT");
    }

    #[test]
    fn grid_fits_and_centers() {
        // 6 cities, 24h (28 cols). scale 100 for an exact check.
        let g = compute_grid(1920, 1080, 100, 6, true);
        assert_eq!(g.cols, 28);
        assert_eq!(g.rows, 6);
        // by_w = 1920/28 = 68; by_h = 1080/8 = 135; min = 68.
        // scale 100 -> border 5% per side -> 90% of the full fit.
        assert_eq!(g.cell, 68 * 90 / 100);
        assert_eq!(g.origin_x, (1920 - 28 * g.cell) / 2);
        assert_eq!(g.origin_y, (1080 - 6 * g.cell) / 2);
    }

    #[test]
    fn grid_scale_shrinks_cell() {
        let full = compute_grid(1920, 1080, 100, 6, true).cell;
        let half = compute_grid(1920, 1080, 50, 6, true).cell;
        assert!(half < full);
        assert!(half > full / 2, "scale curve must not be linear-to-zero");
    }

    #[test]
    fn grid_scale_zero_keeps_clock_floor() {
        // Slider 0 must match the clock's floor: border 30% per side,
        // i.e. cells sized to 40% of the full fit — not 1px.
        let base = 1920 / 28; // horizontal fit wins at 1080p
        let g = compute_grid(1920, 1080, 0, 6, true);
        assert_eq!(g.cell, base * 40 / 100);
    }
}
