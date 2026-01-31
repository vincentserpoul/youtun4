//! Theme configuration for `MP3YouTube`.
//!
//! Modern dark mode with vibrant accent colors.
//! Designed for performance: system fonts, minimal shadows, GPU-accelerated animations.

/// Color palette for the application.
pub mod colors {
    /// Background colors (dark mode with subtle depth).
    pub mod background {
        /// Primary background - deep dark.
        pub const PRIMARY: &str = "#0f0f12";
        /// Secondary/elevated background.
        pub const SECONDARY: &str = "#18181b";
        /// Tertiary/card background.
        pub const TERTIARY: &str = "#27272a";
        /// Hover state background.
        pub const HOVER: &str = "#3f3f46";
    }

    /// Text colors.
    pub mod text {
        /// Primary text color.
        pub const PRIMARY: &str = "#fafafa";
        /// Secondary/muted text.
        pub const SECONDARY: &str = "#a1a1aa";
        /// Disabled text.
        pub const DISABLED: &str = "#52525b";
    }

    /// Accent colors (vibrant and modern).
    pub mod accent {
        /// Primary accent - indigo/violet.
        pub const PRIMARY: &str = "#8b5cf6";
        /// Secondary accent - pink/rose.
        pub const SECONDARY: &str = "#ec4899";
        /// Tertiary accent - cyan.
        pub const TERTIARY: &str = "#06b6d4";
        /// Success - emerald green.
        pub const SUCCESS: &str = "#10b981";
        /// Warning - amber.
        pub const WARNING: &str = "#f59e0b";
        /// Error - red.
        pub const ERROR: &str = "#ef4444";
        /// Info - blue.
        pub const INFO: &str = "#3b82f6";
    }

    /// Border colors.
    pub mod border {
        /// Default border - subtle.
        pub const DEFAULT: &str = "#3f3f46";
        /// Focused border.
        pub const FOCUSED: &str = "#8b5cf6";
        /// Subtle border for cards.
        pub const SUBTLE: &str = "#27272a";
    }

    /// Shadow/overlay colors (minimal for performance).
    pub mod shadow {
        /// Primary shadow (violet).
        pub const PRIMARY_GLOW: &str = "rgba(139, 92, 246, 0.25)";
        /// Secondary shadow (pink).
        pub const SECONDARY_GLOW: &str = "rgba(236, 72, 153, 0.25)";
        /// Tertiary shadow (cyan).
        pub const TERTIARY_GLOW: &str = "rgba(6, 182, 212, 0.25)";
        /// Success shadow.
        pub const SUCCESS_GLOW: &str = "rgba(16, 185, 129, 0.25)";
        /// Warning shadow.
        pub const WARNING_GLOW: &str = "rgba(245, 158, 11, 0.25)";
        /// Error shadow.
        pub const ERROR_GLOW: &str = "rgba(239, 68, 68, 0.25)";
        /// Overlay background.
        pub const OVERLAY: &str = "rgba(0, 0, 0, 0.75)";
    }

    /// Gradient definitions.
    pub mod gradient {
        /// Brand gradient (primary to secondary).
        pub const BRAND: &str = "linear-gradient(135deg, #8b5cf6 0%, #ec4899 100%)";
        /// Success gradient.
        pub const SUCCESS: &str = "linear-gradient(135deg, #10b981 0%, #06b6d4 100%)";
        /// Warm gradient.
        pub const WARM: &str = "linear-gradient(135deg, #f59e0b 0%, #ef4444 100%)";
    }
}

/// Typography configuration.
pub mod typography {
    /// Font family - Inter for body, Space Grotesk for headings.
    pub const FONT_FAMILY: &str =
        "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif";
    /// Heading font family - geometric and modern.
    pub const FONT_FAMILY_HEADING: &str =
        "'Space Grotesk', 'Inter', -apple-system, BlinkMacSystemFont, sans-serif";

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
pub fn generate_css_variables() -> String {
    format!(
        r":root {{
  /* Background colors */
  --bg-primary: {bg_primary};
  --bg-secondary: {bg_secondary};
  --bg-tertiary: {bg_tertiary};
  --bg-hover: {bg_hover};

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
  --accent-info: {accent_info};

  /* Gradients */
  --gradient-brand: {gradient_brand};
  --gradient-success: {gradient_success};
  --gradient-warm: {gradient_warm};

  /* Border colors */
  --border-default: {border_default};
  --border-focused: {border_focused};
  --border-subtle: {border_subtle};

  /* Shadow/Glow colors */
  --shadow-primary: {shadow_primary};
  --shadow-secondary: {shadow_secondary};
  --shadow-tertiary: {shadow_tertiary};
  --shadow-success: {shadow_success};
  --shadow-warning: {shadow_warning};
  --shadow-error: {shadow_error};
  --overlay-bg: {overlay_bg};

  /* Typography */
  --font-family: {font_family};
  --font-family-heading: {font_family_heading};
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

  /* Transitions (GPU-accelerated) */
  --transition-fast: 0.15s ease;
  --transition-normal: 0.2s ease;
  --transition-slow: 0.3s ease;
}}",
        bg_primary = colors::background::PRIMARY,
        bg_secondary = colors::background::SECONDARY,
        bg_tertiary = colors::background::TERTIARY,
        bg_hover = colors::background::HOVER,
        text_primary = colors::text::PRIMARY,
        text_secondary = colors::text::SECONDARY,
        text_disabled = colors::text::DISABLED,
        accent_primary = colors::accent::PRIMARY,
        accent_secondary = colors::accent::SECONDARY,
        accent_tertiary = colors::accent::TERTIARY,
        accent_success = colors::accent::SUCCESS,
        accent_warning = colors::accent::WARNING,
        accent_error = colors::accent::ERROR,
        accent_info = colors::accent::INFO,
        gradient_brand = colors::gradient::BRAND,
        gradient_success = colors::gradient::SUCCESS,
        gradient_warm = colors::gradient::WARM,
        border_default = colors::border::DEFAULT,
        border_focused = colors::border::FOCUSED,
        border_subtle = colors::border::SUBTLE,
        shadow_primary = colors::shadow::PRIMARY_GLOW,
        shadow_secondary = colors::shadow::SECONDARY_GLOW,
        shadow_tertiary = colors::shadow::TERTIARY_GLOW,
        shadow_success = colors::shadow::SUCCESS_GLOW,
        shadow_warning = colors::shadow::WARNING_GLOW,
        shadow_error = colors::shadow::ERROR_GLOW,
        overlay_bg = colors::shadow::OVERLAY,
        font_family = typography::FONT_FAMILY,
        font_family_heading = typography::FONT_FAMILY_HEADING,
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
