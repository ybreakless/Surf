use chrono::{NaiveDate, Utc};
use eframe::egui::{self, RichText};
use rust_decimal::Decimal;
use std::str::FromStr;
use uuid::Uuid;

use super::theme::Theme;
use crate::core::accounting::AccountingEngine;
use crate::core::model::{BillStatus, InvoiceItem, Product, ProductKind, Vendor, VendorBill};
use crate::core::numbering::NumberingEngine;
use crate::db::repository::Repository;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PurchasesTab {
    #[default]
    Bills,
    Vendors,
    Catalog,
}

#[derive(Default)]
pub struct VendorFormState {
    pub is_open: bool,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub address: String,
    pub city: String,
    pub country: String,
    pub postal_code: String,
    pub currency: String,
    pub payment_terms: String,
    pub notes: String,
    pub error_msg: Option<String>,
}

pub struct ProductFormState {
    pub is_open: bool,
    pub name: String,
    pub kind_idx: usize,
    pub unit: String,
    pub default_price: String,
    pub tax_rate: String,
    pub income_account: String,
    pub expense_account: String,
    pub error_msg: Option<String>,
}

impl Default for ProductFormState {
    fn default() -> Self {
        Self {
            is_open: false,
            name: String::new(),
            kind_idx: 0,
            unit: "Pcs".to_string(),
            default_price: "0.00".to_string(),
            tax_rate: "0".to_string(),
            income_account: "4000".to_string(),
            expense_account: "6000".to_string(),
            error_msg: None,
        }
    }
}

pub struct BillLineState {
    pub id: Uuid,
    pub description: String,
    pub quantity: String,
    pub unit: String,
    pub unit_price: String,
}

pub struct BillFormState {
    pub is_open: bool,
    pub bill_number: String,
    pub selected_vendor_id: Option<Uuid>,
    pub issue_date_str: String,
    pub due_date_str: String,
    pub items: Vec<BillLineState>,
    pub tax_rate_str: String,
    pub notes: String,
    pub error_msg: Option<String>,
    pub filter: String,
}

impl Default for BillFormState {
    fn default() -> Self {
        let now = Utc::now().date_naive();
        Self {
            is_open: false,
            bill_number: String::new(),
            selected_vendor_id: None,
            issue_date_str: now.format("%Y-%m-%d").to_string(),
            due_date_str: (now + chrono::Duration::days(30))
                .format("%Y-%m-%d")
                .to_string(),
            items: vec![BillLineState {
                id: Uuid::new_v4(),
                description: String::new(),
                quantity: "1".to_string(),
                unit: "Pcs".to_string(),
                unit_price: "0.00".to_string(),
            }],
            tax_rate_str: "0".to_string(),
            notes: String::new(),
            error_msg: None,
            filter: String::new(),
        }
    }
}

#[derive(Default)]
pub struct PurchasesState {
    pub sub_tab: PurchasesTab,
    pub vendor_form: VendorFormState,
    pub product_form: ProductFormState,
    pub bill_form: BillFormState,
}

pub fn render(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut PurchasesState,
    notification: &mut Option<(String, bool)>,
) {
    ui.add_space(8.0);

    // Sub-tab selector
    ui.horizontal(|ui| {
        for (tab, label) in [
            (PurchasesTab::Bills, "Bills"),
            (PurchasesTab::Vendors, "Vendors"),
            (PurchasesTab::Catalog, "Products & Services"),
        ] {
            let active = state.sub_tab == tab;
            let btn = ui.add(
                egui::Button::new(RichText::new(label).color(if active {
                    Theme::on_primary()
                } else {
                    Theme::text_muted()
                }))
                .fill(if active {
                    Theme::primary()
                } else {
                    Theme::bg_card_light()
                })
                .corner_radius(egui::CornerRadius::same(6)),
            );
            if btn.clicked() {
                state.sub_tab = tab;
            }
        }
    });
    ui.add_space(8.0);

    match state.sub_tab {
        PurchasesTab::Bills => render_bills(ui, repo, state, notification),
        PurchasesTab::Vendors => render_vendors(ui, repo, state, notification),
        PurchasesTab::Catalog => render_catalog(ui, repo, state, notification),
    }
}

