use eframe::egui::{self, Color32, CornerRadius, Stroke, Vec2};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl ThemeMode {
    pub fn as_u8(self) -> u8 {
        match self {
            ThemeMode::Dark => 0,
            ThemeMode::Light => 1,
        }
    }

    pub fn from_u8(v: u8) -> Self {
        if v == ThemeMode::Light.as_u8() {
            ThemeMode::Light
        } else {
            ThemeMode::Dark
        }
    }

    /// Display name for the settings UI and other user-facing selectors.
    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Dark => "Black & White (Dark)",
            ThemeMode::Light => "White & Black (Light)",
        }
    }
}

// Process-wide theme mode: 0 = Dark, 1 = Light.
static THEME_MODE: AtomicU8 = AtomicU8::new(0);

pub struct Theme;

impl Theme {
    pub fn mode() -> ThemeMode {
        ThemeMode::from_u8(THEME_MODE.load(Ordering::Relaxed))
    }

    pub fn set_mode(mode: ThemeMode) {
        THEME_MODE.store(mode.as_u8(), Ordering::Relaxed);
    }

    pub fn is_light() -> bool {
        Theme::mode() == ThemeMode::Light
    }

    // Surf brand blue stays constant in both modes.
    pub const fn primary() -> Color32 {
        Color32::from_rgb(46, 125, 230)
    }

    pub fn success() -> Color32 {
        if Self::is_light() {
            Color32::from_rgb(28, 126, 72)
        } else {
            Color32::from_rgb(46, 180, 100)
        }
    }

    pub fn warning() -> Color32 {
        if Self::is_light() {
            Color32::from_rgb(178, 116, 8)
        } else {
            Color32::from_rgb(230, 160, 40)
        }
    }

    pub fn danger() -> Color32 {
        if Self::is_light() {
            Color32::from_rgb(198, 48, 48)
        } else {
            Color32::from_rgb(220, 60, 60)
        }
    }

    pub fn bg_card() -> Color32 {
        if Self::is_light() {
            Color32::WHITE
        } else {
            Color32::from_rgb(28, 30, 36)
        }
    }

    pub fn bg_card_light() -> Color32 {
        if Self::is_light() {
            Color32::from_rgb(240, 243, 248)
        } else {
            Color32::from_rgb(38, 42, 50)
        }
    }

    pub fn bg_page() -> Color32 {
        if Self::is_light() {
            Color32::from_rgb(246, 248, 251)
        } else {
            Color32::from_rgb(22, 24, 30)
        }
    }

    pub fn bg_sidebar() -> Color32 {
        if Self::is_light() {
            Color32::from_rgb(240, 243, 248)
        } else {
            Color32::from_rgb(18, 20, 26)
        }
    }

    pub fn divider() -> Color32 {
        if Self::is_light() {
            Color32::from_rgb(222, 227, 236)
        } else {
            Color32::from_rgb(45, 48, 58)
        }
    }

    pub fn text_strong() -> Color32 {
        if Self::is_light() {
            Color32::from_rgb(18, 22, 30)
        } else {
            Color32::WHITE
        }
    }

    pub fn text_muted() -> Color32 {
        if Self::is_light() {
            Color32::from_rgb(104, 111, 128)
        } else {
            Color32::from_rgb(150, 155, 170)
        }
    }

    /// Fill color used by sidebar navigation items.
    pub fn nav_fill(active: bool) -> Color32 {
        if active {
            Self::primary()
        } else {
            Color32::TRANSPARENT
        }
    }

    /// Text color placed on top of primary-colored fills.
    pub fn on_primary() -> Color32 {
        Color32::WHITE
    }

    pub fn card_frame() -> egui::Frame {
        egui::Frame::new()
            .fill(Self::bg_card())
            .corner_radius(CornerRadius::same(8))
            .stroke(Stroke::new(1.0, Self::divider()))
            .inner_margin(egui::Margin::same(12))
    }

    pub fn status_badge(ui: &mut egui::Ui, text: &str, color: Color32) {
        let padding = Vec2::new(8.0, 3.0);
        let font_id = egui::TextStyle::Body.resolve(ui.style());
        let galley = ui
            .painter()
            .layout_no_wrap(text.to_string(), font_id, Color32::WHITE);
        let (rect, _response) =
            ui.allocate_exact_size(galley.size() + padding * 2.0, egui::Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::same(4), color);
        ui.painter()
            .galley(rect.min + padding, galley, Color32::WHITE);
    }

    /// Applies complete egui visuals for the current mode.
    pub fn apply_visuals(ctx: &egui::Context) {
        let mut vis = if Self::is_light() {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };
        vis.panel_fill = Self::bg_page();
        vis.window_fill = Self::bg_card();
        vis.extreme_bg_color = if Self::is_light() {
            Color32::from_rgb(233, 237, 243)
        } else {
            Color32::from_rgb(14, 15, 19)
        };
        vis.faint_bg_color = if Self::is_light() {
            Color32::from_rgb(240, 243, 248)
        } else {
            Color32::from_rgb(26, 28, 34)
        };
        vis.override_text_color = Some(Self::text_strong());
        vis.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Self::divider());
        vis.widgets.inactive.bg_fill = Self::bg_card_light();
        vis.selection.bg_fill = Self::primary().gamma_multiply(0.35);
        ctx.set_visuals(vis);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_roundtrip() {
        Theme::set_mode(ThemeMode::Light);
        assert!(Theme::is_light());
        assert_eq!(Theme::mode().as_u8(), 1);
        Theme::set_mode(ThemeMode::Dark);
        assert!(!Theme::is_light());

        // Brand color is constant across modes.
        assert_eq!(Theme::primary(), Color32::from_rgb(46, 125, 230));
        // Light mode cards are white; dark mode keeps original palette.
        Theme::set_mode(ThemeMode::Light);
        assert_eq!(Theme::bg_card(), Color32::WHITE);
        Theme::set_mode(ThemeMode::Dark);
        assert_eq!(Theme::bg_card(), Color32::from_rgb(28, 30, 36));
        assert_eq!(Theme::text_muted(), Color32::from_rgb(150, 155, 170));
    }
}
