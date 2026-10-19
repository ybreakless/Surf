use eframe::egui::{self, Color32, RichText};
use rust_decimal::Decimal;

use super::theme::Theme;
use crate::core::model::InvoiceStatus;
use crate::core::money::format_money_by_code;
use crate::core::reports;
use crate::db::repository::Repository;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ReportView {
    #[default]
    Overview,
    TrialBalance,
    IncomeStatement,
    BalanceSheet,
    Aging,
}

#[derive(Default)]
pub struct ReportsState {
    pub view: ReportView,
}

pub fn render(ui: &mut egui::Ui, repo: &Repository, state: &mut ReportsState) {
    ui.add_space(8.0);
    let accounts = repo.get_accounts().unwrap_or_default();
    let invoices = repo.get_invoices().unwrap_or_default();
    let base = repo.get_base_currency();

    Theme::card_frame().show(ui, |ui| {
        ui.heading("Reports Center");
        ui.label(
            RichText::new("Premium accounting reports computed live from your local ledger.")
                .size(11.0)
                .color(Theme::text_muted()),
        );
        ui.separator();

        ui.horizontal(|ui| {
            view_button(ui, state, ReportView::Overview, "Overview");
            view_button(ui, state, ReportView::TrialBalance, "Trial Balance");
            view_button(ui, state, ReportView::IncomeStatement, "Income Statement");
            view_button(ui, state, ReportView::BalanceSheet, "Balance Sheet");
            view_button(ui, state, ReportView::Aging, "Aged Receivables");
        });

        ui.add_space(8.0);
        match state.view {
            ReportView::Overview => overview(ui, &accounts, &invoices, &base),
            ReportView::TrialBalance => trial_balance(ui, &accounts, &base),
            ReportView::IncomeStatement => income_statement(ui, &accounts, &base),
            ReportView::BalanceSheet => balance_sheet(ui, &accounts, &base),
            ReportView::Aging => aging(ui, &invoices),
        }
    });
}

fn view_button(ui: &mut egui::Ui, state: &mut ReportsState, view: ReportView, label: &str) {
    if ui
        .add(
            egui::Button::new(RichText::new(label).strong().size(12.5))
                .fill(if state.view == view {
                    Theme::primary()
                } else {
                    Color32::TRANSPARENT
                })
                .corner_radius(egui::CornerRadius::same(5)),
        )
        .clicked()
    {
        state.view = view;
    }
}

fn amount(currency: &str, value: Decimal) -> String {
    format_money_by_code(currency, value)
}

fn overview(
    ui: &mut egui::Ui,
    accounts: &[crate::core::model::Account],
    invoices: &[crate::core::model::Invoice],
    base: &str,
) {
    let is = reports::income_statement(accounts);
    let bs = reports::balance_sheet(accounts);
    let rows = reports::trial_balance(accounts);
    let (d, c) = reports::trial_balance_totals(&rows);

    ui.columns(3, |cols| {
        metric_card(
            &mut cols[0],
            "Revenue (Credits)",
            &amount(base, is.revenue),
            Theme::success(),
        );
        metric_card(
            &mut cols[1],
            "Expenses",
            &amount(base, is.expenses),
            Theme::danger(),
        );
        metric_card(
            &mut cols[2],
            "Net Income",
            &amount(base, is.net_income),
            Theme::primary(),
        );
    });
    ui.add_space(8.0);
    ui.columns(3, |cols| {
        metric_card(
            &mut cols[0],
            "Assets",
            &amount(base, bs.assets),
            Theme::primary(),
        );
        metric_card(
            &mut cols[1],
            "Liabilities",
            &amount(base, bs.liabilities),
            Theme::warning(),
        );
        metric_card(
            &mut cols[2],
            "Debits = Credits",
            &format!("{d} \u{2714} {c}"),
            Theme::success(),
        );
    });
    ui.add_space(8.0);
    let outstanding: Decimal = invoices
        .iter()
        .filter(|i| i.status == InvoiceStatus::Sent)
        .map(|i| i.total_amount)
        .sum();
    let overdue: Decimal = invoices
        .iter()
        .filter(|i| i.status == InvoiceStatus::Sent && chrono::Utc::now().date_naive() > i.due_date)
        .map(|i| i.total_amount)
        .sum();
    ui.columns(2, |cols| {
        metric_card(
            &mut cols[0],
            "Open Receivables",
            &amount(base, outstanding),
            Theme::danger(),
        );
        metric_card(
            &mut cols[1],
            "Past-Due Receivables",
            &amount(base, overdue),
            Theme::danger(),
        );
    });
    ui.add_space(12.0);
    ui.label(
        RichText::new(
            "Pick a detailed report above. Values are computed live from the double-entry ledger (base currency shown once set).",
        )
        .size(11.0)
        .color(Theme::text_muted()),
    );
}

fn metric_card(ui: &mut egui::Ui, title: &str, value: &str, color: Color32) {
    egui::Frame::new()
        .fill(Theme::bg_card_light())
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.label(
                RichText::new(title)
                    .size(11.0)
                    .color(Theme::text_muted())
                    .strong(),
            );
            ui.label(RichText::new(value).size(18.0).color(color).strong());
        });
}