// --- Bills ---

fn render_bills(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut PurchasesState,
    notification: &mut Option<(String, bool)>,
) {
    let vendors = repo.get_vendors().unwrap_or_default();
    let bills = repo.get_vendor_bills().unwrap_or_default();

    if state.bill_form.is_open {
        render_bill_form(ui, repo, state, &vendors, notification);
        return;
    }

    Theme::card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Vendor Bills");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(
                        RichText::new("+ Record New Bill")
                            .color(Theme::text_strong())
                            .strong(),
                    )
                    .clicked()
                {
                    state.bill_form = BillFormState::default();
                    let seq = repo.next_bill_sequence().unwrap_or(1);
                    state.bill_form.bill_number = NumberingEngine::format_bill_number(seq);
                    state.bill_form.selected_vendor_id = vendors.first().map(|v| v.id);
                    state.bill_form.is_open = true;
                }
            });
        });
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Search:")
                    .color(Theme::text_muted())
                    .size(12.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut state.bill_form.filter)
                    .hint_text("Bill #, vendor, status...")
                    .desired_width(260.0),
            );
        });
        ui.separator();

        if bills.is_empty() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No bills recorded yet.").color(Theme::text_muted()));
                ui.label("Click '+ Record New Bill' to log your first supplier bill.");
            });
            ui.add_space(16.0);
            return;
        }

        let query = state.bill_form.filter.trim().to_lowercase();
        let filtered: Vec<&VendorBill> = bills
            .iter()
            .filter(|b| {
                query.is_empty()
                    || b.bill_number.to_lowercase().contains(&query)
                    || b.vendor_name.to_lowercase().contains(&query)
                    || b.status.as_str().to_lowercase().contains(&query)
            })
            .collect();

        egui::Grid::new("bills_table_header")
            .num_columns(6)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Bill #").strong());
                ui.label(RichText::new("Vendor").strong());
                ui.label(RichText::new("Issue Date").strong());
                ui.label(RichText::new("Due Date").strong());
                ui.label(RichText::new("Total").strong());
                ui.label(RichText::new("Status").strong());
                ui.end_row();
            });
        ui.separator();

        let list_height = (ui.available_height() - 16.0).max(240.0);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .max_height(list_height)
            .show(ui, |ui| {
                for bill in filtered {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&bill.bill_number).strong());
                        ui.label(&bill.vendor_name);
                        ui.label(bill.issue_date.to_string());
                        ui.label(bill.due_date.to_string());
                        ui.label(
                            RichText::new(crate::core::money::format_money_by_code(
                                &bill.currency,
                                bill.total_amount,
                            ))
                            .strong(),
                        );

                        let badge_color = match bill.status {
                            BillStatus::Paid => Theme::success(),
                            BillStatus::Partial => Theme::warning(),
                            BillStatus::Unpaid => Theme::danger(),
                        };
                        Theme::status_badge(ui, bill.status.as_str(), badge_color);

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if bill.status != BillStatus::Paid
                                && ui
                                    .button(RichText::new("Mark Paid").color(Theme::success()))
                                    .clicked()
                            {
                                mark_bill_as_paid(repo, bill, notification);
                            }
                        });
                    });
                    ui.separator();
                }
            });
    });
}

