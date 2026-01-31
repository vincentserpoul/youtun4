//! Theme configuration for `Youtun4`.
//!
//! Modern dark mode with vibrant accent colors and glassmorphic elements.
//! Designed for performance: system fonts, subtle depth, GPU-accelerated animations.

/// Color palette for the application.
pub mod colors {
    /// Background colors (dark mode with refined depth).
    pub mod background {
        /// Primary background - rich dark with slight warmth.
        pub const PRIMARY: &str = "#09090b";
        /// Secondary/elevated background - subtle lift.
        pub const SECONDARY: &str = "#131316";
        /// Tertiary/card background - interactive surfaces.
        pub const TERTIARY: &str = "#1c1c21";
        /// Hover state background.
        pub const HOVER: &str = "#2a2a32";
        /// Glass effect background.
        pub const GLASS: &str = "rgba(25, 25, 30, 0.8)";
        /// Elevated surface with subtle glow.
        pub const ELEVATED: &str = "#1f1f24";
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
        /// Primary accent - refined violet with more saturation.
        pub const PRIMARY: &str = "#a78bfa";
        /// Primary accent darker variant for hover states.
        pub const PRIMARY_DIM: &str = "#7c3aed";
        /// Secondary accent - vibrant rose.
        pub const SECONDARY: &str = "#f472b6";
        /// Tertiary accent - electric cyan.
        pub const TERTIARY: &str = "#22d3ee";
        /// Success - vibrant emerald.
        pub const SUCCESS: &str = "#34d399";
        /// Warning - warm amber.
        pub const WARNING: &str = "#fbbf24";
        /// Error - soft coral red.
        pub const ERROR: &str = "#f87171";
        /// Info - sky blue.
        pub const INFO: &str = "#60a5fa";
    }

    /// Border colors.
    pub mod border {
        /// Default border - subtle and refined.
        pub const DEFAULT: &str = "rgba(255, 255, 255, 0.08)";
        /// Focused border.
        pub const FOCUSED: &str = "#a78bfa";
        /// Subtle border for cards.
        pub const SUBTLE: &str = "rgba(255, 255, 255, 0.04)";
        /// Strong border for emphasis.
        pub const STRONG: &str = "rgba(255, 255, 255, 0.12)";
    }

    /// Shadow/overlay colors (refined for depth).
    pub mod shadow {
        /// Primary shadow (violet) - more pronounced.
        pub const PRIMARY_GLOW: &str = "rgba(167, 139, 250, 0.3)";
        /// Secondary shadow (pink).
        pub const SECONDARY_GLOW: &str = "rgba(244, 114, 182, 0.25)";
        /// Tertiary shadow (cyan).
        pub const TERTIARY_GLOW: &str = "rgba(34, 211, 238, 0.25)";
        /// Success shadow.
        pub const SUCCESS_GLOW: &str = "rgba(52, 211, 153, 0.25)";
        /// Warning shadow.
        pub const WARNING_GLOW: &str = "rgba(251, 191, 36, 0.25)";
        /// Error shadow.
        pub const ERROR_GLOW: &str = "rgba(248, 113, 113, 0.25)";
        /// Overlay background - deeper for better contrast.
        pub const OVERLAY: &str = "rgba(0, 0, 0, 0.85)";
        /// Ambient shadow for floating elements.
        pub const AMBIENT: &str = "0 8px 32px rgba(0, 0, 0, 0.4)";
        /// Soft shadow for cards.
        pub const SOFT: &str = "0 4px 16px rgba(0, 0, 0, 0.2)";
    }

    /// Gradient definitions.
    pub mod gradient {
        /// Brand gradient (primary to secondary) - more vibrant.
        pub const BRAND: &str = "linear-gradient(135deg, #a78bfa 0%, #f472b6 100%)";
        /// Brand gradient subtle for backgrounds.
        pub const BRAND_SUBTLE: &str =
            "linear-gradient(135deg, rgba(167, 139, 250, 0.1) 0%, rgba(244, 114, 182, 0.1) 100%)";
        /// Success gradient - fresh and vibrant.
        pub const SUCCESS: &str = "linear-gradient(135deg, #34d399 0%, #22d3ee 100%)";
        /// Warm gradient - energetic.
        pub const WARM: &str = "linear-gradient(135deg, #fbbf24 0%, #f87171 100%)";
        /// Glass gradient for surfaces.
        pub const GLASS: &str =
            "linear-gradient(135deg, rgba(255, 255, 255, 0.05) 0%, rgba(255, 255, 255, 0.02) 100%)";
        /// Shimmer effect.
        pub const SHIMMER: &str = "linear-gradient(90deg, transparent 0%, rgba(255, 255, 255, 0.05) 50%, transparent 100%)";
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
    /// Extra small radius.
    pub const XS: &str = "0.25rem";
    /// Small radius.
    pub const SM: &str = "0.375rem";
    /// Medium radius.
    pub const MD: &str = "0.625rem";
    /// Large radius.
    pub const LG: &str = "1rem";
    /// Extra large radius.
    pub const XL: &str = "1.25rem";
    /// 2XL radius for panels.
    pub const XXL: &str = "1.5rem";
    /// Full/pill radius.
    pub const FULL: &str = "9999px";
}

/// Animation/transition configuration.
pub mod animation {
    /// Micro interaction - very fast feedback.
    pub const MICRO: &str = "0.1s cubic-bezier(0.4, 0, 0.2, 1)";
    /// Fast transition for interactive elements.
    pub const FAST: &str = "0.15s cubic-bezier(0.4, 0, 0.2, 1)";
    /// Normal transition for most UI changes.
    pub const NORMAL: &str = "0.2s cubic-bezier(0.4, 0, 0.2, 1)";
    /// Smooth transition for larger changes.
    pub const SMOOTH: &str = "0.3s cubic-bezier(0.4, 0, 0.2, 1)";
    /// Slow transition for dramatic effects.
    pub const SLOW: &str = "0.5s cubic-bezier(0.4, 0, 0.2, 1)";
    /// Spring-like bounce effect.
    pub const SPRING: &str = "0.4s cubic-bezier(0.34, 1.56, 0.64, 1)";
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
  --bg-glass: {bg_glass};
  --bg-elevated: {bg_elevated};

