use eframe::egui::{self, Color32, RichText};
use rust_decimal::Decimal;

use super::theme::Theme;
use crate::core::model::AccountType;
use crate::db::repository::Repository;

pub fn render(ui: &mut egui::Ui, repo: &Repository) {
    ui.add_space(8.0);

    let accounts = repo.get_accounts().unwrap_or_default();
    let journal_entries = repo.get_journal_entries().unwrap_or_default();

    // Calculate total debits and credits across all journal lines to verify balance
    let mut total_debits = Decimal::ZERO;
    let mut total_credits = Decimal::ZERO;

    for je in &journal_entries {
        for line in &je.lines {
            total_debits += line.debit;
            total_credits += line.credit;
        }
    }

    let is_balanced = total_debits == total_credits;

    // Header banner with double-entry integrity check
    let banner_frame = egui::Frame::new()
        .fill(if is_balanced {
            Color32::from_rgb(20, 35, 25)
        } else {
            Color32::from_rgb(45, 20, 20)
        })
        .stroke(egui::Stroke::new(
            1.0,
            if is_balanced {
                Theme::SUCCESS
            } else {
                Theme::DANGER
            },
        ))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::same(12));

    banner_frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            if is_balanced {
                ui.label(
                    RichText::new("✓ DOUBLE-ENTRY INTEGRITY VERIFIED")
                        .color(Theme::SUCCESS)
                        .strong(),
                );
                ui.label(
                    RichText::new("Debits equal Credits across all postings.")
                        .color(Color32::LIGHT_GRAY),
                );
            } else {
                ui.label(
                    RichText::new("⚠ LEDGER IMBALANCE DETECTED")
                        .color(Theme::DANGER)
                        .strong(),
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!(
                        "Total Credits: {}",
                        crate::core::money::format_money_by_code(
                            &repo.get_base_currency(),
                            total_credits
                        )
                    ))
                    .strong(),
                );
                ui.label(RichText::new(" | ").color(Theme::TEXT_MUTED));
                ui.label(
                    RichText::new(format!(
                        "Total Debits: {}",
                        crate::core::money::format_money_by_code(
                            &repo.get_base_currency(),
                            total_debits
                        )
                    ))
                    .strong(),
                );
            });
        });
    });

    ui.add_space(16.0);

    ui.columns(2, |cols| {
        // --- Left Column: Chart of Accounts ---
        Theme::card_frame().show(&mut cols[0], |ui| {
            ui.heading("Chart of Accounts");
            ui.separator();

            egui::Grid::new("coa_table_header")
                .num_columns(4)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Code").strong());
                    ui.label(RichText::new("Account Name").strong());
                    ui.label(RichText::new("Type").strong());
                    ui.label(RichText::new("Balance").strong());
                    ui.end_row();
                });
            ui.separator();

            egui::ScrollArea::vertical()
                .max_height(480.0)
                .show(ui, |ui| {
                    for acc in &accounts {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&acc.code).strong());
                            ui.label(&acc.name);

                            let type_color = match acc.account_type {
                                AccountType::Asset => Theme::PRIMARY,
                                AccountType::Liability => Theme::WARNING,
                                AccountType::Equity => Color32::LIGHT_BLUE,
                                AccountType::Revenue => Theme::SUCCESS,
                                AccountType::Expense => Color32::from_rgb(220, 120, 120),
                            };
                            ui.label(
                                RichText::new(format!("{:?}", acc.account_type))
                                    .color(type_color)
                                    .size(11.0),
                            );

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(crate::core::money::format_money_by_code(
                                            &repo.get_base_currency(),
                                            acc.balance,
                                        ))
                                        .strong(),
                                    );
                                },
                            );
                        });
                        ui.separator();
                    }
                });
        });

        // --- Right Column: Journal Entries Log ---
        Theme::card_frame().show(&mut cols[1], |ui| {
            ui.heading("General Ledger Journal");
            ui.separator();

            if journal_entries.is_empty() {
                ui.label(RichText::new("No journal entries posted yet.").color(Theme::TEXT_MUTED));
                return;
            }

            egui::ScrollArea::vertical()
                .max_height(480.0)
                .show(ui, |ui| {
                    for je in &journal_entries {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&je.entry_number).strong());
                            ui.label(je.entry_date.to_string());
                            if let Some(ref_no) = &je.reference {
                                ui.label(
                                    RichText::new(format!("Ref: {}", ref_no))
                                        .color(Theme::PRIMARY)
                                        .size(11.0),
                                );
                            }
                        });
                        ui.label(
                            RichText::new(&je.memo)
                                .color(Color32::LIGHT_GRAY)
                                .size(12.0),
                        );

                        // Show splits / lines
                        for line in &je.lines {
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                ui.label(
                                    RichText::new(format!(
                                        "{}: {}",
                                        line.account_code, line.account_name
                                    ))
                                    .size(11.0),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if line.credit > Decimal::ZERO {
                                            ui.label(
                                                RichText::new(format!(
                                                    "Cr: {}",
                                                    crate::core::money::format_money_by_code(
                                                        &repo.get_base_currency(),
                                                        line.credit
                                                    )
                                                ))
                                                .color(Theme::WARNING)
                                                .size(11.0),
                                            );
                                        }
                                        if line.debit > Decimal::ZERO {
                                            ui.label(
                                                RichText::new(format!(
                                                    "Dr: {}",
                                                    crate::core::money::format_money_by_code(
                                                        &repo.get_base_currency(),
                                                        line.debit
                                                    )
                                                ))
                                                .color(Theme::SUCCESS)
                                                .size(11.0),
                                            );
                                        }
                                    },
                                );
                            });
                        }
                        ui.separator();
                    }
                });
        });
    });
}