fn render_bill_form(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut PurchasesState,
    vendors: &[Vendor],
    notification: &mut Option<(String, bool)>,
) {
    let form = &mut state.bill_form;
    let products = repo.get_products().unwrap_or_default();
    let purchasable: Vec<&Product> = products.iter().filter(|p| p.kind.purchasable()).collect();

    Theme::card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Record Vendor Bill");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("← Back to List").clicked() {
                    form.is_open = false;
                }
            });
        });
        ui.separator();

        if let Some(err) = &form.error_msg {
            ui.label(RichText::new(err).color(Theme::danger()).strong());
            ui.add_space(4.0);
        }

        egui::Grid::new("bill_form_top")
            .num_columns(4)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Vendor:");
                let current_vendor = vendors
                    .iter()
                    .find(|v| Some(v.id) == form.selected_vendor_id)
                    .map(|v| v.name.as_str())
                    .unwrap_or("Select a Vendor");

                egui::ComboBox::from_id_salt("vendor_combo")
                    .selected_text(current_vendor)
                    .show_ui(ui, |ui| {
                        for v in vendors {
                            let is_selected = Some(v.id) == form.selected_vendor_id;
                            if ui.selectable_label(is_selected, &v.name).clicked() {
                                form.selected_vendor_id = Some(v.id);
                            }
                        }
                    });

                ui.label(RichText::new("Bill #:").strong());
                ui.text_edit_singleline(&mut form.bill_number);
                ui.end_row();

                ui.label("Issue Date (YYYY-MM-DD):");
                ui.text_edit_singleline(&mut form.issue_date_str);

                ui.label("Due Date (YYYY-MM-DD):");
                ui.text_edit_singleline(&mut form.due_date_str);
                ui.end_row();
            });

        ui.add_space(12.0);
        ui.label(
            RichText::new(
                "Line items — pick from the purchase catalog or type a custom description.",
            )
            .size(11.0)
            .color(Theme::text_muted()),
        );
        ui.separator();

        let mut to_remove = None;
        let mut subtotal = Decimal::ZERO;
        let num_items = form.items.len();
        let display_currency = vendors
            .iter()
            .find(|v| Some(v.id) == form.selected_vendor_id)
            .map(|v| v.currency.clone())
            .unwrap_or_else(|| repo.get_base_currency());

        for (idx, item) in form.items.iter_mut().enumerate() {
            let qty = Decimal::from_str(&item.quantity).unwrap_or(Decimal::ZERO);
            let price = Decimal::from_str(&item.unit_price).unwrap_or(Decimal::ZERO);
            let line_total = qty * price;
            subtotal += line_total;

            ui.horizontal(|ui| {
                ui.label(format!("{}.", idx + 1));

                let selected_name = purchasable
                    .iter()
                    .find(|p| p.name == item.description && p.unit == item.unit)
                    .map(|p| p.name.as_str())
                    .unwrap_or("Catalog...");
                egui::ComboBox::from_id_salt(("bill_product_combo", idx))
                    .selected_text(selected_name)
                    .show_ui(ui, |ui| {
                        for p in &purchasable {
                            if ui
                                .selectable_label(p.name == item.description, &p.name)
                                .clicked()
                            {
                                item.description = p.name.clone();
                                item.unit = p.unit.clone();
                                item.unit_price = p.default_price.to_string();
                            }
                        }
                    });

                ui.add(
                    egui::TextEdit::singleline(&mut item.description)
                        .hint_text("Item or service description")
                        .desired_width(180.0),
                );
                ui.label("Qty:");
                ui.add(egui::TextEdit::singleline(&mut item.quantity).desired_width(45.0));
                ui.label("Unit:");
                ui.add(egui::TextEdit::singleline(&mut item.unit).desired_width(50.0));
                ui.label(format!("Price ({}):", display_currency));
                ui.add(egui::TextEdit::singleline(&mut item.unit_price).desired_width(70.0));
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
            form.items.remove(idx);
        }

        if ui.button("+ Add Line Item").clicked() {
            form.items.push(BillLineState {
                id: Uuid::new_v4(),
                description: String::new(),
                quantity: "1".to_string(),
                unit: "Pcs".to_string(),
                unit_price: "0.00".to_string(),
            });
        }

        ui.add_space(16.0);
        ui.separator();

        let tax_rate = Decimal::from_str(&form.tax_rate_str).unwrap_or(Decimal::ZERO);
        let tax_amount = subtotal * (tax_rate / Decimal::from(100));
        let total_amount = subtotal + tax_amount;

        ui.columns(2, |cols| {
            cols[0].vertical(|ui| {
                ui.label("Tax Rate (%):");
                ui.add(egui::TextEdit::singleline(&mut form.tax_rate_str).desired_width(60.0));
                ui.label("Notes:");
                ui.text_edit_multiline(&mut form.notes);
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
                    ui.label(format!("Tax ({}%):", tax_rate));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(crate::core::money::format_money_by_code(
                            &display_currency,
                            tax_amount,
                        ));
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.heading(RichText::new("Total Payable:").size(18.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.heading(
                            RichText::new(crate::core::money::format_money_by_code(
                                &display_currency,
                                total_amount,
                            ))
                            .color(Theme::danger())
                            .size(20.0),
                        );
                    });
                });
            });
        });

        ui.add_space(16.0);

        ui.horizontal(|ui| {
            if ui
                .button(
                    RichText::new("Post & Record Bill")
                        .color(Theme::text_strong())
                        .strong(),
                )
                .clicked()
            {
                save_and_post_bill(
                    repo,
                    form,
                    vendors,
                    subtotal,
                    tax_rate,
                    tax_amount,
                    total_amount,
                    notification,
                );
            }
            if ui.button("Cancel").clicked() {
                form.is_open = false;
            }
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn save_and_post_bill(
    repo: &Repository,
    form: &mut BillFormState,
    vendors: &[Vendor],
    subtotal: Decimal,
    tax_rate: Decimal,
    tax_amount: Decimal,
    total_amount: Decimal,
    notification: &mut Option<(String, bool)>,
) {
    let vendor = match vendors
        .iter()
        .find(|v| Some(v.id) == form.selected_vendor_id)
    {
        Some(v) => v,
        None => {
            form.error_msg = Some("Please select a vendor".to_string());
            return;
        }
    };

    if form.bill_number.trim().is_empty() {
        form.error_msg = Some("Bill number cannot be empty".to_string());
        return;
    }

    let issue_date = match NaiveDate::parse_from_str(&form.issue_date_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            form.error_msg = Some("Invalid Issue Date format (use YYYY-MM-DD)".to_string());
            return;
        }
    };

    let due_date = match NaiveDate::parse_from_str(&form.due_date_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => {
            form.error_msg = Some("Invalid Due Date format (use YYYY-MM-DD)".to_string());
            return;
        }
    };

    let items: Vec<InvoiceItem> = form
        .items
        .iter()
        .map(|it| {
            let q = Decimal::from_str(&it.quantity).unwrap_or(Decimal::ONE);
            let p = Decimal::from_str(&it.unit_price).unwrap_or(Decimal::ZERO);
            InvoiceItem {
                id: it.id,
                description: it.description.clone(),
                quantity: q,
                unit: it.unit.trim().to_string(),
                unit_price: p,
                discount_pct: Decimal::ZERO,
                amount: q * p,
            }
        })
        .collect();

    let bill = VendorBill {
        id: Uuid::new_v4(),
        bill_number: form.bill_number.trim().to_string(),
        vendor_id: vendor.id,
        vendor_name: vendor.name.clone(),
        issue_date,
        due_date,
        status: BillStatus::Unpaid,
        items,
        subtotal,
        tax_rate,
        tax_amount,
        total_amount,
        currency: vendor.currency.clone(),
        notes: form.notes.trim().to_string(),
        journal_entry_id: None,
        created_at: Utc::now(),
    };

    // Auto-post: Debit Expense (+ input tax), Credit Accounts Payable.
    let accounts = repo.get_accounts().unwrap_or_default();
    let ap_acc = accounts.iter().find(|a| a.code == "2000").cloned();
    let tax_acc = accounts.iter().find(|a| a.code == "2200").cloned();
    let expense_acc = accounts.iter().find(|a| a.code == "6000").cloned();

    if let (Some(ap), Some(expense)) = (ap_acc, expense_acc) {
        let seq = repo.next_journal_sequence().unwrap_or(1);
        if let Ok(je) = AccountingEngine::create_bill_journal_entry(
            &bill,
            &ap,
            &expense,
            &tax_acc.unwrap(),
            seq,
        ) {
            let _ = repo.insert_journal_entry(&je, true);
        }
    }

    match repo.insert_vendor_bill(&bill, true) {
        Ok(_) => {
            *notification = Some((
                format!(
                    "Bill {} recorded — posted to Accounts Payable!",
                    bill.bill_number
                ),
                true,
            ));
            form.is_open = false;
        }
        Err(e) => {
            form.error_msg = Some(format!("Database error: {}", e));
        }
    }
}

fn mark_bill_as_paid(
    repo: &Repository,
    bill: &VendorBill,
    notification: &mut Option<(String, bool)>,
) {
    let mut updated = bill.clone();
    updated.status = BillStatus::Paid;

    let accounts = repo.get_accounts().unwrap_or_default();
    let cash_acc = accounts.iter().find(|a| a.code == "1000").cloned();
    let ap_acc = accounts.iter().find(|a| a.code == "2000").cloned();

    if let (Some(cash), Some(ap)) = (cash_acc, ap_acc) {
        let seq = repo.next_journal_sequence().unwrap_or(1);
        if let Ok(je) =
            AccountingEngine::create_bill_payment_journal_entry(&updated, &cash, &ap, seq)
        {
            let _ = repo.insert_journal_entry(&je, true);
        }
    }

    match repo.insert_vendor_bill(&updated, true) {
        Ok(_) => {
            *notification = Some((
                format!("Bill {} marked as PAID — cash disbursed!", bill.bill_number),
                true,
            ));
        }
        Err(e) => {
            *notification = Some((format!("Error updating bill: {}", e), false));
        }
    }
}

// --- Vendors ---

fn render_vendors(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut PurchasesState,
    notification: &mut Option<(String, bool)>,
) {
    let vendors = repo.get_vendors().unwrap_or_default();

    if state.vendor_form.is_open {
        render_vendor_form(ui, repo, state, notification);
        return;
    }

    Theme::card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Vendors");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(
                        RichText::new("+ Add Vendor")
                            .color(Theme::text_strong())
                            .strong(),
                    )
                    .clicked()
                {
                    state.vendor_form = VendorFormState::default();
                    state.vendor_form.is_open = true;
                }
            });
        });
        ui.separator();

        if vendors.is_empty() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No vendors yet.").color(Theme::text_muted()));
                ui.label("Add your first supplier to start recording bills.");
            });
            ui.add_space(16.0);
            return;
        }

        egui::Grid::new("vendors_table_header")
            .num_columns(6)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Vendor").strong());
                ui.label(RichText::new("Code").strong());
                ui.label(RichText::new("Phone").strong());
                ui.label(RichText::new("City").strong());
                ui.label(RichText::new("Currency").strong());
                ui.label(RichText::new("Terms").strong());
                ui.end_row();
            });
        ui.separator();

        let list_height = (ui.available_height() - 16.0).max(240.0);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .max_height(list_height)
            .show(ui, |ui| {
                for v in &vendors {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&v.name).strong());
                        ui.label(
                            RichText::new(format!("[{}]", v.code))
                                .color(Theme::primary())
                                .size(12.0),
                        );
                        ui.label(&v.phone);
                        ui.label(&v.city);
                        ui.label(&v.currency);
                        ui.label(format!("Net {} days", v.payment_terms_days));
                    });
                    ui.separator();
                }
            });
    });
}

