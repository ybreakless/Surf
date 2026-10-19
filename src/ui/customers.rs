use chrono::Utc;
use eframe::egui::{self, Color32, RichText};
use uuid::Uuid;

use super::theme::Theme;
use crate::core::model::Customer;
use crate::core::numbering::NumberingEngine;
use crate::db::repository::Repository;

#[derive(Default)]
pub struct CustomerFormState {
    pub id: Option<Uuid>,
    pub name: String,
    pub custom_code: String,
    pub email: String,
    pub phone: String,
    pub address: String,
    pub city: String,
    pub country: String,
    pub postal_code: String,
    pub custom_zone: String,
    pub currency: String,
    pub notes: String,
    pub search_query: String,
    pub error_msg: Option<String>,
}

impl CustomerFormState {
    pub fn reset(&mut self) {
        self.id = None;
        self.name.clear();
        self.custom_code.clear();
        self.email.clear();
        self.phone.clear();
        self.address.clear();
        self.city.clear();
        self.country.clear();
        self.postal_code.clear();
        self.custom_zone.clear();
        self.currency = "EUR".to_string();
        self.notes.clear();
        self.error_msg = None;
    }

    pub fn load(&mut self, c: &Customer) {
        self.id = Some(c.id);
        self.name = c.name.clone();
        self.custom_code = c.code.clone();
        self.email = c.email.clone();
        self.phone = c.phone.clone();
        self.address = c.address.clone();
        self.city = c.city.clone();
        self.country = c.country.clone();
        self.postal_code = c.postal_code.clone();
        self.custom_zone = c.postal_zone.clone();
        self.currency = c.currency.clone();
        self.notes = c.notes.clone();
        self.error_msg = None;
    }
}