  /* Text colors */
  --text-primary: {text_primary};
  --text-secondary: {text_secondary};
  --text-disabled: {text_disabled};

  /* Accent colors */
  --accent-primary: {accent_primary};
  --accent-primary-dim: {accent_primary_dim};
  --accent-secondary: {accent_secondary};
  --accent-tertiary: {accent_tertiary};
  --accent-success: {accent_success};
  --accent-warning: {accent_warning};
  --accent-error: {accent_error};
  --accent-info: {accent_info};

  /* Gradients */
  --gradient-brand: {gradient_brand};
  --gradient-brand-subtle: {gradient_brand_subtle};
  --gradient-success: {gradient_success};
  --gradient-warm: {gradient_warm};
  --gradient-glass: {gradient_glass};
  --gradient-shimmer: {gradient_shimmer};

  /* Border colors */
  --border-default: {border_default};
  --border-focused: {border_focused};
  --border-subtle: {border_subtle};
  --border-strong: {border_strong};

  /* Shadow/Glow colors */
  --shadow-primary: {shadow_primary};
  --shadow-secondary: {shadow_secondary};
  --shadow-tertiary: {shadow_tertiary};
  --shadow-success: {shadow_success};
  --shadow-warning: {shadow_warning};
  --shadow-error: {shadow_error};
  --shadow-ambient: {shadow_ambient};
  --shadow-soft: {shadow_soft};
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
  --radius-xs: {radius_xs};
  --radius-sm: {radius_sm};
  --radius-md: {radius_md};
  --radius-lg: {radius_lg};
  --radius-xl: {radius_xl};
  --radius-xxl: {radius_xxl};
  --radius-full: {radius_full};

  /* Transitions (GPU-accelerated with refined easing) */
  --transition-micro: {transition_micro};
  --transition-fast: {transition_fast};
  --transition-normal: {transition_normal};
  --transition-smooth: {transition_smooth};
  --transition-slow: {transition_slow};
  --transition-spring: {transition_spring};
}}",
        bg_primary = colors::background::PRIMARY,
        bg_secondary = colors::background::SECONDARY,
        bg_tertiary = colors::background::TERTIARY,
        bg_hover = colors::background::HOVER,
        bg_glass = colors::background::GLASS,
        bg_elevated = colors::background::ELEVATED,
        text_primary = colors::text::PRIMARY,
        text_secondary = colors::text::SECONDARY,
        text_disabled = colors::text::DISABLED,
        accent_primary = colors::accent::PRIMARY,
        accent_primary_dim = colors::accent::PRIMARY_DIM,
        accent_secondary = colors::accent::SECONDARY,
        accent_tertiary = colors::accent::TERTIARY,
        accent_success = colors::accent::SUCCESS,
        accent_warning = colors::accent::WARNING,
        accent_error = colors::accent::ERROR,
        accent_info = colors::accent::INFO,
        gradient_brand = colors::gradient::BRAND,
        gradient_brand_subtle = colors::gradient::BRAND_SUBTLE,
        gradient_success = colors::gradient::SUCCESS,
        gradient_warm = colors::gradient::WARM,
        gradient_glass = colors::gradient::GLASS,
        gradient_shimmer = colors::gradient::SHIMMER,
        border_default = colors::border::DEFAULT,
        border_focused = colors::border::FOCUSED,
        border_subtle = colors::border::SUBTLE,
        border_strong = colors::border::STRONG,
        shadow_primary = colors::shadow::PRIMARY_GLOW,
        shadow_secondary = colors::shadow::SECONDARY_GLOW,
        shadow_tertiary = colors::shadow::TERTIARY_GLOW,
        shadow_success = colors::shadow::SUCCESS_GLOW,
        shadow_warning = colors::shadow::WARNING_GLOW,
        shadow_error = colors::shadow::ERROR_GLOW,
        shadow_ambient = colors::shadow::AMBIENT,
        shadow_soft = colors::shadow::SOFT,
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
        radius_xs = radius::XS,
        radius_sm = radius::SM,
        radius_md = radius::MD,
        radius_lg = radius::LG,
        radius_xl = radius::XL,
        radius_xxl = radius::XXL,
        radius_full = radius::FULL,
        transition_micro = animation::MICRO,
        transition_fast = animation::FAST,
        transition_normal = animation::NORMAL,
        transition_smooth = animation::SMOOTH,
        transition_slow = animation::SLOW,
        transition_spring = animation::SPRING,
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