fn render_vendor_form(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut PurchasesState,
    notification: &mut Option<(String, bool)>,
) {
    let form = &mut state.vendor_form;

    Theme::card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("New Vendor");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("← Back to List").clicked() {
                    form.is_open = false;
                }
            });
        });
        ui.separator();

        if let Some(err) = &form.error_msg {
            ui.label(RichText::new(err).color(Theme::danger()).strong());
            ui.add_space(4.0);
        }

        egui::Grid::new("vendor_form_grid")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Vendor Name:");
                ui.text_edit_singleline(&mut form.name);
                ui.end_row();

                ui.label("Email:");
                ui.text_edit_singleline(&mut form.email);
                ui.end_row();

                ui.label("Phone:");
                ui.text_edit_singleline(&mut form.phone);
                ui.end_row();

                ui.label("Street Address:");
                ui.text_edit_singleline(&mut form.address);
                ui.end_row();

                ui.label("City:");
                ui.text_edit_singleline(&mut form.city);
                ui.end_row();

                ui.label("Country:");
                ui.text_edit_singleline(&mut form.country);
                ui.end_row();

                ui.label("Postal Code:");
                ui.text_edit_singleline(&mut form.postal_code);
                ui.end_row();

                ui.label("Currency:");
                ui.text_edit_singleline(&mut form.currency);
                ui.end_row();

                ui.label("Payment Terms (days):");
                ui.text_edit_singleline(&mut form.payment_terms);
                ui.end_row();

                ui.label("Notes:");
                ui.text_edit_multiline(&mut form.notes);
                ui.end_row();
            });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui
                .button(
                    RichText::new("Save Vendor")
                        .color(Theme::text_strong())
                        .strong(),
                )
                .clicked()
            {
                save_vendor(repo, form, notification);
            }
            if ui.button("Cancel").clicked() {
                form.is_open = false;
            }
        });
    });
}

