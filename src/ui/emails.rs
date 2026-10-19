use eframe::egui::{self, RichText};
use uuid::Uuid;

use super::theme::Theme;
use crate::core::model::{CompanyProfile, Customer, DeliveryOrder, Invoice};
use crate::core::templates::{self, MessageLang};
use crate::db::repository::Repository;

#[derive(Debug, PartialEq, Eq)]
pub enum EmailMode {
    Invoice,
    ShippingStatus,
}

pub struct EmailCenterState {
    pub mode: EmailMode,
    pub lang: MessageLang,
    pub selected_customer_id: Option<Uuid>,
    pub selected_invoice_id: Option<Uuid>,
    pub selected_delivery_id: Option<Uuid>,
    pub shipping_status_type: String,
    pub recipient_email: String,
    pub email_subject: String,
    pub email_body: String,
    pub tracking_number: String,
    pub carrier_name: String,
    pub status_msg: Option<String>,
}

impl Default for EmailCenterState {
    fn default() -> Self {
        Self {
            mode: EmailMode::Invoice,
            lang: MessageLang::English,
            selected_customer_id: None,
            selected_invoice_id: None,
            selected_delivery_id: None,
            shipping_status_type: "Dispatched & En Route".to_string(),
            recipient_email: String::new(),
            email_subject: String::new(),
            email_body: String::new(),
            tracking_number: "TRK-982410-AE".to_string(),
            carrier_name: "Surf Express Freight".to_string(),
            status_msg: None,
        }
    }
}

