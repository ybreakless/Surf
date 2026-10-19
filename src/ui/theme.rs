use eframe::egui::{self, Color32, CornerRadius, Stroke, Vec2};

pub struct Theme;

impl Theme {
    pub const PRIMARY: Color32 = Color32::from_rgb(46, 125, 230); // Surf Blue
    pub const SUCCESS: Color32 = Color32::from_rgb(46, 180, 100); // Green
    pub const WARNING: Color32 = Color32::from_rgb(230, 160, 40); // Orange
    pub const DANGER: Color32 = Color32::from_rgb(220, 60, 60); // Red
    pub const BG_CARD: Color32 = Color32::from_rgb(28, 30, 36);
    pub const BG_CARD_LIGHT: Color32 = Color32::from_rgb(38, 42, 50);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(150, 155, 170);

    pub fn card_frame() -> egui::Frame {
        egui::Frame::new()
            .fill(Self::BG_CARD)
            .corner_radius(CornerRadius::same(8))
            .stroke(Stroke::new(1.0, Color32::from_rgb(45, 48, 58)))
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
}
