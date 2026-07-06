//! /c mode: minimal settings dialog from an in-code DLGTEMPLATE.

use crate::settings::{self, Settings};
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const IDC_12H: i32 = 101;
const IDC_24H: i32 = 102;
const IDC_SCALE: i32 = 103;

// Missing from the windows crate's Controls bindings; TBM_GETPOS = WM_USER.
const TBM_GETPOS: u32 = 1024;

struct DlgBuilder {
    words: Vec<u16>,
}

impl DlgBuilder {
    fn new(title: &str, cx: i16, cy: i16, item_count: u16) -> DlgBuilder {
        let mut b = DlgBuilder { words: Vec::new() };
        let style: u32 = (DS_SETFONT as u32)
            | (DS_MODALFRAME as u32)
            | WS_POPUP.0
            | WS_CAPTION.0
            | WS_SYSMENU.0;
        b.dword(style);
        b.dword(0); // dwExtendedStyle
        b.word(item_count);
        b.word(0); // x
        b.word(0); // y
        b.word(cx as u16);
        b.word(cy as u16);
        b.word(0); // no menu
        b.word(0); // default dialog class
        b.wstr(title);
        b.word(8); // font point size (DS_SETFONT)
        b.wstr("MS Shell Dlg");
        b
    }

    fn word(&mut self, w: u16) {
        self.words.push(w);
    }

    fn dword(&mut self, d: u32) {
        self.word((d & 0xFFFF) as u16);
        self.word((d >> 16) as u16);
    }

    fn wstr(&mut self, s: &str) {
        self.words.extend(s.encode_utf16());
        self.word(0);
    }

    fn align_dword(&mut self) {
        if self.words.len() % 2 == 1 {
            self.word(0);
        }
    }

    fn item_atom(&mut self, style: u32, x: i16, y: i16, cx: i16, cy: i16, id: u16, atom: u16, text: &str) {
        self.item_header(style, x, y, cx, cy, id);
        self.word(0xFFFF);
        self.word(atom);
        self.wstr(text);
        self.word(0); // no creation data
    }

    fn item_class(&mut self, style: u32, x: i16, y: i16, cx: i16, cy: i16, id: u16, class: &str, text: &str) {
        self.item_header(style, x, y, cx, cy, id);
        self.wstr(class);
        self.wstr(text);
        self.word(0);
    }

    fn item_header(&mut self, style: u32, x: i16, y: i16, cx: i16, cy: i16, id: u16) {
        self.align_dword();
        self.dword(style | WS_CHILD.0 | WS_VISIBLE.0);
        self.dword(0); // exstyle
        self.word(x as u16);
        self.word(y as u16);
        self.word(cx as u16);
        self.word(cy as u16);
        self.word(id);
    }
}

fn build_template(font_name: &str) -> Vec<u16> {
    let mut b = DlgBuilder::new("FlipSaver Settings", 175, 92, 9);
    b.item_atom(0, 7, 9, 45, 8, 0, 0x0082, "Time format:"); // STATIC
    b.item_atom(
        BS_AUTORADIOBUTTON as u32 | WS_TABSTOP.0 | WS_GROUP.0,
        60, 7, 45, 10, IDC_12H as u16, 0x0080, "12 hour",
    );
    b.item_atom(BS_AUTORADIOBUTTON as u32, 115, 7, 45, 10, IDC_24H as u16, 0x0080, "24 hour");
    b.item_atom(0, 7, 32, 45, 8, 0, 0x0082, "Size:");
    b.item_class(
        (TBS_AUTOTICKS | TBS_HORZ) as u32 | WS_TABSTOP.0 | WS_GROUP.0,
        58, 29, 110, 15, IDC_SCALE as u16, "msctls_trackbar32", "",
    );
    b.item_atom(0, 7, 52, 45, 8, 0, 0x0082, "Font:");
    b.item_atom(0, 60, 52, 108, 8, 0, 0x0082, font_name);
    b.item_atom(
        BS_DEFPUSHBUTTON as u32 | WS_TABSTOP.0 | WS_GROUP.0,
        63, 71, 50, 14, IDOK.0 as u16, 0x0080, "OK",
    );
    b.item_atom(BS_PUSHBUTTON as u32 | WS_TABSTOP.0, 118, 71, 50, 14, IDCANCEL.0 as u16, 0x0080, "Cancel");
    b.words
}

unsafe extern "system" fn dlgproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> isize {
    match msg {
        WM_INITDIALOG => {
            let s = &*(lp.0 as *const Settings);
            let _ = CheckRadioButton(hwnd, IDC_12H, IDC_24H, if s.display_24hr { IDC_24H } else { IDC_12H });
            // Slider is 0..10; INI stores slider x 10 (0..100), like FlipIt.
            let _ = SendDlgItemMessageW(hwnd, IDC_SCALE, TBM_SETRANGE, WPARAM(1), LPARAM(10 << 16));
            let _ = SendDlgItemMessageW(hwnd, IDC_SCALE, TBM_SETPOS, WPARAM(1), LPARAM((s.scale / 10) as isize));
            1
        }
        WM_COMMAND => match (wp.0 & 0xFFFF) as i32 {
            id if id == IDOK.0 => {
                let pos = SendDlgItemMessageW(hwnd, IDC_SCALE, TBM_GETPOS, WPARAM(0), LPARAM(0)).0 as i32;
                let mut s = settings::load(&settings::default_path());
                s.display_24hr = IsDlgButtonChecked(hwnd, IDC_24H) == 1;
                s.scale = pos * 10;
                let _ = settings::save(&settings::default_path(), &s);
                let _ = EndDialog(hwnd, 1);
                1
            }
            id if id == IDCANCEL.0 => {
                let _ = EndDialog(hwnd, 0);
                1
            }
            _ => 0,
        },
        _ => 0,
    }
}

pub fn run_config() {
    unsafe {
        // Trackbar class lives in comctl32; v6 activation comes from the
        // embedded manifest (always present; compiled via llvm-rc at build time).
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES,
        };
        let _ = InitCommonControlsEx(&icc);
        let settings = settings::load(&settings::default_path());
        let template = build_template(crate::screensaver::font_display_name());
        let instance: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        let _ = DialogBoxIndirectParamW(
            Some(instance),
            template.as_ptr() as *const DLGTEMPLATE,
            None,
            Some(dlgproc),
            LPARAM(&settings as *const Settings as isize),
        );
    }
}