pub fn render(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut EmailCenterState,
    notification: &mut Option<(String, bool)>,
) {
    ui.add_space(8.0);

    let customers = repo.get_customers().unwrap_or_default();
    let invoices = repo.get_invoices().unwrap_or_default();
    let deliveries = repo.get_delivery_orders().unwrap_or_default();
    let company = repo.get_company_profile();

    // Mode + Language Selector Header
    Theme::card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Client Communications Dispatch");
            ui.add_space(24.0);

            let is_inv = state.mode == EmailMode::Invoice;
            if ui.selectable_label(is_inv, "Send Invoice Email").clicked() {
                state.mode = EmailMode::Invoice;
                update_invoice_email_template(state, &customers, &invoices, &company);
            }

            let is_ship = state.mode == EmailMode::ShippingStatus;
            if ui
                .selectable_label(is_ship, "Send Shipping Update")
                .clicked()
            {
                state.mode = EmailMode::ShippingStatus;
                update_shipping_email_template(state, &deliveries, &company);
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Language:")
                    .size(12.0)
                    .color(Theme::text_muted()),
            );
            let lang_before = state.lang;
            egui::ComboBox::from_id_salt("email_lang_combo")
                .selected_text(state.lang.name())
                .width(130.0)
                .show_ui(ui, |ui| {
                    for l in MessageLang::ALL {
                        let is_sel = state.lang == l;
                        if ui.selectable_label(is_sel, l.name()).clicked() {
                            state.lang = l;
                        }
                    }
                });
            if lang_before != state.lang {
                // Regenerate the ready text in the newly selected language.
                match state.mode {
                    EmailMode::Invoice => {
                        update_invoice_email_template(state, &customers, &invoices, &company)
                    }
                    EmailMode::ShippingStatus => {
                        update_shipping_email_template(state, &deliveries, &company)
                    }
                }
            }
        });
    });

    ui.add_space(16.0);

    ui.columns(2, |cols| {
        // --- Left: Selection & Configuration Panel ---
        Theme::card_frame().show(&mut cols[0], |ui| {
            match state.mode {
                EmailMode::Invoice => {
                    ui.heading("Invoice Email Settings");
                    ui.separator();

                    egui::Grid::new("invoice_email_selectors")
                        .num_columns(2)
                        .spacing([12.0, 10.0])
                        .show(ui, |ui| {
                            ui.label("Select Invoice:");
                            let current_inv_text = invoices
                                .iter()
                                .find(|i| Some(i.id) == state.selected_invoice_id)
                                .map(|i| format!("{} ({})", i.invoice_number, i.customer_name))
                                .unwrap_or_else(|| "Select an Invoice".to_string());

                            egui::ComboBox::from_id_salt("email_inv_combo")
                                .selected_text(current_inv_text)
                                .show_ui(ui, |ui| {
                                    for inv in &invoices {
                                        let is_sel = Some(inv.id) == state.selected_invoice_id;
                                        let label = format!(
                                            "{} ({})",
                                            inv.invoice_number, inv.customer_name
                                        );
                                        if ui.selectable_label(is_sel, label).clicked() {
                                            state.selected_invoice_id = Some(inv.id);
                                            state.selected_customer_id = Some(inv.customer_id);
                                            update_invoice_email_template(
                                                state, &customers, &invoices, &company,
                                            );
                                        }
                                    }
                                });
                            ui.end_row();

                            ui.label("Recipient Email:");
                            ui.text_edit_singleline(&mut state.recipient_email);
                            ui.end_row();

                            ui.label("Subject Line:");
                            ui.text_edit_singleline(&mut state.email_subject);
                            ui.end_row();
                        });

                    ui.add_space(12.0);
                    ui.label(RichText::new("Attachments Included:").strong());
                    ui.label(
                        RichText::new("• Vector PDF invoice (wire info, official seal)")
                            .color(Theme::success())
                            .size(11.0),
                    );
                    ui.label(
                        RichText::new("• High-resolution PNG invoice graphic")
                            .color(Theme::success())
                            .size(11.0),
                    );
                }
                EmailMode::ShippingStatus => {
                    ui.heading("Shipping & Handling Status Settings");
                    ui.separator();

                    egui::Grid::new("shipping_email_selectors")
                        .num_columns(2)
                        .spacing([12.0, 10.0])
                        .show(ui, |ui| {
                            ui.label("Select Delivery Order:");
                            let current_do_text = deliveries
                                .iter()
                                .find(|d| Some(d.id) == state.selected_delivery_id)
                                .map(|d| format!("{} ({})", d.delivery_number, d.customer_name))
                                .unwrap_or_else(|| "Select a Delivery Order".to_string());

                            egui::ComboBox::from_id_salt("email_do_combo")
                                .selected_text(current_do_text)
                                .show_ui(ui, |ui| {
                                    for d in &deliveries {
                                        let is_sel = Some(d.id) == state.selected_delivery_id;
                                        let label =
                                            format!("{} ({})", d.delivery_number, d.customer_name);
                                        if ui.selectable_label(is_sel, label).clicked() {
                                            state.selected_delivery_id = Some(d.id);
                                            state.selected_customer_id = Some(d.customer_id);
                                            update_shipping_email_template(
                                                state,
                                                &deliveries,
                                                &company,
                                            );
                                        }
                                    }
                                });
                            ui.end_row();

                            ui.label("Shipping Event / Status:");
                            let statuses = [
                                "Dispatched & En Route",
                                "In Transit / Customs Cleared",
                                "Out for Delivery Today",
                                "Delivered Successfully",
                            ];
                            egui::ComboBox::from_id_salt("shipping_event_combo")
                                .selected_text(&state.shipping_status_type)
                                .show_ui(ui, |ui| {
                                    for s in statuses {
                                        let is_sel = state.shipping_status_type == s;
                                        if ui.selectable_label(is_sel, s).clicked() {
                                            state.shipping_status_type = s.to_string();
                                            update_shipping_email_template(
                                                state,
                                                &deliveries,
                                                &company,
                                            );
                                        }
                                    }
                                });
                            ui.end_row();

                            ui.label("Carrier / Driver:");
                            ui.text_edit_singleline(&mut state.carrier_name);
                            ui.end_row();

                            ui.label("Tracking Reference:");
                            ui.text_edit_singleline(&mut state.tracking_number);
                            ui.end_row();

                            ui.label("Recipient Email:");
                            ui.text_edit_singleline(&mut state.recipient_email);
                            ui.end_row();
                        });
                }
            }

            ui.add_space(20.0);
            ui.separator();

            // Dispatch Buttons
            ui.horizontal(|ui| {
                if ui
                    .button(
                        RichText::new("Launch Email Client (mailto:)")
                            .color(Theme::text_strong())
                            .strong(),
                    )
                    .clicked()
                {
                    let mailto_url = format!(
                        "mailto:{}?subject={}&body={}",
                        percent_encode_bytes(&state.recipient_email),
                        percent_encode_bytes(&state.email_subject),
                        percent_encode_bytes(&state.email_body)
                    );
                    ui.ctx().open_url(egui::OpenUrl::same_tab(&mailto_url));
                    *notification = Some((
                        "Launched default email application with the pre-filled message!"
                            .to_string(),
                        true,
                    ));
                }

                if ui.button("Copy Message Text").clicked() {
                    ui.ctx().copy_text(state.email_body.clone());
                    *notification = Some(("Message text copied to clipboard!".to_string(), true));
                }
            });
        });

        // --- Right: Live Message Preview & Editor ---
        Theme::card_frame().show(&mut cols[1], |ui| {
            ui.horizontal(|ui| {
                ui.heading("Message Preview & Customization");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("Editable before sending")
                            .size(11.0)
                            .color(Theme::text_muted()),
                    );
                });
            });
            ui.separator();

            ui.horizontal(|ui| {
                ui.label(RichText::new("To:").strong());
                ui.label(RichText::new(&state.recipient_email).color(Theme::primary()));
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Subject:").strong());
                ui.label(RichText::new(&state.email_subject).color(Theme::text_strong()));
            });
            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .max_height(ui.available_height() - 20.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut state.email_body)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(24)
                            .desired_width(f32::INFINITY),
                    );
                });
        });
    });
}

