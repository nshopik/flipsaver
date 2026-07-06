//! Font preference order. Pure logic, host-testable; the DirectWrite probe
//! lives in screensaver.rs. The licensed Helvetica LT Std never ships with
//! the binary — it is only used when already installed on the system.

pub struct Candidate {
    pub family: &'static str,
    /// True when the family needs DWRITE_FONT_STRETCH_CONDENSED to select
    /// the condensed face (typographic family). The GDI-compatible
    /// "... Cond" family already is the condensed face at normal stretch.
    pub condensed: bool,
}

/// Probe order, most preferred first. Family names verified against the
/// reference TTFs (fc-scan): WSS family "Helvetica LT Std Cond" (Bold),
/// typographic family "Helvetica LT Std" (style "Bold Condensed").
pub const SYSTEM_CANDIDATES: [Candidate; 2] = [
    Candidate { family: "Helvetica LT Std Cond", condensed: false },
    Candidate { family: "Helvetica LT Std", condensed: true },
];

/// First usable candidate wins; None means fall back to embedded Oswald.
/// The predicate must verify the bold condensed face is actually present,
/// not just the family name (see screensaver::pick_font).
pub fn pick(is_usable: impl Fn(&Candidate) -> bool) -> Option<&'static Candidate> {
    SYSTEM_CANDIDATES.iter().find(|c| is_usable(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_wss_family_when_both_installed() {
        let c = pick(|_| true).unwrap();
        assert_eq!(c.family, "Helvetica LT Std Cond");
        assert!(!c.condensed);
    }

    #[test]
    fn typographic_family_needs_condensed_stretch() {
        let c = pick(|c| c.family == "Helvetica LT Std").unwrap();
        assert_eq!(c.family, "Helvetica LT Std");
        assert!(c.condensed);
    }

    #[test]
    fn none_installed_falls_back() {
        assert!(pick(|_| false).is_none());
    }
}
