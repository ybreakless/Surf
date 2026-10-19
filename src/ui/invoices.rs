use chrono::{NaiveDate, Utc};
use eframe::egui::{self, RichText};
use rust_decimal::Decimal;
use std::path::PathBuf;
use std::str::FromStr;
use uuid::Uuid;

use super::theme::Theme;
use crate::core::accounting::AccountingEngine;
use crate::core::export::InvoiceExporter;
use crate::core::model::{
    Customer, DeliveryItem, DeliveryOrder, DeliveryStatus, Invoice, InvoiceItem, InvoiceStatus,
};
use crate::core::numbering::NumberingEngine;
use crate::core::templates::{self, MessageLang};
use crate::db::repository::Repository;

pub struct InvoiceLineState {
    pub id: Uuid,
    pub description: String,
    pub quantity: String,
    pub unit: String,
    pub unit_price: String,
    pub discount_pct: String,
}

/// Invoice document + sharing side state.
#[derive(Default)]
pub struct InvoiceViewState {
    /// Currently opened in-app document preview: invoice id + rendered texture.
    pub preview: Option<(Uuid, String, egui::TextureHandle)>,
    /// Background Telegram upload worker (invoice number, result).
    pub telegram_rx: Option<std::sync::mpsc::Receiver<(String, Result<(), String>)>>,
}

pub struct InvoiceFormState {
    pub is_open: bool,
    pub id: Option<Uuid>,
    pub invoice_number: String,
    pub selected_customer_id: Option<Uuid>,
    pub issue_date_str: String,
    pub due_date_str: String,
    pub items: Vec<InvoiceLineState>,
    pub tax_rate_str: String,
    pub notes: String,
    pub error_msg: Option<String>,
    pub filter: String,
    pub view: InvoiceViewState,
}

impl Default for InvoiceFormState {
    fn default() -> Self {
        let now = Utc::now().date_naive();
        Self {
            is_open: false,
            id: None,
            invoice_number: String::new(),
            selected_customer_id: None,
            issue_date_str: now.format("%Y-%m-%d").to_string(),
            due_date_str: (now + chrono::Duration::days(30))
                .format("%Y-%m-%d")
                .to_string(),
            items: vec![InvoiceLineState {
                id: Uuid::new_v4(),
                description: String::new(),
                quantity: "1".to_string(),
                unit: "Pcs".to_string(),
                unit_price: "0.00".to_string(),
                discount_pct: "0".to_string(),
            }],
            tax_rate_str: "5.0".to_string(),
            notes: "Payment due within 30 days.".to_string(),
            error_msg: None,
            filter: String::new(),
            view: InvoiceViewState::default(),
        }
    }
}

impl InvoiceFormState {
    pub fn new_with_customer(c: &Customer, repo: &Repository) -> Self {
        let next_seq = repo
            .next_invoice_sequence(&c.postal_zone, &c.code)
            .unwrap_or(1);
        Self {
            is_open: true,
            selected_customer_id: Some(c.id),
            invoice_number: NumberingEngine::format_invoice_number(
                &c.postal_zone,
                &c.code,
                next_seq,
            ),
            ..Self::default()
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

pub fn render(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut InvoiceFormState,
    notification: &mut Option<(String, bool)>,
) {
    ui.add_space(8.0);

    // Poll the background Telegram uploader (keeps the GUI non-blocking).
    if let Some(rx) = &state.view.telegram_rx {
        match rx.try_recv() {
            Ok((number, Ok(()))) => {
                state.view.telegram_rx = None;
                *notification = Some((format!("Invoice {number} sent to Telegram!"), true));
            }
            Ok((number, Err(e))) => {
                state.view.telegram_rx = None;
                *notification = Some((format!("Telegram send failed for {number}: {}", e), false));
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.view.telegram_rx = None;
            }
        }
    }

    let customers = repo.get_customers().unwrap_or_default();
    let invoices = repo.get_invoices().unwrap_or_default();

    if state.is_open {
        render_invoice_form(ui, repo, state, &customers, notification);
    } else {
        render_invoice_list(ui, repo, state, &invoices, &customers, notification);
        render_preview_window(ui.ctx(), state, notification);
    }
}

/// In-app invoice document preview (rendered from the same canvas as the PDF/PNG exports).
fn render_preview_window(
    ctx: &egui::Context,
    state: &mut InvoiceFormState,
    notification: &mut Option<(String, bool)>,
) {
    if let Some((_, number, handle)) = &state.view.preview {
        let window_title = format!("Preview - {}", number);
        let mut keep_open = true;
        let mut close_pressed = false;
        egui::Window::new(window_title)
            .open(&mut keep_open)
            .resizable(true)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.add(
                    egui::Image::from_texture(&handle.clone())
                        .fit_to_exact_size(egui::vec2(620.0, 876.0)),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let number = number.clone();
                    let pdf_path = crate::services::share::exports_dir().join("pdf");
                    if ui.button("Open PDF in system viewer").clicked() {
                        let path = pdf_path.join(format!("{}.pdf", number.replace('/', "_")));
                        match crate::services::share::open_in_default_app(&path.to_string_lossy()) {
                            Ok(()) => {
                                *notification = Some((format!("Opened {}", path.display()), true));
                            }
                            Err(e) => {
                                *notification = Some((e, false));
                            }
                        }
                    }
                    if ui.button("Close Preview").clicked() {
                        close_pressed = true;
                    }
                });
            });
        if !keep_open || close_pressed {
            state.view.preview = None;
        }
    }
}

