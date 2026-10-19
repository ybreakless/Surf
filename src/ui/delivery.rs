use chrono::{NaiveDate, Utc};
use eframe::egui::{self, RichText};
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

use super::theme::Theme;
use crate::core::model::{Customer, DeliveryItem, DeliveryOrder, DeliveryStatus};
use crate::core::numbering::NumberingEngine;
use crate::db::repository::Repository;

pub struct DeliveryItemFormState {
    pub id: Uuid,
    pub description: String,
    pub quantity: String,
    pub unit: String,
}

pub struct DeliveryFormState {
    pub is_open: bool,
    pub id: Option<Uuid>,
    pub delivery_number: String,
    pub selected_customer_id: Option<Uuid>,
    pub delivery_address: String,
    pub delivery_date_str: String,
    pub items: Vec<DeliveryItemFormState>,
    pub driver_notes: String,
    pub error_msg: Option<String>,
}

impl Default for DeliveryFormState {
    fn default() -> Self {
        let now = Utc::now().date_naive();
        Self {
            is_open: false,
            id: None,
            delivery_number: String::new(),
            selected_customer_id: None,
            delivery_address: String::new(),
            delivery_date_str: now.format("%Y-%m-%d").to_string(),
            items: vec![DeliveryItemFormState {
                id: Uuid::new_v4(),
                description: String::new(),
                quantity: "1".to_string(),
                unit: "Boxes".to_string(),
            }],
            driver_notes: String::new(),
            error_msg: None,
        }
    }
}

pub fn render(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut DeliveryFormState,
    notification: &mut Option<(String, bool)>,
) {
    ui.add_space(8.0);

    let customers = repo.get_customers().unwrap_or_default();
    let deliveries = repo.get_delivery_orders().unwrap_or_default();

    if state.is_open {
        render_delivery_form(ui, repo, state, &customers, notification);
    } else {
        render_delivery_list(ui, repo, state, &deliveries, &customers, notification);
    }
}

fn render_delivery_list(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut DeliveryFormState,
    deliveries: &[DeliveryOrder],
    customers: &[Customer],
    notification: &mut Option<(String, bool)>,
) {
    Theme::card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Delivery Orders & Logistics");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new("+ New Delivery Order").color(Theme::text_strong()).strong()).clicked() {
                    *state = DeliveryFormState::default();
                    state.is_open = true;
                    if let Some(first_cust) = customers.first() {
                        state.selected_customer_id = Some(first_cust.id);
                        state.delivery_address = first_cust.address.clone();
                        let next_seq = repo.next_delivery_sequence(&first_cust.postal_zone, &first_cust.code).unwrap_or(1);
                        state.delivery_number = NumberingEngine::format_delivery_number(&first_cust.postal_zone, &first_cust.code, next_seq);
                    }
                }
            });
        });
        ui.separator();

        if deliveries.is_empty() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No delivery orders created yet.").color(Theme::text_muted()));
                ui.label("Generate delivery orders directly from Invoices or click '+ New Delivery Order'.");
            });
            ui.add_space(16.0);
            return;
        }

        // Table Header
        egui::Grid::new("delivery_table_header")
            .num_columns(7)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(RichText::new("DO #").strong());
                ui.label(RichText::new("Zone").strong());
                ui.label(RichText::new("Customer").strong());
                ui.label(RichText::new("Destination").strong());
                ui.label(RichText::new("Date").strong());
                ui.label(RichText::new("Status").strong());
                ui.label(RichText::new("Action").strong());
                ui.end_row();
            });
        ui.separator();

        egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
            for d in deliveries {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&d.delivery_number).strong());
                    ui.label(RichText::new(format!("[{}]", d.postal_zone)).color(Theme::primary()).size(12.0));
                    ui.label(&d.customer_name);
                    ui.label(RichText::new(&d.delivery_address).color(Theme::text_muted()));
                    ui.label(d.delivery_date.to_string());

                    let badge_color = match d.status {
                        DeliveryStatus::Delivered => Theme::success(),
                        DeliveryStatus::InTransit => Theme::warning(),
                        DeliveryStatus::Pending => Theme::primary(),
                        DeliveryStatus::Cancelled => Theme::danger(),
                    };
                    Theme::status_badge(ui, d.status.as_str(), badge_color);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        match d.status {
                            DeliveryStatus::Pending if ui.button("Dispatch").clicked() => {
                                update_status(repo, d, DeliveryStatus::InTransit, notification);
                            }
                            DeliveryStatus::InTransit
                                if ui
                                    .button(RichText::new("Confirm Delivered").color(Theme::success()))
                                    .clicked() =>
                            {
                                update_status(repo, d, DeliveryStatus::Delivered, notification);
                            }
                            _ => {}
                        }
                    });
                });
                ui.separator();
            }
        });
    });
}