fn save_vendor(
    repo: &Repository,
    form: &mut VendorFormState,
    notification: &mut Option<(String, bool)>,
) {
    if form.name.trim().is_empty() {
        form.error_msg = Some("Vendor name is required".to_string());
        return;
    }

    let code = NumberingEngine::derive_customer_code(&form.name);
    let terms = form.payment_terms.trim().parse::<u32>().unwrap_or(30);
    let currency = if form.currency.trim().is_empty() {
        repo.get_base_currency()
    } else {
        form.currency.trim().to_uppercase()
    };

    let vendor = Vendor {
        id: Uuid::new_v4(),
        name: form.name.trim().to_string(),
        code,
        email: form.email.trim().to_string(),
        phone: form.phone.trim().to_string(),
        address: form.address.trim().to_string(),
        city: form.city.trim().to_string(),
        country: form.country.trim().to_string(),
        postal_code: form.postal_code.trim().to_string(),
        currency,
        payment_terms_days: terms,
        notes: form.notes.trim().to_string(),
        created_at: Utc::now(),
    };

    match repo.insert_vendor(&vendor, true) {
        Ok(_) => {
            *notification = Some((format!("Vendor '{}' saved!", vendor.name), true));
            form.is_open = false;
        }
        Err(e) => {
            form.error_msg = Some(format!("Database error: {}", e));
        }
    }
}