fn render_invoice_list(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut InvoiceFormState,
    invoices: &[Invoice],
    customers: &[Customer],
    notification: &mut Option<(String, bool)>,
) {
    let company = repo.get_company_profile();
    let share_lang = MessageLang::from_code(
        repo.get_setting("share_language")
            .unwrap_or_default()
            .as_str(),
    );

    Theme::card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Sales Invoices");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(
                        RichText::new("+ Create New Invoice")
                            .color(Theme::text_strong())
                            .strong(),
                    )
                    .clicked()
                {
                    state.reset();
                    state.is_open = true;
                    if let Some(first_cust) = customers.first() {
                        state.selected_customer_id = Some(first_cust.id);
                        let next_seq = repo
                            .next_invoice_sequence(&first_cust.postal_zone, &first_cust.code)
                            .unwrap_or(1);
                        state.invoice_number = NumberingEngine::format_invoice_number(
                            &first_cust.postal_zone,
                            &first_cust.code,
                            next_seq,
                        );
                    }
                }
            });
        });
        ui.add_space(6.0);

        // Quick filter
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Search:")
                    .color(Theme::text_muted())
                    .size(12.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut state.filter)
                    .hint_text("Invoice #, customer, status...")
                    .desired_width(260.0),
            );
            if !state.filter.is_empty() && ui.small_button("Clear").clicked() {
                state.filter.clear();
            }
        });
        ui.separator();

        if invoices.is_empty() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No invoices created yet.").color(Theme::text_muted()));
                ui.label("Click '+ Create New Invoice' above to draft your first invoice.");
            });
            ui.add_space(16.0);
            return;
        }

        let query = state.filter.trim().to_lowercase();
        let filtered: Vec<&Invoice> = invoices
            .iter()
            .filter(|i| {
                query.is_empty()
                    || i.invoice_number.to_lowercase().contains(&query)
                    || i.customer_name.to_lowercase().contains(&query)
                    || i.status.as_str().to_lowercase().contains(&query)
            })
            .collect();

        if filtered.is_empty() {
            ui.add_space(12.0);
            ui.label(
                RichText::new(format!("No invoices match '{}'.", state.filter))
                    .color(Theme::text_muted()),
            );
            return;
        }

        // Invoices Table Header
        egui::Grid::new("invoices_table_header")
            .num_columns(8)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Invoice #").strong());
                ui.label(RichText::new("Postal Zone").strong());
                ui.label(RichText::new("Customer").strong());
                ui.label(RichText::new("Issue Date").strong());
                ui.label(RichText::new("Due Date").strong());
                ui.label(RichText::new("Total").strong());
                ui.label(RichText::new("Status").strong());
                ui.label(RichText::new("Actions").strong());
                ui.end_row();
            });
        ui.separator();

        let list_height = (ui.available_height() - 16.0).max(240.0);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .max_height(list_height)
            .show(ui, |ui| {
                for inv in filtered {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&inv.invoice_number).strong());
                        ui.label(
                            RichText::new(format!("[{}]", inv.postal_zone))
                                .color(Theme::primary())
                                .size(12.0),
                        );
                        ui.label(&inv.customer_name);
                        ui.label(inv.issue_date.to_string());
                        ui.label(inv.due_date.to_string());
                        ui.label(
                            RichText::new(crate::core::money::format_money_by_code(
                                &inv.currency,
                                inv.total_amount,
                            ))
                            .strong(),
                        );

                        let badge_color = match inv.status {
                            InvoiceStatus::Paid => Theme::success(),
                            InvoiceStatus::Sent => Theme::primary(),
                            InvoiceStatus::Draft => Theme::text_muted(),
                            InvoiceStatus::Cancelled => Theme::danger(),
                        };
                        Theme::status_badge(ui, inv.status.as_str(), badge_color);

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Share submenu (preview / open / send)
                            render_share_menu(
                                ui,
                                repo,
                                state,
                                inv,
                                &company,
                                share_lang,
                                notification,
                            );

                            // Generate Delivery Order Button
                            if ui.button("Create DO").clicked() {
                                create_delivery_order_from_invoice(repo, inv, notification);
                            }

                            // Export PDF Button
                            if ui.button("PDF").clicked() {
                                notify_export(
                                    InvoiceExporter::export_to_pdf(inv, &company, &pdf_dir()),
                                    "Invoice PDF exported to",
                                    notification,
                                );
                            }

                            // Export Image Button
                            if ui.button("PNG").clicked() {
                                notify_export(
                                    InvoiceExporter::export_to_image(inv, &company, &img_dir()),
                                    "Invoice PNG image exported to",
                                    notification,
                                );
                            }

                            // Mark as Paid Button
                            if inv.status != InvoiceStatus::Paid
                                && ui
                                    .button(RichText::new("Mark Paid").color(Theme::success()))
                                    .clicked()
                            {
                                mark_invoice_as_paid(repo, inv, notification);
                            }
                        });
                    });
                    ui.separator();
                }
            });
    });
}

