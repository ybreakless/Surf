use eframe::egui::{self, Color32, RichText};
use uuid::Uuid;

use super::theme::Theme;
use crate::core::model::{CompanyProfile, Customer, DeliveryOrder, Invoice};
use crate::db::repository::Repository;

#[derive(Debug, PartialEq, Eq)]
pub enum EmailMode {
    Invoice,
    ShippingStatus,
}

pub struct EmailCenterState {
    pub mode: EmailMode,
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

    // Mode Selector Header
    Theme::card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Email & Client Communications Dispatch");
            ui.add_space(24.0);

            let is_inv = state.mode == EmailMode::Invoice;
            if ui
                .selectable_label(is_inv, "📄 Send Invoice Email")
                .clicked()
            {
                state.mode = EmailMode::Invoice;
                update_invoice_email_template(state, &customers, &invoices, &company);
            }

            let is_ship = state.mode == EmailMode::ShippingStatus;
            if ui
                .selectable_label(is_ship, "🚚 Send Shipping & Handling Update")
                .clicked()
            {
                state.mode = EmailMode::ShippingStatus;
                update_shipping_email_template(state, &customers, &deliveries, &company);
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
                                .map(|i| {
                                    format!(
                                        "{} ({} - ${:.2})",
                                        i.invoice_number, i.customer_name, i.total_amount
                                    )
                                })
                                .unwrap_or_else(|| "Select an Invoice".to_string());

                            egui::ComboBox::from_id_salt("email_inv_combo")
                                .selected_text(current_inv_text)
                                .show_ui(ui, |ui| {
                                    for inv in &invoices {
                                        let is_sel = Some(inv.id) == state.selected_invoice_id;
                                        let label = format!(
                                            "{} ({} - ${:.2})",
                                            inv.invoice_number, inv.customer_name, inv.total_amount
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
                        RichText::new(
                            "• Vector PDF Invoice (with official company seal & wire info)",
                        )
                        .color(Theme::SUCCESS)
                        .size(11.0),
                    );
                    ui.label(
                        RichText::new("• High-Resolution PNG Graphic Invoice")
                            .color(Theme::SUCCESS)
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
                                .map(|d| {
                                    format!(
                                        "{} ({} - Zone {})",
                                        d.delivery_number, d.customer_name, d.postal_zone
                                    )
                                })
                                .unwrap_or_else(|| "Select a Delivery Order".to_string());

                            egui::ComboBox::from_id_salt("email_do_combo")
                                .selected_text(current_do_text)
                                .show_ui(ui, |ui| {
                                    for d in &deliveries {
                                        let is_sel = Some(d.id) == state.selected_delivery_id;
                                        let label = format!(
                                            "{} ({} - Zone {})",
                                            d.delivery_number, d.customer_name, d.postal_zone
                                        );
                                        if ui.selectable_label(is_sel, label).clicked() {
                                            state.selected_delivery_id = Some(d.id);
                                            state.selected_customer_id = Some(d.customer_id);
                                            update_shipping_email_template(
                                                state,
                                                &customers,
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
                                                &customers,
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
                        RichText::new("🚀 Launch Email Client (mailto:)")
                            .color(Color32::WHITE)
                            .strong(),
                    )
                    .clicked()
                {
                    let mailto_url = format!(
                        "mailto:{}?subject={}&body={}",
                        urlencoding::encode(&state.recipient_email),
                        urlencoding::encode(&state.email_subject),
                        urlencoding::encode(&state.email_body)
                    );
                    ui.ctx().open_url(egui::OpenUrl::same_tab(&mailto_url));
                    *notification = Some((
                        "Launched default email application with pre-filled message!".to_string(),
                        true,
                    ));
                }

                if ui.button("📋 Copy Message Text").clicked() {
                    ui.ctx().copy_text(state.email_body.clone());
                    *notification = Some(("Email message copied to clipboard!".to_string(), true));
                }
            });
        });

        // --- Right: Live Email Body Preview & Editor ---
        Theme::card_frame().show(&mut cols[1], |ui| {
            ui.horizontal(|ui| {
                ui.heading("Email Message Preview & Customization");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new("Editable before sending")
                            .size(11.0)
                            .color(Theme::TEXT_MUTED),
                    );
                });
            });
            ui.separator();

            ui.horizontal(|ui| {
                ui.label(RichText::new("To:").strong());
                ui.label(RichText::new(&state.recipient_email).color(Theme::PRIMARY));
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Subject:").strong());
                ui.label(RichText::new(&state.email_subject).color(Color32::WHITE));
            });
            ui.separator();

            egui::ScrollArea::vertical()
                .max_height(420.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut state.email_body)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(18)
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

    if let (Some(invoice), Some(customer)) = (inv, cust) {
        state.recipient_email = customer.email.clone();
        state.email_subject = format!(
            "Invoice {} from {}",
            invoice.invoice_number, company.trading_name
        );

        let mut body = String::new();
        body.push_str(&format!("Dear {},\n\n", customer.name));
        body.push_str(&format!(
            "Thank you for your business. Please find attached invoice {} issued on {}.\n\n",
            invoice.invoice_number, invoice.issue_date
        ));
        body.push_str("--------------------------------------------------\n");
        body.push_str(&format!("INVOICE SUMMARY: {}\n", invoice.invoice_number));
        body.push_str(&format!("Due Date: {}\n", invoice.due_date));
        body.push_str(&format!(
            "Total Amount Due: ${:.2} ({})\n",
            invoice.total_amount, invoice.currency
        ));
        body.push_str("--------------------------------------------------\n\n");

        body.push_str("BANK WIRE REMITTANCE DETAILS:\n");
        body.push_str(&format!("Bank Name: {}\n", company.bank_name));
        body.push_str(&format!("IBAN: {}\n", company.bank_iban));
        body.push_str(&format!("SWIFT/BIC: {}\n", company.bank_swift));
        body.push_str(&format!("Account Name: {}\n", company.legal_name));
        body.push_str(&format!(
            "Payment Reference: {}\n\n",
            invoice.invoice_number
        ));

        body.push_str("If you have any questions or require an updated delivery schedule, please feel free to reach out.\n\n");
        body.push_str(&format!(
            "Warm regards,\n{}\n{}\n{}",
            company.trading_name, company.phone, company.website
        ));

        state.email_body = body;
    }
}

fn update_shipping_email_template(
    state: &mut EmailCenterState,
    customers: &[Customer],
    deliveries: &[DeliveryOrder],
    company: &CompanyProfile,
) {
    let d_opt = deliveries
        .iter()
        .find(|d| Some(d.id) == state.selected_delivery_id);
    let cust = customers
        .iter()
        .find(|c| Some(c.id) == state.selected_customer_id);

    if let (Some(d), Some(customer)) = (d_opt, cust) {
        state.recipient_email = customer.email.clone();
        state.email_subject = format!(
            "Shipping Update: Delivery Order {} is {}",
            d.delivery_number, state.shipping_status_type
        );

        let mut body = String::new();
        body.push_str(&format!("Dear {},\n\n", customer.name));
        body.push_str(&format!(
            "We are writing to update you on your shipment under Delivery Order {}.\n\n",
            d.delivery_number
        ));
        body.push_str("--------------------------------------------------\n");
        body.push_str(&format!(
            "LOGISTICS STATUS: {}\n",
            state.shipping_status_type.to_uppercase()
        ));
        body.push_str(&format!("Carrier / Fleet: {}\n", state.carrier_name));
        body.push_str(&format!("Tracking Reference: {}\n", state.tracking_number));
        body.push_str(&format!("Scheduled Date: {}\n", d.delivery_date));
        body.push_str(&format!("Destination: {}\n", d.delivery_address));
        body.push_str(&format!("Postal Routing Zone: {}\n", d.postal_zone));
        body.push_str("--------------------------------------------------\n\n");

        body.push_str("PACKAGE MANIFEST:\n");
        for (i, it) in d.items.iter().enumerate() {
            body.push_str(&format!(
                "{}. {} (Qty: {} {})\n",
                i + 1,
                it.description,
                it.quantity,
                it.unit
            ));
        }
        body.push_str("\nSpecial Handling Notes:\n");
        body.push_str(&format!("{}\n\n", d.driver_notes));

        body.push_str("Our dispatch team is monitoring the delivery. Please ensure an authorized receiver is available at the delivery location.\n\n");
        body.push_str(&format!(
            "Best regards,\nLogistics & Dispatch Team\n{}\n{}",
            company.trading_name, company.phone
        ));

        state.email_body = body;
    }
}

mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut result = String::new();
        for b in s.bytes() {
            match b {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    result.push(b as char);
                }
                b' ' => result.push_str("%20"),
                b'\n' => result.push_str("%0D%0A"),
                _ => {
                    result.push_str(&format!("%{:02X}", b));
                }
            }
        }
        result
    }
}
