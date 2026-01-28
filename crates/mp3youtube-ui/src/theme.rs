//! Theme configuration for MP3YouTube.
//!
//! Provides dark mode colors with pastel and neon accents.

/// Color palette for the application.
pub mod colors {
    /// Background colors (dark mode).
    pub mod background {
        /// Primary background color.
        pub const PRIMARY: &str = "#121212";
        /// Secondary/elevated background.
        pub const SECONDARY: &str = "#1E1E1E";
        /// Tertiary/card background.
        pub const TERTIARY: &str = "#2D2D2D";
    }

    /// Text colors.
    pub mod text {
        /// Primary text color.
        pub const PRIMARY: &str = "#FFFFFF";
        /// Secondary/muted text.
        pub const SECONDARY: &str = "#B3B3B3";
        /// Disabled text.
        pub const DISABLED: &str = "#666666";
    }

    /// Accent colors (pastel/neon).
    pub mod accent {
        /// Primary accent - neon cyan.
        pub const PRIMARY: &str = "#00FFFF";
        /// Secondary accent - pastel pink.
        pub const SECONDARY: &str = "#FF6B9D";
        /// Tertiary accent - pastel purple.
        pub const TERTIARY: &str = "#B388FF";
        /// Success - pastel green.
        pub const SUCCESS: &str = "#69F0AE";
        /// Warning - pastel orange.
        pub const WARNING: &str = "#FFD180";
        /// Error - pastel red.
        pub const ERROR: &str = "#FF8A80";
    }

    /// Border colors.
    pub mod border {
        /// Default border.
        pub const DEFAULT: &str = "#404040";
        /// Focused border.
        pub const FOCUSED: &str = "#00FFFF";
    }
}

/// Typography configuration.
pub mod typography {
    /// Font family for the application.
    pub const FONT_FAMILY: &str = "'Fira Sans', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif";

    /// Font sizes.
    pub mod sizes {
        /// Extra small text.
        pub const XS: &str = "0.75rem";
        /// Small text.
        pub const SM: &str = "0.875rem";
        /// Base text.
        pub const BASE: &str = "1rem";
        /// Large text.
        pub const LG: &str = "1.125rem";
        /// Extra large text.
        pub const XL: &str = "1.25rem";
        /// 2x extra large text.
        pub const XXL: &str = "1.5rem";
        /// Heading.
        pub const HEADING: &str = "2rem";
    }
}

/// Spacing values.
pub mod spacing {
    /// Extra small spacing.
    pub const XS: &str = "0.25rem";
    /// Small spacing.
    pub const SM: &str = "0.5rem";
    /// Medium spacing.
    pub const MD: &str = "1rem";
    /// Large spacing.
    pub const LG: &str = "1.5rem";
    /// Extra large spacing.
    pub const XL: &str = "2rem";
}

/// Border radius values.
pub mod radius {
    /// Small radius.
    pub const SM: &str = "0.25rem";
    /// Medium radius.
    pub const MD: &str = "0.5rem";
    /// Large radius.
    pub const LG: &str = "1rem";
    /// Full/pill radius.
    pub const FULL: &str = "9999px";
}

/// Generate CSS custom properties for the theme.
#[must_use]
pub fn generate_css_variables() -> String {
    format!(
        r#":root {{
  /* Background colors */
  --bg-primary: {bg_primary};
  --bg-secondary: {bg_secondary};
  --bg-tertiary: {bg_tertiary};

  /* Text colors */
  --text-primary: {text_primary};
  --text-secondary: {text_secondary};
  --text-disabled: {text_disabled};

  /* Accent colors */
  --accent-primary: {accent_primary};
  --accent-secondary: {accent_secondary};
  --accent-tertiary: {accent_tertiary};
  --accent-success: {accent_success};
  --accent-warning: {accent_warning};
  --accent-error: {accent_error};

  /* Border colors */
  --border-default: {border_default};
  --border-focused: {border_focused};

  /* Typography */
  --font-family: {font_family};
  --font-size-xs: {font_xs};
  --font-size-sm: {font_sm};
  --font-size-base: {font_base};
  --font-size-lg: {font_lg};
  --font-size-xl: {font_xl};
  --font-size-xxl: {font_xxl};
  --font-size-heading: {font_heading};

  /* Spacing */
  --spacing-xs: {spacing_xs};
  --spacing-sm: {spacing_sm};
  --spacing-md: {spacing_md};
  --spacing-lg: {spacing_lg};
  --spacing-xl: {spacing_xl};

  /* Border radius */
  --radius-sm: {radius_sm};
  --radius-md: {radius_md};
  --radius-lg: {radius_lg};
  --radius-full: {radius_full};
}}"#,
        bg_primary = colors::background::PRIMARY,
        bg_secondary = colors::background::SECONDARY,
        bg_tertiary = colors::background::TERTIARY,
        text_primary = colors::text::PRIMARY,
        text_secondary = colors::text::SECONDARY,
        text_disabled = colors::text::DISABLED,
        accent_primary = colors::accent::PRIMARY,
        accent_secondary = colors::accent::SECONDARY,
        accent_tertiary = colors::accent::TERTIARY,
        accent_success = colors::accent::SUCCESS,
        accent_warning = colors::accent::WARNING,
        accent_error = colors::accent::ERROR,
        border_default = colors::border::DEFAULT,
        border_focused = colors::border::FOCUSED,
        font_family = typography::FONT_FAMILY,
        font_xs = typography::sizes::XS,
        font_sm = typography::sizes::SM,
        font_base = typography::sizes::BASE,
        font_lg = typography::sizes::LG,
        font_xl = typography::sizes::XL,
        font_xxl = typography::sizes::XXL,
        font_heading = typography::sizes::HEADING,
        spacing_xs = spacing::XS,
        spacing_sm = spacing::SM,
        spacing_md = spacing::MD,
        spacing_lg = spacing::LG,
        spacing_xl = spacing::XL,
        radius_sm = radius::SM,
        radius_md = radius::MD,
        radius_lg = radius::LG,
        radius_full = radius::FULL,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_css_variables() {
        let css = generate_css_variables();
        assert!(css.contains(":root"));
        assert!(css.contains("--bg-primary"));
        assert!(css.contains("--accent-primary"));
        assert!(css.contains("--font-family"));
    }

    #[test]
    fn test_color_values() {
        assert!(colors::background::PRIMARY.starts_with('#'));
        assert!(colors::accent::PRIMARY.starts_with('#'));
    }
}