fn pdf_dir() -> PathBuf {
    crate::services::share::exports_dir().join("pdf")
}

fn img_dir() -> PathBuf {
    crate::services::share::exports_dir().join("images")
}

fn notify_export(
    result: Result<PathBuf, String>,
    success_prefix: &str,
    notification: &mut Option<(String, bool)>,
) {
    match result {
        Ok(p) => {
            *notification = Some((format!("{} '{}'!", success_prefix, p.display()), true));
        }
        Err(e) => {
            *notification = Some((format!("Export error: {}", e), false));
        }
    }
}

/// Compact "..." action menu for one invoice row.
fn render_share_menu(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut InvoiceFormState,
    inv: &Invoice,
    company: &crate::core::model::CompanyProfile,
    share_lang: MessageLang,
    notification: &mut Option<(String, bool)>,
) {
    let mut action = None;
    ui.menu_button("...", |ui| {
        if ui.button("Preview in App").clicked() {
            action = Some("preview");
        }
        if ui.button("Open PDF (system viewer)").clicked() {
            action = Some("open_pdf");
        }
        if ui.button("Send file via Telegram").clicked() {
            action = Some("telegram");
        }
        if ui.button("Share to WhatsApp").clicked() {
            action = Some("whatsapp");
        }
        if ui.button("Copy PDF file path").clicked() {
            action = Some("copy_path");
        }
    });

    let Some(action) = action else { return };

    match action {
        "preview" => {
            let (rgba, size) = InvoiceExporter::render_rgba_pixels(inv, company);
            let image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
            let handle =
                ui.ctx()
                    .load_texture("invoice_preview", image, egui::TextureOptions::LINEAR);
            state.view.preview = Some((inv.id, inv.invoice_number.clone(), handle));
        }
        "open_pdf" => {
            let pdf_path = pdf_dir().join(format!("{}.pdf", inv.invoice_number.replace('/', "_")));
            if !pdf_path.exists() {
                let _ = InvoiceExporter::export_to_pdf(inv, company, &pdf_dir());
            }
            match crate::services::share::open_in_default_app(&pdf_path.to_string_lossy()) {
                Ok(()) => {
                    *notification = Some((format!("Opened {}", pdf_path.display()), true));
                }
                Err(e) => {
                    *notification = Some((e, false));
                }
            }
        }
        "telegram" => {
            let pdf_path = pdf_dir().join(format!("{}.pdf", inv.invoice_number.replace('/', "_")));
            if !pdf_path.exists() {
                let _ = InvoiceExporter::export_to_pdf(inv, company, &pdf_dir());
            }
            let db_path = repo.db_path().to_path_buf();
            let number = inv.invoice_number.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            let path_for_thread = pdf_path.clone();
            std::thread::spawn(move || {
                let res = match Repository::new(&db_path) {
                    Ok(worker_repo) => crate::services::share::send_document_to_telegram(
                        &worker_repo,
                        &path_for_thread,
                    ),
                    Err(e) => Err(format!("Cannot open database: {}", e)),
                };
                tx.send((number, res.map_err(|e| e.to_string()))).ok();
            });
            state.view.telegram_rx = Some(rx);
            *notification = Some((
                format!(
                    "Sending {} to Telegram in background...",
                    inv.invoice_number
                ),
                true,
            ));
        }
        "whatsapp" => {
            let pdf_path = pdf_dir().join(format!("{}.pdf", inv.invoice_number.replace('/', "_")));
            if !pdf_path.exists() {
                let _ = InvoiceExporter::export_to_pdf(inv, company, &pdf_dir());
            }
            let customers = repo.get_customers().unwrap_or_default();
            let phone = customers
                .iter()
                .find(|c| c.id == inv.customer_id)
                .map(|c| c.phone.clone())
                .unwrap_or_default();

            let link = crate::services::share::wa_me_link(
                &phone,
                &templates::compose_short_invoice_share(share_lang, inv),
            );
            match crate::services::share::open_in_default_app(&link) {
                Ok(()) => {
                    *notification = Some((
                        format!(
                            "WhatsApp chat opened. Attach the exported PDF from '{}' if your WhatsApp app supports it.",
                            pdf_path.display()
                        ),
                        true,
                    ));
                }
                Err(e) => {
                    *notification = Some((e, false));
                }
            }
        }
        "copy_path" => {
            let pdf_path = pdf_dir().join(format!("{}.pdf", inv.invoice_number.replace('/', "_")));
            ui.ctx().copy_text(pdf_path.to_string_lossy().to_string());
            *notification = Some((format!("Path copied: {}", pdf_path.display()), true));
        }
        _ => {}
    }
}