// --- Products & Services Catalog ---

const PRODUCT_KINDS: [ProductKind; 3] =
    [ProductKind::Sale, ProductKind::Purchase, ProductKind::Both];

fn render_catalog(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut PurchasesState,
    notification: &mut Option<(String, bool)>,
) {
    let products = repo.get_products().unwrap_or_default();

    if state.product_form.is_open {
        render_product_form(ui, repo, state, notification);
        return;
    }

    Theme::card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Products & Services");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(
                        RichText::new("+ Add Product / Service")
                            .color(Theme::text_strong())
                            .strong(),
                    )
                    .clicked()
                {
                    state.product_form = ProductFormState::default();
                    state.product_form.is_open = true;
                }
            });
        });
        ui.separator();

        if products.is_empty() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Catalog is empty.").color(Theme::text_muted()));
                ui.label("Add sellable and purchasable items to speed up document creation.");
            });
            ui.add_space(16.0);
            return;
        }

        egui::Grid::new("products_table_header")
            .num_columns(6)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Name").strong());
                ui.label(RichText::new("Kind").strong());
                ui.label(RichText::new("Unit").strong());
                ui.label(RichText::new("Default Price").strong());
                ui.label(RichText::new("Tax %").strong());
                ui.label(RichText::new("GL Accounts").strong());
                ui.end_row();
            });
        ui.separator();

        let list_height = (ui.available_height() - 16.0).max(240.0);
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .max_height(list_height)
            .show(ui, |ui| {
                for p in &products {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&p.name).strong());
                        ui.label(p.kind.as_str());
                        ui.label(&p.unit);
                        ui.label(crate::core::money::format_money_by_code(
                            &repo.get_base_currency(),
                            p.default_price,
                        ));
                        ui.label(format!("{}", p.tax_rate));
                        ui.label(
                            RichText::new(format!(
                                "{} / {}",
                                p.income_account_code, p.expense_account_code
                            ))
                            .size(12.0)
                            .color(Theme::text_muted()),
                        );
                    });
                    ui.separator();
                }
            });
    });
}