fn render_delivery_form(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut DeliveryFormState,
    customers: &[Customer],
    notification: &mut Option<(String, bool)>,
) {
    Theme::card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("New Delivery Order");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("← Back to List").clicked() {
                    state.is_open = false;
                }
            });
        });
        ui.separator();

        if let Some(err) = &state.error_msg {
            ui.label(RichText::new(err).color(Theme::danger()).strong());
            ui.add_space(4.0);
        }

        egui::Grid::new("do_form_top")
            .num_columns(4)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Customer:");
                let current_cust_name = customers
                    .iter()
                    .find(|c| Some(c.id) == state.selected_customer_id)
                    .map(|c| c.name.as_str())
                    .unwrap_or("Select a Customer");

                egui::ComboBox::from_id_salt("do_customer_combo")
                    .selected_text(current_cust_name)
                    .show_ui(ui, |ui| {
                        for c in customers {
                            let is_selected = Some(c.id) == state.selected_customer_id;
                            if ui.selectable_label(is_selected, &c.name).clicked() {
                                state.selected_customer_id = Some(c.id);
                                state.delivery_address = c.address.clone();
                                let next_seq = repo
                                    .next_delivery_sequence(&c.postal_zone, &c.code)
                                    .unwrap_or(1);
                                state.delivery_number = NumberingEngine::format_delivery_number(
                                    &c.postal_zone,
                                    &c.code,
                                    next_seq,
                                );
                            }
                        }
                    });

                ui.label(RichText::new("Generated DO #:").strong());
                ui.text_edit_singleline(&mut state.delivery_number);
                ui.end_row();

                ui.label("Delivery Destination:");
                ui.text_edit_singleline(&mut state.delivery_address);

                ui.label("Scheduled Date:");
                ui.text_edit_singleline(&mut state.delivery_date_str);
                ui.end_row();
            });

        ui.add_space(12.0);
        ui.heading("Packages / Goods Manifest");
        ui.separator();

        let mut to_remove = None;
        let num_items = state.items.len();
        for (idx, item) in state.items.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{}.", idx + 1));
                ui.add(
                    egui::TextEdit::singleline(&mut item.description)
                        .hint_text("Goods / Box description")
                        .desired_width(260.0),
                );
                ui.label("Qty:");
                ui.add(egui::TextEdit::singleline(&mut item.quantity).desired_width(50.0));
                ui.label("Unit:");
                ui.add(egui::TextEdit::singleline(&mut item.unit).desired_width(70.0));

                if num_items > 1 && ui.small_button("✕").clicked() {
                    to_remove = Some(idx);
                }
            });
            ui.add_space(4.0);
        }

        if let Some(idx) = to_remove {
            state.items.remove(idx);
        }

        if ui.button("+ Add Manifest Item").clicked() {
            state.items.push(DeliveryItemFormState {
                id: Uuid::new_v4(),
                description: String::new(),
                quantity: "1".to_string(),
                unit: "Pcs".to_string(),
            });
        }

        ui.add_space(12.0);
        ui.label("Driver & Routing Instructions:");
        ui.text_edit_multiline(&mut state.driver_notes);

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui
                .button(
                    RichText::new("Create & Dispatch DO")
                        .color(Theme::text_strong())
                        .strong(),
                )
                .clicked()
            {
                save_delivery_order(repo, state, customers, notification);
            }
            if ui.button("Cancel").clicked() {
                state.is_open = false;
            }
        });
    });
}

fn save_delivery_order(
    repo: &Repository,
    state: &mut DeliveryFormState,
    customers: &[Customer],
    notification: &mut Option<(String, bool)>,
) {
    let cust = match customers
        .iter()
        .find(|c| Some(c.id) == state.selected_customer_id)
    {
        Some(c) => c,
        None => {
            state.error_msg = Some("Please select a customer".to_string());
            return;
        }
    };

    if state.delivery_number.trim().is_empty() {
        state.error_msg = Some("Delivery number cannot be empty".to_string());
        return;
    }

    let delivery_date = match NaiveDate::parse_from_str(&state.delivery_date_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            state.error_msg = Some("Invalid Date format (use YYYY-MM-DD)".to_string());
            return;
        }
    };

    let items: Vec<DeliveryItem> = state
        .items
        .iter()
        .map(|it| {
            let q = Decimal::from_str(&it.quantity).unwrap_or(Decimal::ONE);
            DeliveryItem {
                id: it.id,
                description: it.description.clone(),
                quantity: q,
                unit: it.unit.clone(),
            }
        })
        .collect();

    let delivery = DeliveryOrder {
        id: state.id.unwrap_or_else(Uuid::new_v4),
        delivery_number: state.delivery_number.trim().to_string(),
        invoice_id: None,
        invoice_number: None,
        customer_id: cust.id,
        customer_name: cust.name.clone(),
        delivery_address: state.delivery_address.trim().to_string(),
        postal_code: cust.postal_code.clone(),
        postal_zone: cust.postal_zone.clone(),
        delivery_date,
        status: DeliveryStatus::Pending,
        items,
        driver_notes: state.driver_notes.trim().to_string(),
        created_at: Utc::now(),
    };

    match repo.insert_delivery_order(&delivery, true) {
        Ok(_) => {
            *notification = Some((
                format!(
                    "Delivery Order {} created successfully!",
                    delivery.delivery_number
                ),
                true,
            ));
            state.is_open = false;
        }
        Err(e) => {
            state.error_msg = Some(format!("Database error: {}", e));
        }
    }
}

fn update_status(
    repo: &Repository,
    d: &DeliveryOrder,
    new_status: DeliveryStatus,
    notification: &mut Option<(String, bool)>,
) {
    let mut updated = d.clone();
    updated.status = new_status;
    match repo.insert_delivery_order(&updated, true) {
        Ok(_) => {
            *notification = Some((
                format!(
                    "Delivery Order {} status updated to '{}'!",
                    d.delivery_number,
                    new_status.as_str()
                ),
                true,
            ));
        }
        Err(e) => {
            *notification = Some((format!("Error updating delivery status: {}", e), false));
        }
    }
}