fn render_invoice_form(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut InvoiceFormState,
    customers: &[Customer],
    notification: &mut Option<(String, bool)>,
) {
    Theme::card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("New Sales Invoice");
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

        egui::Grid::new("invoice_form_top")
            .num_columns(4)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Customer:");
                let current_cust_name = customers
                    .iter()
                    .find(|c| Some(c.id) == state.selected_customer_id)
                    .map(|c| c.name.as_str())
                    .unwrap_or("Select a Customer");

                egui::ComboBox::from_id_salt("customer_combo")
                    .selected_text(current_cust_name)
                    .show_ui(ui, |ui| {
                        for c in customers {
                            let is_selected = Some(c.id) == state.selected_customer_id;
                            if ui.selectable_label(is_selected, &c.name).clicked() {
                                state.selected_customer_id = Some(c.id);
                                let next_seq = repo
                                    .next_invoice_sequence(&c.postal_zone, &c.code)
                                    .unwrap_or(1);
                                state.invoice_number = NumberingEngine::format_invoice_number(
                                    &c.postal_zone,
                                    &c.code,
                                    next_seq,
                                );
                            }
                        }
                    });

                ui.label(RichText::new("Generated Invoice #:").strong());
                ui.text_edit_singleline(&mut state.invoice_number);
                ui.end_row();

                ui.label("Issue Date (YYYY-MM-DD):");
                ui.text_edit_singleline(&mut state.issue_date_str);

                ui.label("Due Date (YYYY-MM-DD):");
                ui.text_edit_singleline(&mut state.due_date_str);
                ui.end_row();
            });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.heading("📊 Spreadsheet Invoice Creator");
            ui.add_space(16.0);
            ui.label(
                RichText::new(
                    "Excel-Style Live Formula Grid: Line Total = (Qty * Price) * (1 - Disc%)",
                )
                .size(11.0)
                .color(Theme::text_muted()),
            );
        });
        ui.separator();

        // Excel Column Headers Bar
        let header_frame = egui::Frame::new()
            .fill(Theme::bg_card_light())
            .inner_margin(egui::Margin::symmetric(8, 4))
            .corner_radius(egui::CornerRadius::same(3));

        header_frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("A: #")
                        .strong()
                        .size(11.0)
                        .color(Theme::primary()),
                );
                ui.add_space(12.0);
                ui.label(
                    RichText::new("B: Item / Service Description")
                        .strong()
                        .size(11.0)
                        .color(Theme::text_strong()),
                );
                ui.add_space(100.0);
                ui.label(
                    RichText::new("C: Qty")
                        .strong()
                        .size(11.0)
                        .color(Theme::text_strong()),
                );
                ui.add_space(20.0);
                ui.label(
                    RichText::new("D: Unit")
                        .strong()
                        .size(11.0)
                        .color(Theme::text_strong()),
                );
                ui.add_space(18.0);
                ui.label(
                    RichText::new("E: Price ($)")
                        .strong()
                        .size(11.0)
                        .color(Theme::text_strong()),
                );
                ui.add_space(20.0);
                ui.label(
                    RichText::new("F: Disc%")
                        .strong()
                        .size(11.0)
                        .color(Theme::text_strong()),
                );
                ui.add_space(20.0);
                ui.label(
                    RichText::new("G: Line Total")
                        .strong()
                        .size(11.0)
                        .color(Theme::success()),
                );
            });
        });
        ui.add_space(4.0);

        // Items Grid
        let mut to_remove = None;
        let mut subtotal = Decimal::ZERO;
        let num_items = state.items.len();
        let display_currency = customers
            .iter()
            .find(|c| Some(c.id) == state.selected_customer_id)
            .map(|c| c.currency.clone())
            .unwrap_or_else(|| "USD".to_string());

        let products = repo.get_products().unwrap_or_default();
        let sellable: Vec<&crate::core::model::Product> =
            products.iter().filter(|p| p.kind.sellable()).collect();

        for (idx, item) in state.items.iter_mut().enumerate() {
            let qty = Decimal::from_str(&item.quantity).unwrap_or(Decimal::ZERO);
            let price = Decimal::from_str(&item.unit_price).unwrap_or(Decimal::ZERO);
            let disc = Decimal::from_str(&item.discount_pct)
                .unwrap_or(Decimal::ZERO)
                .clamp(Decimal::ZERO, Decimal::from(100));
            let line_total = qty * price * (Decimal::from(100) - disc) / Decimal::from(100);
            subtotal += line_total;

            ui.horizontal(|ui| {
                ui.label(format!("{}.", idx + 1));

                let selected_name = sellable
                    .iter()
                    .find(|p| p.name == item.description && p.unit == item.unit)
                    .map(|p| p.name.as_str())
                    .unwrap_or("Catalog...");
                egui::ComboBox::from_id_salt(("inv_product_combo", idx))
                    .selected_text(selected_name)
                    .show_ui(ui, |ui| {
                        for p in &sellable {
                            if ui
                                .selectable_label(p.name == item.description, &p.name)
                                .clicked()
                            {
                                item.description = p.name.clone();
                                item.unit = p.unit.clone();
                                item.unit_price = p.default_price.to_string();
                                if p.tax_rate > Decimal::ZERO {
                                    state.tax_rate_str = p.tax_rate.to_string();
                                }
                            }
                        }
                    });

                ui.add(
                    egui::TextEdit::singleline(&mut item.description)
                        .hint_text("Item or Service description")
                        .desired_width(220.0),
                );
                ui.label("Qty:");
                ui.add(egui::TextEdit::singleline(&mut item.quantity).desired_width(45.0));
                ui.label("Unit:");
                ui.add(egui::TextEdit::singleline(&mut item.unit).desired_width(50.0));
                ui.label(format!("Price ({}):", display_currency));
                ui.add(egui::TextEdit::singleline(&mut item.unit_price).desired_width(70.0));
                ui.label("Disc %:");
                ui.add(egui::TextEdit::singleline(&mut item.discount_pct).desired_width(40.0));
                ui.label(
                    RichText::new(format!(
                        "= {}",
                        crate::core::money::format_money_by_code(&display_currency, line_total)
                    ))
                    .strong(),
                );

                if num_items > 1 && ui.small_button("✕").clicked() {
                    to_remove = Some(idx);
                }
            });
            ui.add_space(4.0);
        }

        if let Some(idx) = to_remove {
            state.items.remove(idx);
        }

        if ui.button("+ Add Line Item").clicked() {
            state.items.push(InvoiceLineState {
                id: Uuid::new_v4(),
                description: String::new(),
                quantity: "1".to_string(),
                unit: "Pcs".to_string(),
                unit_price: "0.00".to_string(),
                discount_pct: "0".to_string(),
            });
        }

        ui.add_space(16.0);
        ui.separator();

        // Calculations & Tax
        let tax_rate = Decimal::from_str(&state.tax_rate_str).unwrap_or(Decimal::ZERO);
        let tax_amount = subtotal * (tax_rate / Decimal::from(100));
        let total_amount = subtotal + tax_amount;

        ui.columns(2, |cols| {
            cols[0].vertical(|ui| {
                ui.label("Tax Rate (%):");
                ui.add(egui::TextEdit::singleline(&mut state.tax_rate_str).desired_width(60.0));
                ui.label("Terms / Notes:");
                ui.text_edit_multiline(&mut state.notes);
            });

            cols[1].vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Subtotal:");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(crate::core::money::format_money_by_code(
                            &display_currency,
                            subtotal,
                        ));
                    });
                });
                ui.horizontal(|ui| {
                    ui.label(format!("Sales Tax ({}%):", tax_rate));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(crate::core::money::format_money_by_code(
                            &display_currency,
                            tax_amount,
                        ));
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.heading(RichText::new("Grand Total:").size(18.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.heading(
                            RichText::new(crate::core::money::format_money_by_code(
                                &display_currency,
                                total_amount,
                            ))
                            .color(Theme::success())
                            .size(20.0),
                        );
                    });
                });
            });
        });

        ui.add_space(16.0);

        // Action Buttons
        ui.horizontal(|ui| {
            if ui
                .button(
                    RichText::new("Post & Issue Invoice")
                        .color(Theme::text_strong())
                        .strong(),
                )
                .clicked()
            {
                let totals = InvoiceTotals {
                    subtotal,
                    tax_rate,
                    tax_amount,
                    total_amount,
                };
                save_and_post_invoice(repo, state, customers, totals, notification);
            }
            if ui.button("Cancel").clicked() {
                state.is_open = false;
            }
        });
    });
}