fn update_invoice_email_template(
    state: &mut EmailCenterState,
    customers: &[Customer],
    invoices: &[Invoice],
    company: &CompanyProfile,
) {
    let inv = invoices
        .iter()
        .find(|i| Some(i.id) == state.selected_invoice_id);
    let cust = customers
        .iter()
        .find(|c| Some(c.id) == state.selected_customer_id);
    // Fall back to the invoice's own customer when no selection was made.
    let fallback_customer = inv
        .and_then(|i| customers.iter().find(|c| c.id == i.customer_id))
        .cloned();

    if let Some(invoice) = inv {
        state.email_subject = templates::invoice_subject(state.lang, invoice, company);
        state.email_body = templates::compose_invoice_message(state.lang, invoice, company);
        let recipient = cust
            .or(fallback_customer.as_ref())
            .map(|c| c.email.clone())
            .unwrap_or_default();
        state.recipient_email = recipient;
    }
}

fn update_shipping_email_template(
    state: &mut EmailCenterState,
    deliveries: &[DeliveryOrder],
    company: &CompanyProfile,
) {
    let d_opt = deliveries
        .iter()
        .find(|d| Some(d.id) == state.selected_delivery_id);

    if let Some(d) = d_opt {
        state.email_subject =
            templates::shipping_subject(state.lang, d, &state.shipping_status_type);
        state.email_body = templates::compose_shipping_message(
            state.lang,
            d,
            &state.shipping_status_type,
            &state.carrier_name,
            &state.tracking_number,
            company,
        );
    }
}

fn percent_encode_bytes(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push_str("%20"),
            b'\n' => out.push_str("%0D%0A"),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mailto_encoding_matches_spec() {
        assert_eq!(percent_encode_bytes("a b"), "a%20b");
        assert_eq!(percent_encode_bytes("x\ny"), "x%0D%0Ay");
        assert_eq!(percent_encode_bytes("$100"), "%24100");
    }
}
