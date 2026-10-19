use eframe::egui::{self, Color32, CornerRadius, RichText};
use std::path::Path;

use super::theme::Theme;
use crate::core::model::CompanyProfile;
use crate::db::repository::Repository;

#[derive(Default)]
pub struct CompanyFormState {
    pub profile: CompanyProfile,
    pub is_saved: bool,
    pub status_msg: Option<String>,
}

pub fn render(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut CompanyFormState,
    notification: &mut Option<(String, bool)>,
) {
    ui.add_space(8.0);

    // Load from DB once
    if !state.is_saved {
        state.profile = repo.get_company_profile();
        state.is_saved = true;
    }

    ui.columns(2, |cols| {
        // --- Left: Company Identity Form ---
        Theme::card_frame().show(&mut cols[0], |ui| {
            ui.horizontal(|ui| {
                ui.heading("Company Profile & Business Identity");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("Save Changes").color(Theme::text_strong()).strong()).clicked() {
                        match repo.save_company_profile(&state.profile) {
                            Ok(_) => {
                                *notification = Some(("Company Profile updated successfully!".to_string(), true));
                                state.status_msg = Some("Saved to local database".to_string());
                            }
                            Err(e) => {
                                state.status_msg = Some(format!("Error saving profile: {}", e));
                            }
                        }
                    }
                });
            });
            ui.separator();

            egui::ScrollArea::vertical().max_height(580.0).show(ui, |ui| {
                egui::Grid::new("company_settings_grid")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Legal Entity Name:").strong());
                        ui.text_edit_singleline(&mut state.profile.legal_name);
                        ui.end_row();

                        ui.label("Trading / Brand Name:");
                        ui.text_edit_singleline(&mut state.profile.trading_name);
                        ui.end_row();

                        ui.label("Tax ID / TRN / VAT Reg:");
                        ui.text_edit_singleline(&mut state.profile.tax_id);
                        ui.end_row();

                        ui.label("Headquarters Address:");
                        ui.text_edit_singleline(&mut state.profile.address);
                        ui.end_row();

                        ui.label("City:");
                        ui.text_edit_singleline(&mut state.profile.city);
                        ui.end_row();

                        ui.label("Country:");
                        ui.text_edit_singleline(&mut state.profile.country);
                        ui.end_row();

                        ui.label("Postal Code:");
                        ui.text_edit_singleline(&mut state.profile.postal_code);
                        ui.end_row();

                        ui.label("Billing Email:");
                        ui.text_edit_singleline(&mut state.profile.email);
                        ui.end_row();

                        ui.label("Office Phone:");
                        ui.text_edit_singleline(&mut state.profile.phone);
                        ui.end_row();

                        ui.label("Website:");
                        ui.text_edit_singleline(&mut state.profile.website);
                        ui.end_row();
                    });

                ui.add_space(16.0);
                ui.heading("Bank & Wire Remittance Details");
                ui.separator();

                egui::Grid::new("company_bank_grid")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Bank Name:");
                        ui.text_edit_singleline(&mut state.profile.bank_name);
                        ui.end_row();

                        ui.label("IBAN / Account Number:");
                        ui.text_edit_singleline(&mut state.profile.bank_iban);
                        ui.end_row();

                        ui.label("SWIFT / BIC Code:");
                        ui.text_edit_singleline(&mut state.profile.bank_swift);
                        ui.end_row();
                    });

                ui.add_space(16.0);
                ui.heading("Company Stamp & Logo Assets");
                ui.separator();

                egui::Grid::new("company_assets_grid")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Company Stamp / Seal Path:");
                        ui.text_edit_singleline(&mut state.profile.stamp_path);
                        ui.end_row();

                        ui.label("Company Logo Path:");
                        ui.text_edit_singleline(&mut state.profile.logo_path);
                        ui.end_row();
                    });
            });
        });

        // --- Right: Live Document Header & Stamp Preview ---
        Theme::card_frame().show(&mut cols[1], |ui| {
            ui.heading("Official Invoice & Stamp Preview");
            ui.separator();
            ui.label(
                RichText::new("This is how your company identity, official stamp, and banking instructions appear on generated PDF and image invoices.")
                    .size(11.0)
                    .color(Theme::text_muted()),
            );
            ui.add_space(8.0);

            let preview_frame = egui::Frame::new()
                .fill(Color32::from_rgb(248, 250, 253))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(215, 222, 235)))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(egui::Margin::same(16));

            preview_frame.show(ui, |ui| {
                // Top blue strip
                let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(ui.available_width(), 4.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, CornerRadius::ZERO, Theme::primary());
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("TAX INVOICE").color(Color32::from_rgb(20, 25, 35)).strong().size(18.0));
                        ui.label(RichText::new("INV-100-ARF-26001").color(Theme::primary()).strong().size(12.0));
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&state.profile.legal_name).color(Color32::BLACK).strong().size(12.0));
                            ui.label(RichText::new(&state.profile.address).color(Color32::DARK_GRAY).size(10.0));
                            ui.label(RichText::new(format!("{}, {}", state.profile.city, state.profile.country)).color(Color32::DARK_GRAY).size(10.0));
                            ui.label(RichText::new(format!("TRN: {}", state.profile.tax_id)).color(Color32::DARK_GRAY).size(10.0));
                            ui.label(RichText::new(format!("Email: {}", state.profile.email)).color(Color32::DARK_GRAY).size(10.0));
                        });
                    });
                });

                ui.add_space(16.0);
                // Sample items table snippet
                let table_box = egui::Frame::new()
                    .fill(Color32::from_rgb(238, 242, 248))
                    .inner_margin(egui::Margin::symmetric(10, 6));
                table_box.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("#").color(Color32::BLACK).strong().size(10.0));
                        ui.add_space(10.0);
                        ui.label(RichText::new("Description").color(Color32::BLACK).strong().size(10.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new("Total ($)").color(Color32::BLACK).strong().size(10.0));
                            ui.label(RichText::new("Price").color(Color32::BLACK).strong().size(10.0));
                            ui.label(RichText::new("Qty").color(Color32::BLACK).strong().size(10.0));
                        });
                    });
                });

                ui.add_space(20.0);

                // Stamp preview & Bank wire preview
                ui.horizontal(|ui| {
                    // Stamp Box
                    ui.vertical(|ui| {
                        let stamp_box = egui::Frame::new()
                            .fill(Color32::from_rgb(255, 245, 245))
                            .stroke(egui::Stroke::new(1.5, Color32::from_rgb(215, 50, 50)))
                            .corner_radius(CornerRadius::same(50))
                            .inner_margin(egui::Margin::same(14));

                        stamp_box.show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new("★ OFFICIAL ★").color(Color32::from_rgb(200, 40, 40)).size(8.0).strong());
                                ui.label(RichText::new(&state.profile.trading_name).color(Color32::from_rgb(180, 30, 30)).size(9.0).strong());
                                ui.label(RichText::new("VERIFIED & APPROVED").color(Color32::from_rgb(200, 40, 40)).size(8.0).strong());
                                ui.label(RichText::new("SURF ACCOUNTING").color(Color32::from_rgb(150, 40, 40)).size(7.0));
                            });
                        });

                        if !state.profile.stamp_path.is_empty() {
                            let p = Path::new(&state.profile.stamp_path);
                            if p.exists() {
                                ui.label(RichText::new("✓ Custom stamp file detected").color(Theme::success()).size(10.0));
                            } else {
                                ui.label(RichText::new("ℹ Using high-trust vector seal").color(Theme::primary()).size(10.0));
                            }
                        }
                    });

                    ui.add_space(16.0);

                    // Bank Instructions Box
                    ui.vertical(|ui| {
                        let bank_box = egui::Frame::new()
                            .fill(Color32::from_rgb(240, 244, 250))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(200, 215, 235)))
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(egui::Margin::same(8));

                        bank_box.show(ui, |ui| {
                            ui.label(RichText::new("WIRE INSTRUCTIONS").color(Theme::primary()).strong().size(9.0));
                            ui.label(RichText::new(format!("Bank: {}", state.profile.bank_name)).color(Color32::BLACK).size(9.0));
                            ui.label(RichText::new(format!("IBAN: {}", state.profile.bank_iban)).color(Color32::BLACK).size(9.0));
                            ui.label(RichText::new(format!("SWIFT: {}", state.profile.bank_swift)).color(Color32::BLACK).size(9.0));
                        });
                    });
                });
            });
        });
    });
}