struct InvoiceTotals {
    subtotal: Decimal,
    tax_rate: Decimal,
    tax_amount: Decimal,
    total_amount: Decimal,
}

fn save_and_post_invoice(
    repo: &Repository,
    state: &mut InvoiceFormState,
    customers: &[Customer],
    totals: InvoiceTotals,
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

    if state.invoice_number.trim().is_empty() {
        state.error_msg = Some("Invoice number cannot be empty".to_string());
        return;
    }

    let issue_date = match NaiveDate::parse_from_str(&state.issue_date_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            state.error_msg = Some("Invalid Issue Date format (use YYYY-MM-DD)".to_string());
            return;
        }
    };

    let due_date = match NaiveDate::parse_from_str(&state.due_date_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            state.error_msg = Some("Invalid Due Date format (use YYYY-MM-DD)".to_string());
            return;
        }
    };

    let items: Vec<InvoiceItem> = state
        .items
        .iter()
        .map(|it| {
            let q = Decimal::from_str(&it.quantity).unwrap_or(Decimal::ONE);
            let p = Decimal::from_str(&it.unit_price).unwrap_or(Decimal::ZERO);
            let d = Decimal::from_str(&it.discount_pct)
                .unwrap_or(Decimal::ZERO)
                .clamp(Decimal::ZERO, Decimal::from(100));
            InvoiceItem {
                id: it.id,
                description: it.description.clone(),
                quantity: q,
                unit: it.unit.trim().to_string(),
                unit_price: p,
                discount_pct: d,
                amount: q * p * (Decimal::from(100) - d) / Decimal::from(100),
            }
        })
        .collect();

    let invoice = Invoice {
        id: state.id.unwrap_or_else(Uuid::new_v4),
        invoice_number: state.invoice_number.trim().to_string(),
        customer_id: cust.id,
        customer_name: cust.name.clone(),
        postal_code: cust.postal_code.clone(),
        postal_zone: cust.postal_zone.clone(),
        issue_date,
        due_date,
        status: InvoiceStatus::Sent,
        items,
        subtotal: totals.subtotal,
        tax_rate: totals.tax_rate,
        tax_amount: totals.tax_amount,
        total_amount: totals.total_amount,
        currency: cust.currency.clone(),
        notes: state.notes.trim().to_string(),
        journal_entry_id: None,
        created_at: Utc::now(),
    };

    // Auto-Post Double-Entry Journal Entry
    let accounts = repo.get_accounts().unwrap_or_default();
    let ar_acc = accounts.iter().find(|a| a.code == "1200").cloned();
    let rev_acc = accounts.iter().find(|a| a.code == "4000").cloned();
    let tax_acc = accounts.iter().find(|a| a.code == "2200").cloned();

    if let (Some(ar), Some(rev), Some(tax)) = (ar_acc, rev_acc, tax_acc) {
        let seq = repo.next_journal_sequence().unwrap_or(1);
        if let Ok(je) =
            AccountingEngine::create_invoice_journal_entry(&invoice, &ar, &rev, &tax, seq)
        {
            let _ = repo.insert_journal_entry(&je, true);
        }
    }

    match repo.insert_invoice(&invoice, true) {
        Ok(_) => {
            let company = repo.get_company_profile();
            let _ = InvoiceExporter::export_to_pdf(&invoice, &company, &pdf_dir());
            let _ = InvoiceExporter::export_to_image(&invoice, &company, &img_dir());

            *notification = Some((
                format!(
                    "Invoice {} posted to Ledger & exported to PDF & PNG!",
                    invoice.invoice_number
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

fn mark_invoice_as_paid(
    repo: &Repository,
    inv: &Invoice,
    notification: &mut Option<(String, bool)>,
) {
    let mut updated = inv.clone();
    updated.status = InvoiceStatus::Paid;

    let accounts = repo.get_accounts().unwrap_or_default();
    let cash_acc = accounts.iter().find(|a| a.code == "1000").cloned();
    let ar_acc = accounts.iter().find(|a| a.code == "1200").cloned();

    if let (Some(cash), Some(ar)) = (cash_acc, ar_acc) {
        let seq = repo.next_journal_sequence().unwrap_or(1);
        if let Ok(je) = AccountingEngine::create_payment_journal_entry(&updated, &cash, &ar, seq) {
            let _ = repo.insert_journal_entry(&je, true);
        }
    }

    match repo.insert_invoice(&updated, true) {
        Ok(_) => {
            *notification = Some((
                format!(
                    "Invoice {} marked as PAID and Cash received!",
                    inv.invoice_number
                ),
                true,
            ));
        }
        Err(e) => {
            *notification = Some((format!("Error updating invoice: {}", e), false));
        }
    }
}

fn create_delivery_order_from_invoice(
    repo: &Repository,
    inv: &Invoice,
    notification: &mut Option<(String, bool)>,
) {
    let customers = repo.get_customers().unwrap_or_default();
    let cust = customers.iter().find(|c| c.id == inv.customer_id);

    let cust_code = cust.map(|c| c.code.as_str()).unwrap_or("CUS");
    let next_do_seq = repo
        .next_delivery_sequence(&inv.postal_zone, cust_code)
        .unwrap_or(1);
    let do_num = NumberingEngine::format_delivery_number(&inv.postal_zone, cust_code, next_do_seq);

    let items: Vec<DeliveryItem> = inv
        .items
        .iter()
        .map(|it| DeliveryItem {
            id: Uuid::new_v4(),
            description: it.description.clone(),
            quantity: it.quantity,
            unit: "Pcs".to_string(),
        })
        .collect();

    let delivery = DeliveryOrder {
        id: Uuid::new_v4(),
        delivery_number: do_num,
        invoice_id: Some(inv.id),
        invoice_number: Some(inv.invoice_number.clone()),
        customer_id: inv.customer_id,
        customer_name: inv.customer_name.clone(),
        delivery_address: cust.map(|c| c.address.clone()).unwrap_or_default(),
        postal_code: inv.postal_code.clone(),
        postal_zone: inv.postal_zone.clone(),
        delivery_date: Utc::now().date_naive(),
        status: DeliveryStatus::Pending,
        items,
        driver_notes: "Handle with care. Verification on delivery.".to_string(),
        created_at: Utc::now(),
    };

    match repo.insert_delivery_order(&delivery, true) {
        Ok(_) => {
            *notification = Some((
                format!(
                    "Delivery Order {} created for Invoice {}!",
                    delivery.delivery_number, inv.invoice_number
                ),
                true,
            ));
        }
        Err(e) => {
            *notification = Some((format!("Error creating delivery order: {}", e), false));
        }
    }
}