fn render_product_form(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut PurchasesState,
    notification: &mut Option<(String, bool)>,
) {
    let form = &mut state.product_form;

    Theme::card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("New Product / Service");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("← Back to List").clicked() {
                    form.is_open = false;
                }
            });
        });
        ui.separator();

        if let Some(err) = &form.error_msg {
            ui.label(RichText::new(err).color(Theme::danger()).strong());
            ui.add_space(4.0);
        }

        egui::Grid::new("product_form_grid")
            .num_columns(2)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut form.name);
                ui.end_row();

                ui.label("Kind:");
                egui::ComboBox::from_id_salt("product_kind_combo")
                    .selected_text(PRODUCT_KINDS[form.kind_idx].as_str())
                    .show_ui(ui, |ui| {
                        for (idx, kind) in PRODUCT_KINDS.iter().enumerate() {
                            if ui
                                .selectable_label(idx == form.kind_idx, kind.as_str())
                                .clicked()
                            {
                                form.kind_idx = idx;
                            }
                        }
                    });
                ui.end_row();

                ui.label("Unit:");
                ui.text_edit_singleline(&mut form.unit);
                ui.end_row();

                ui.label(format!("Default Price ({}):", repo.get_base_currency()));
                ui.text_edit_singleline(&mut form.default_price);
                ui.end_row();

                ui.label("Tax Rate (%):");
                ui.text_edit_singleline(&mut form.tax_rate);
                ui.end_row();

                ui.label("Income Account (sales):");
                ui.text_edit_singleline(&mut form.income_account);
                ui.end_row();

                ui.label("Expense Account (purchases):");
                ui.text_edit_singleline(&mut form.expense_account);
                ui.end_row();
            });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui
                .button(
                    RichText::new("Save Item")
                        .color(Theme::text_strong())
                        .strong(),
                )
                .clicked()
            {
                save_product(repo, form, notification);
            }
            if ui.button("Cancel").clicked() {
                form.is_open = false;
            }
        });
    });
}

fn save_product(
    repo: &Repository,
    form: &mut ProductFormState,
    notification: &mut Option<(String, bool)>,
) {
    if form.name.trim().is_empty() {
        form.error_msg = Some("Item name is required".to_string());
        return;
    }

    let product = Product {
        id: Uuid::new_v4(),
        name: form.name.trim().to_string(),
        kind: PRODUCT_KINDS[form.kind_idx],
        unit: if form.unit.trim().is_empty() {
            "Pcs".to_string()
        } else {
            form.unit.trim().to_string()
        },
        default_price: Decimal::from_str(&form.default_price).unwrap_or(Decimal::ZERO),
        tax_rate: Decimal::from_str(&form.tax_rate).unwrap_or(Decimal::ZERO),
        income_account_code: form.income_account.trim().to_string(),
        expense_account_code: form.expense_account.trim().to_string(),
        active: true,
        created_at: Utc::now(),
    };

    match repo.insert_product(&product, true) {
        Ok(_) => {
            *notification = Some((format!("Item '{}' saved to catalog!", product.name), true));
            form.is_open = false;
        }
        Err(e) => {
            form.error_msg = Some(format!("Database error: {}", e));
        }
    }
}
