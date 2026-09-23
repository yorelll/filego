//! Resolved light/dark/system theme tokens (M03.4).
//!
//! The Slint UI reads a [`ResolvedTheme`] snapshot (exports a flat set of hex
//! token string properties from Rust via `set_theme_tokens` / the adapter) so
//! the tokens live in one place and can be unit-tested for contrast. Slint's
//! bundled Fluent widgets resolve their own palette from the same
//! `ColorScheme` decision through `Palette.color-scheme`, so the custom window
//! tokens and the widget palette always agree.
//!
//! `system` is not a color: the adapter resolves it to Light/Dark at bind time
//! by asking the platform for the current color scheme. This module only maps a
//! preference plus a resolved scheme into concrete tokens.

use crate::domain::settings::ThemePreference;

/// A concrete resolved color scheme (\"light\" vs \"dark\").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResolvedColorScheme {
    Light,
    #[default]
    Dark,
}

/// Concrete, contrast-checked token set for one scheme.
///
/// All pairings satisfy at least WCAG AA 4.5:1 text contrast (computed from
/// sRGB relative luminance in the tests below). Values are chosen to blend with
/// the Fluent-on-`#FAFAFA`/`#1C1C1C` base colours of Slint 1.18's fluent style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedTheme {
    pub scheme: ResolvedColorScheme,
    /// Main window background.
    pub background: &'static str,
    /// Card/search-container surface.
    pub surface: &'static str,
    /// Hairline separators and control borders.
    pub border: &'static str,
    /// Primary text.
    pub text: &'static str,
    /// Secondary text (path/tag line, hints).
    pub text_secondary: &'static str,
    /// Accent (selected row background tint / focus).
    pub accent: &'static str,
    /// Text on the accent surface.
    pub on_accent: &'static str,
    /// Warning / inaccessible glyph colour — text group, never a whole-row red.
    pub warning: &'static str,
    /// Row selection background tint (distinct from accent, still visible as a
    /// selection background, not a colour-only signal: the row borders also
    /// change and an accessible label toggles).
    pub selection: &'static str,
}

impl ResolvedTheme {
    pub const LIGHT: ResolvedTheme = ResolvedTheme {
        scheme: ResolvedColorScheme::Light,
        background: "#F7F8FA",
        surface: "#FFFFFF",
        border: "#E2E5EA",
        text: "#1B1B1F",
        text_secondary: "#5F6368",
        accent: "#005FB8",
        on_accent: "#FFFFFF",
        // `#8A5A00` (a darker amber) is used so the warning text meets WCAG AA
        // 4.5:1 on the `#F7F8FA` light background (computed in the tests).
        warning: "#8A5A00",
        selection: "#DEEBFA",
    };

    pub const DARK: ResolvedTheme = ResolvedTheme {
        scheme: ResolvedColorScheme::Dark,
        background: "#1C1C1C",
        surface: "#252526",
        border: "#3A3A3D",
        text: "#F3F3F3",
        text_secondary: "#C9C9CD",
        accent: "#60CDFF",
        on_accent: "#000000",
        warning: "#F2C94C",
        selection: "#39393D",
    };

    /// Resolve a user preference plus a system scheme into concrete tokens.
    pub fn resolve(preference: ThemePreference, system: ResolvedColorScheme) -> ResolvedTheme {
        let scheme = match preference {
            ThemePreference::Light => ResolvedColorScheme::Light,
            ThemePreference::Dark => ResolvedColorScheme::Dark,
            ThemePreference::System => system,
        };
        match scheme {
            ResolvedColorScheme::Light => ResolvedTheme::LIGHT,
            ResolvedColorScheme::Dark => ResolvedTheme::DARK,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ResolvedColorScheme, ResolvedTheme};
    use crate::domain::settings::ThemePreference;

    /// WCAG AA body-text threshold.
    const AA: f64 = 4.5;

    // sRGB relative luminance (WCAG 2.x definition).
    fn luminance(hex: &str) -> f64 {
        let rgb: u32 = u32::from_str_radix(&hex[1..], 16).expect("token must be a #RRGGBB hex");
        let channel = |shift: u32| {
            let value = ((rgb >> shift) & 0xFF) as f64 / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    }

    fn contrast_ratio(a: &str, b: &str) -> f64 {
        let (hi, lo) = if luminance(a) > luminance(b) {
            (luminance(a), luminance(b))
        } else {
            (luminance(b), luminance(a))
        };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn system_preference_follows_the_system_scheme() {
        assert_eq!(
            ResolvedTheme::resolve(ThemePreference::System, ResolvedColorScheme::Dark),
            ResolvedTheme::DARK
        );
        assert_eq!(
            ResolvedTheme::resolve(ThemePreference::System, ResolvedColorScheme::Light),
            ResolvedTheme::LIGHT
        );
        assert_eq!(
            ResolvedTheme::resolve(ThemePreference::Dark, ResolvedColorScheme::Light),
            ResolvedTheme::DARK
        );
        assert_eq!(
            ResolvedTheme::resolve(ThemePreference::Light, ResolvedColorScheme::Dark),
            ResolvedTheme::LIGHT
        );
    }

    #[test]
    fn light_theme_text_contrast_is_aa() {
        let theme = ResolvedTheme::LIGHT;
        assert!(
            contrast_ratio(theme.text, theme.background) >= AA,
            "primary text"
        );
        assert!(
            contrast_ratio(theme.text_secondary, theme.background) >= AA,
            "secondary text"
        );
        assert!(
            contrast_ratio(theme.text_secondary, theme.surface) >= AA,
            "secondary text on surface"
        );
        assert!(
            contrast_ratio(theme.text, theme.surface) >= AA,
            "primary text on surface"
        );
        assert!(
            contrast_ratio(theme.on_accent, theme.accent) >= AA,
            "accent text"
        );
        assert!(
            contrast_ratio(theme.warning, theme.background) >= AA,
            "warning text"
        );
    }

    #[test]
    fn dark_theme_text_contrast_is_aa() {
        let theme = ResolvedTheme::DARK;
        assert!(
            contrast_ratio(theme.text, theme.background) >= AA,
            "primary text"
        );
        assert!(
            contrast_ratio(theme.text_secondary, theme.background) >= AA,
            "secondary text"
        );
        assert!(
            contrast_ratio(theme.text_secondary, theme.surface) >= AA,
            "secondary text on surface"
        );
        assert!(
            contrast_ratio(theme.text, theme.surface) >= AA,
            "primary text on surface"
        );
        assert!(
            contrast_ratio(theme.on_accent, theme.accent) >= AA,
            "accent text"
        );
        assert!(
            contrast_ratio(theme.warning, theme.background) >= AA,
            "warning text"
        );
    }

    #[test]
    fn selection_tint_is_distinct_from_background() {
        // The selection background must be visibly different from the plain
        // background so the keyboard cursor is discoverable; it must stay a
        // *tint* (not a saturated fill) so accessibility is not color-only.
        for theme in [ResolvedTheme::LIGHT, ResolvedTheme::DARK] {
            let background = luminance(theme.background);
            let selection = luminance(theme.selection);
            let delta = (selection - background).abs();
            assert!(delta > 0.01, "selection and background must differ");
        }
    }
}