fn trial_balance(ui: &mut egui::Ui, accounts: &[crate::core::model::Account], base: &str) {
    let rows = reports::trial_balance(accounts);
    let (d, c) = reports::trial_balance_totals(&rows);
    ui.separator();
    egui::Grid::new("tb_grid")
        .num_columns(5)
        .spacing([14.0, 5.0])
        .show(ui, |ui| {
            header_cell(ui, "Code");
            header_cell(ui, "Account");
            header_cell(ui, "Type");
            header_cell(ui, "Debit");
            header_cell(ui, "Credit");
            ui.end_row();
            for (r, a) in rows.iter().zip(accounts.iter()) {
                ui.label(&r.code);
                ui.label(&r.name);
                ui.label(format!("{:?}", a.account_type));
                amount_cell(ui, base, r.debit);
                amount_cell(ui, base, r.credit);
                ui.end_row();
            }
        });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("TOTALS").strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("DR {} | CR {}", amount(base, d), amount(base, c))).strong(),
            );
        });
    });
    if d == c {
        ui.label(
            RichText::new("\u{2714} Trial balance is balanced.")
                .color(Theme::success())
                .strong(),
        );
    } else {
        ui.label(
            RichText::new(format!("\u{26A0} Unbalanced by {}", amount(base, d - c)))
                .color(Theme::danger())
                .strong(),
        );
    }
}

fn income_statement(ui: &mut egui::Ui, accounts: &[crate::core::model::Account], base: &str) {
    let is = reports::income_statement(accounts);
    ui.separator();
    report_row(
        ui,
        "Revenue (credit-normal accounts)",
        &amount(base, is.revenue),
        Theme::success(),
    );
    report_row(
        ui,
        "Expenses (debit-normal accounts)",
        &amount(base, is.expenses),
        Theme::danger(),
    );
    ui.separator();
    report_row(
        ui,
        "Net Income",
        &amount(base, is.net_income),
        Theme::primary(),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(format!(
            "Ledger basis: accrual. Net income flows into equity on the Balance Sheet. Ledger base currency: {base}"
        ))
        .size(11.0)
        .color(Theme::text_muted()),
    );
}

fn balance_sheet(ui: &mut egui::Ui, accounts: &[crate::core::model::Account], base: &str) {
    let bs = reports::balance_sheet(accounts);
    ui.separator();
    report_row(ui, "Assets", &amount(base, bs.assets), Theme::primary());
    report_row(
        ui,
        "Liabilities",
        &amount(base, bs.liabilities),
        Theme::warning(),
    );
    report_row(ui, "Equity", &amount(base, bs.equity), Theme::text_strong());
    report_row(
        ui,
        "Current Net Income",
        &amount(base, bs.net_income),
        Theme::success(),
    );
    ui.separator();
    let total = bs.liabilities + bs.equity + bs.net_income;
    report_row(
        ui,
        "Total Liabilities + Equity",
        &amount(base, total),
        Theme::primary(),
    );
}

fn aging(ui: &mut egui::Ui, invoices: &[crate::core::model::Invoice]) {
    let today = chrono::Utc::now().date_naive();
    let rows = reports::aged_receivables(invoices, today);
    ui.separator();
    if rows.is_empty() {
        ui.label(RichText::new("\u{2714} No outstanding receivables.").color(Theme::success()));
        return;
    }
    egui::Grid::new("aging_grid")
        .num_columns(8)
        .spacing([12.0, 5.0])
        .show(ui, |ui| {
            header_cell(ui, "Customer");
            header_cell(ui, "CCY");
            header_cell(ui, "Current");
            header_cell(ui, "1-30");
            header_cell(ui, "31-60");
            header_cell(ui, "61-90");
            header_cell(ui, "90+");
            header_cell(ui, "Total");
            ui.end_row();
            for r in &rows {
                ui.label(&r.customer_name);
                ui.label(&r.currency);
                bucket_cell(ui, r.currency.clone(), r.current);
                bucket_cell(ui, r.currency.clone(), r.b30);
                bucket_cell(ui, r.currency.clone(), r.b60);
                bucket_cell(ui, r.currency.clone(), r.b90);
                bucket_cell(ui, r.currency.clone(), r.b90plus);
                ui.label(RichText::new(amount(&r.currency, r.total)).strong());
                ui.end_row();
            }
        });
}

fn header_cell(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).strong().color(Theme::text_muted()));
}

fn amount_cell(ui: &mut egui::Ui, base: &str, value: Decimal) {
    if value == Decimal::ZERO {
        ui.label(RichText::new("\u{2013}").color(Theme::text_muted()));
    } else {
        ui.label(RichText::new(amount(base, value)).strong());
    }
}

fn bucket_cell(ui: &mut egui::Ui, currency: String, value: Decimal) {
    if value == Decimal::ZERO {
        ui.label(RichText::new("\u{2013}").color(Theme::text_muted()));
    } else {
        ui.label(RichText::new(amount(&currency, value)).strong());
    }
}

fn report_row(ui: &mut egui::Ui, label: &str, value: &str, color: Color32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).color(color).strong().size(14.0));
        });
    });
}