pub fn render(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut CustomerFormState,
    notification: &mut Option<(String, bool)>,
) {
    ui.add_space(8.0);

    let customers = repo.get_customers().unwrap_or_default();

    ui.columns(2, |cols| {
        // --- Left Column: Customer Directory ---
        Theme::card_frame().show(&mut cols[0], |ui| {
            ui.horizontal(|ui| {
                ui.heading("Customer Directory");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("+ New Customer").clicked() {
                        state.reset();
                    }
                });
            });
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.text_edit_singleline(&mut state.search_query);
                if !state.search_query.is_empty() && ui.small_button("Clear").clicked() {
                    state.search_query.clear();
                }
            });
            ui.separator();

            let query = state.search_query.to_lowercase();
            let filtered: Vec<&Customer> = customers
                .iter()
                .filter(|c| {
                    query.is_empty()
                        || c.name.to_lowercase().contains(&query)
                        || c.code.to_lowercase().contains(&query)
                        || c.city.to_lowercase().contains(&query)
                        || c.postal_code.to_lowercase().contains(&query)
                })
                .collect();

            if filtered.is_empty() {
                ui.label(RichText::new("No customers found.").color(Theme::TEXT_MUTED));
            } else {
                egui::ScrollArea::vertical()
                    .max_height(480.0)
                    .show(ui, |ui| {
                        for c in filtered {
                            let is_selected = state.id == Some(c.id);
                            let bg_color = if is_selected {
                                Theme::BG_CARD_LIGHT
                            } else {
                                Color32::TRANSPARENT
                            };

                            let frame = egui::Frame::new()
                                .fill(bg_color)
                                .inner_margin(egui::Margin::same(8))
                                .corner_radius(egui::CornerRadius::same(4));

                            frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(&c.name).strong());
                                            ui.label(
                                                RichText::new(format!("[{}]", c.code))
                                                    .color(Theme::PRIMARY)
                                                    .size(11.0),
                                            );
                                        });
                                        ui.label(
                                            RichText::new(format!(
                                                "📍 {}, {} | Zone: {}",
                                                c.city, c.country, c.postal_zone
                                            ))
                                            .size(12.0)
                                            .color(Theme::TEXT_MUTED),
                                        );
                                        if !c.postal_code.is_empty() {
                                            ui.label(
                                                RichText::new(format!("Postal: {}", c.postal_code))
                                                    .size(11.0)
                                                    .color(Theme::TEXT_MUTED),
                                            );
                                        }
                                    });

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui.button("Edit").clicked() {
                                                state.load(c);
                                            }
                                        },
                                    );
                                });
                            });
                            ui.separator();
                        }
                    });
            }
        });

        // --- Right Column: Customer Details & Intelligent Numbering ---
        Theme::card_frame().show(&mut cols[1], |ui| {
            let is_editing = state.id.is_some();
            ui.heading(if is_editing {
                "Edit Customer"
            } else {
                "Create New Customer"
            });
            ui.separator();

            if let Some(err) = &state.error_msg {
                ui.label(RichText::new(err).color(Theme::DANGER).strong());
                ui.add_space(4.0);
            }

            // Quick Client Templates for rapid creation
            ui.horizontal(|ui| {
                ui.label(RichText::new("Quick Presets:").size(11.0).color(Theme::TEXT_MUTED));
                if ui.small_button("🏪 Local Retail").clicked() {
                    state.city = "Dubai".to_string();
                    state.country = "UAE".to_string();
                    state.postal_code = "00000".to_string();
                    state.currency = "AED".to_string();
                    state.notes = "Payment on Delivery (COD)".to_string();
                }
                if ui.small_button("🏢 Corporate Net 30").clicked() {
                    state.currency = "USD".to_string();
                    state.notes = "Net 30 days bank transfer terms.".to_string();
                }
                if ui.small_button("🚢 Global Freight").clicked() {
                    state.currency = "USD".to_string();
                    state.notes = "FOB Destination. Customs declaration required.".to_string();
                }
            });
            ui.add_space(6.0);

            egui::Grid::new("customer_form_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Customer / Business Name:");
                    ui.text_edit_singleline(&mut state.name);
                    ui.end_row();

                    ui.label("Email:");
                    ui.text_edit_singleline(&mut state.email);
                    ui.end_row();

                    ui.label("Phone:");
                    ui.text_edit_singleline(&mut state.phone);
                    ui.end_row();

                    ui.label("Street Address:");
                    ui.text_edit_singleline(&mut state.address);
                    ui.end_row();

                    ui.label("City:");
                    ui.text_edit_singleline(&mut state.city);
                    ui.end_row();

                    ui.label("Country:");
                    ui.text_edit_singleline(&mut state.country);
                    ui.end_row();

                    ui.label(RichText::new("Postal Code:").strong());
                    ui.text_edit_singleline(&mut state.postal_code);
                    ui.end_row();

                    ui.label("Currency:");
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut state.currency, "USD".to_string(), "USD ($)");
                        ui.selectable_value(&mut state.currency, "EUR".to_string(), "EUR (€)");
                        ui.selectable_value(&mut state.currency, "AED".to_string(), "AED (د.إ)");
                        ui.selectable_value(&mut state.currency, "CNY".to_string(), "CNY (¥)");
                        ui.selectable_value(&mut state.currency, "RUB".to_string(), "RUB (₽)");
                        ui.selectable_value(&mut state.currency, "IRR".to_string(), "IRR (﷼)");
                    });
                    ui.end_row();

                    ui.label("Notes / Terms:");
                    ui.text_edit_multiline(&mut state.notes);
                    ui.end_row();
                });

            ui.add_space(12.0);

            // --- Real-Time Intelligent Logistics & Numbering Box ---
            let active_code = if state.custom_code.trim().is_empty() {
                NumberingEngine::derive_customer_code(&state.name)
            } else {
                state.custom_code.trim().to_uppercase()
            };

            let active_zone = if state.custom_zone.trim().is_empty() {
                NumberingEngine::derive_postal_zone(&state.postal_code, &state.city)
            } else {
                state.custom_zone.trim().to_uppercase()
            };

            let next_inv_seq = repo
                .next_invoice_sequence(&active_zone, &active_code)
                .unwrap_or(1);
            let next_do_seq = repo
                .next_delivery_sequence(&active_zone, &active_code)
                .unwrap_or(1);

            let preview_inv = NumberingEngine::format_invoice_number(
                &active_zone,
                &active_code,
                next_inv_seq,
            );
            let preview_do = NumberingEngine::format_delivery_number(
                &active_zone,
                &active_code,
                next_do_seq,
            );

            let box_frame = egui::Frame::new()
                .fill(Color32::from_rgb(22, 28, 40))
                .stroke(egui::Stroke::new(1.0, Theme::PRIMARY))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::same(12));

            box_frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("⚡ LOGISTICS & NUMBERING LOGIC")
                            .color(Theme::PRIMARY)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new("Automated Regional Routing")
                                .size(11.0)
                                .color(Theme::TEXT_MUTED),
                        );
                    });
                });
                ui.separator();

                ui.columns(2, |c_box| {
                    c_box[0].horizontal(|ui| {
                        ui.label(RichText::new("Customer Code:").size(12.0));
                        ui.label(
                            RichText::new(&active_code)
                                .color(Color32::WHITE)
                                .strong(),
                        );
                    });
                    c_box[0].horizontal(|ui| {
                        ui.label(RichText::new("Postal Zone:").size(12.0));
                        ui.label(
                            RichText::new(&active_zone)
                                .color(Theme::SUCCESS)
                                .strong(),
                        );
                    });

                    c_box[1].horizontal(|ui| {
                        ui.label(RichText::new("Next Invoice:").size(12.0));
                        ui.label(
                            RichText::new(&preview_inv)
                                .color(Theme::PRIMARY)
                                .strong(),
                        );
                    });
                    c_box[1].horizontal(|ui| {
                        ui.label(RichText::new("Next Delivery:").size(12.0));
                        ui.label(
                            RichText::new(&preview_do)
                                .color(Theme::WARNING)
                                .strong(),
                        );
                    });
                });

                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Invoices and Delivery Orders are automatically prefixed with the regional postal zone and customer identifier.",
                    )
                    .size(11.0)
                    .color(Theme::TEXT_MUTED),
                );
            });

            ui.add_space(16.0);

            ui.horizontal(|ui| {
                if ui
                    .button(
                        RichText::new(if is_editing {
                            "Save Changes"
                        } else {
                            "Create Customer"
                        })
                        .color(Color32::WHITE)
                        .strong(),
                    )
                    .clicked()
                {
                    if state.name.trim().is_empty() {
                        state.error_msg = Some("Customer name cannot be empty".to_string());
                    } else {
                        let customer = Customer {
                            id: state.id.unwrap_or_else(Uuid::new_v4),
                            name: state.name.trim().to_string(),
                            code: active_code,
                            email: state.email.trim().to_string(),
                            phone: state.phone.trim().to_string(),
                            address: state.address.trim().to_string(),
                            city: state.city.trim().to_string(),
                            country: state.country.trim().to_string(),
                            postal_code: state.postal_code.trim().to_string(),
                            postal_zone: active_zone,
                            currency: state.currency.clone(),
                            notes: state.notes.trim().to_string(),
                            created_at: Utc::now(),
                        };

                        match repo.insert_customer(&customer, true) {
                            Ok(_) => {
                                *notification = Some((
                                    format!("Customer '{}' saved successfully!", customer.name),
                                    true,
                                ));
                                state.reset();
                            }
                            Err(e) => {
                                state.error_msg = Some(format!("Database error: {}", e));
                            }
                        }
                    }
                }

                if is_editing && ui.button("Cancel").clicked() {
                    state.reset();
                }
            });
        });
    });
}
