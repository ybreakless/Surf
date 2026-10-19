use eframe::egui::{self, RichText};
use rust_decimal::Decimal;
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

use super::theme::Theme;
use crate::core::fx::{FxRate, RateSource, convert, format_rate, parse_rate_str, resolve_rate};
use crate::core::money::{Currency, format_money_by_code, format_toman};
use crate::db::repository::Repository;
use crate::services::fx_provider::{self, FxUpdateMsg};

#[derive(Default)]
pub struct FxState {
    pub update_running: Option<std::sync::mpsc::Receiver<FxUpdateMsg>>,
    pub manual_currency: Currency,
    pub manual_rate: String,
    pub manual_date: String,
    pub conv_amount: String,
    pub conv_from: Currency,
    pub conv_to: Currency,
}

impl FxState {
    pub fn new() -> Self {
        Self {
            manual_currency: Currency::USD,
            manual_date: chrono::Utc::now().date_naive().to_string(),
            conv_amount: "1000".to_string(),
            conv_from: Currency::USD,
            conv_to: Currency::IRR,
            ..Self::default()
        }
    }
}

pub fn render(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut FxState,
    notification: &mut Option<(String, bool)>,
) {
    ui.add_space(8.0);

    // Poll background updater
    if let Some(rx) = &state.update_running {
        let mut done = None;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                FxUpdateMsg::Progress(_) => {}
                FxUpdateMsg::Done(res) => {
                    done = Some(res);
                    break;
                }
            }
        }
        if let Some(res) = done {
            state.update_running = None;
            match res {
                Ok((saved, failed)) => {
                    if failed.is_empty() {
                        *notification = Some((
                            format!("FX rates updated: {} currencies saved.", saved),
                            true,
                        ));
                    } else {
                        *notification = Some((
                            format!(
                                "FX update finished: {} saved, {} failed: {}",
                                saved,
                                failed.len(),
                                failed.join(", ")
                            ),
                            false,
                        ));
                    }
                }
                Err(e) => {
                    *notification = Some((format!("FX update failed: {}", e), false));
                }
            }
        } else {
            ui.label(RichText::new("Updating rates from tgju.org...").color(Theme::primary()));
            ui.ctx().request_repaint_after(Duration::from_millis(250));
        }
    }

    let rates = repo.get_all_fx_rates().unwrap_or_default();

    // Header banner
    let all_fresh = Currency::ALL.iter().all(|c| {
        *c == Currency::IRR || resolve_rate(*c, chrono::Utc::now().date_naive(), &rates).is_some()
    });

    let banner = egui::Frame::new()
        .fill(Theme::bg_card_light())
        .stroke(egui::Stroke::new(
            1.0,
            if all_fresh {
                Theme::success()
            } else {
                Theme::warning()
            },
        ))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(12, 8));

    banner.show(ui, |ui| {
        ui.horizontal(|ui| {
            let status = if all_fresh { "FX DATA CURRENT" } else { "FX DATA MISSING OR STALE" };
            ui.label(
                RichText::new(status)
                    .color(if all_fresh { Theme::success() } else { Theme::warning() })
                    .strong(),
            );
            ui.label(
                RichText::new("Rates are stored as IRR per 1 unit of foreign currency (free-market source: tgju.org).")
                    .size(11.0)
                    .color(Theme::text_muted()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let running = state.update_running.is_some();
                let btn = egui::Button::new(
                    RichText::new("Update from tgju").color(Theme::text_strong()).strong(),
                );
                if ui.add_enabled(!running, btn).clicked() {
                    state.update_running =
                        Some(fx_provider::spawn_fx_update_worker(repo.db_path().to_path_buf()));
                    *notification = Some(("FX update started in background.".to_string(), true));
                }
            });
        });
    });

    ui.add_space(16.0);

    ui.columns(2, |cols| {
        // --- Current rates table ---
        Theme::card_frame().show(&mut cols[0], |ui| {
            ui.heading(RichText::new("Current FX Rates (1 FX = N IRR)").size(16.0));
            ui.separator();

            egui::Grid::new("fx_rates_grid")
                .num_columns(4)
                .spacing([14.0, 6.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Currency").strong());
                    ui.label(RichText::new("Rate (IRR per unit)").strong());
                    ui.label(RichText::new("As of").strong());
                    ui.label(RichText::new("Source").strong());
                    ui.end_row();
                });
            ui.separator();

            let latest = repo.get_latest_fx_rates().unwrap_or_default();
            egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                for c in Currency::ALL {
                    if c == Currency::IRR {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(c.code()).strong());
                            ui.label(c.label());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new("Base").size(11.0).color(Theme::primary()));
                            });
                        });
                        ui.separator();
                        continue;
                    }
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(c.code()).strong());
                        ui.label(RichText::new(c.label()).size(11.0).color(Theme::text_muted()));
                        match latest.iter().find(|r| r.quote == c) {
                            Some(r) => {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let (color, age_label) = match r.fetched_at {
                                        Some(t) if chrono::Utc::now().signed_duration_since(t).num_hours() < 30 => (Theme::success(), "fresh"),
                                        _ => (Theme::warning(), "stale"),
                                    };
                                    ui.label(RichText::new(age_label).size(11.0).color(color));
                                    ui.label(
                                        RichText::new(format_rate(r.rate)).strong(),
                                    );
                                    ui.label(
                                        RichText::new(r.as_of_date.to_string())
                                            .size(11.0)
                                            .color(Theme::text_muted()),
                                    );
                                    ui.label(
                                        RichText::new(match r.source {
                                            RateSource::Manual => "manual",
                                            RateSource::Tgju => "tgju.org",
                                            RateSource::Import => "import",
                                        })
                                        .size(11.0)
                                        .color(Theme::primary()),
                                    );
                                });
                            }
                            None => {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new("No rate yet").color(Theme::danger()).size(11.0));
                                });
                            }
                        }
                    });
                    ui.separator();
                }
            });
        });

        // --- Manual rate entry + converter ---
        cols[1].vertical(|ui| {
            Theme::card_frame().show(ui, |ui| {
                ui.heading(RichText::new("Manual Rate Entry").size(16.0));
                ui.separator();
                ui.label(
                    RichText::new("Manual rates override fetched data for their date. Enter how many IRR one unit of the currency is worth.")
                        .size(11.0)
                        .color(Theme::text_muted()),
                );

                egui::Grid::new("fx_manual_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Currency:");
                        egui::ComboBox::from_id_salt("fx_manual_cur")
                            .selected_text(state.manual_currency.code())
                            .show_ui(ui, |ui| {
                                for c in Currency::ALL {
                                    if c == Currency::IRR { continue; }
                                    if ui.selectable_label(state.manual_currency == c, format!("{} - {}", c.code(), c.label())).clicked() {
                                        state.manual_currency = c;
                                    }
                                }
                            });
                        ui.end_row();

                        ui.label("Rate (IRR per unit):");
                        ui.text_edit_singleline(&mut state.manual_rate);
                        ui.end_row();

                        ui.label("Applies to date:");
                        ui.text_edit_singleline(&mut state.manual_date);
                        ui.end_row();
                    });

                ui.add_space(8.0);
                if ui.button(RichText::new("Save Manual Rate").color(Theme::text_strong()).strong()).clicked() {
                    match (parse_rate_str(&state.manual_rate), chrono::NaiveDate::parse_from_str(&state.manual_date, "%Y-%m-%d")) {
                        (Some(rate), Ok(date)) => {
                            let fx = FxRate {
                                id: Uuid::new_v4(),
                                base: Currency::IRR,
                                quote: state.manual_currency,
                                rate,
                                as_of_date: date,
                                source: RateSource::Manual,
                                source_url: None,
                                raw_value: Some(state.manual_rate.trim().to_string()),
                                fetched_at: Some(chrono::Utc::now()),
                                created_at: chrono::Utc::now(),
                            };
                            match repo.upsert_fx_rate(&fx) {
                                Ok(_) => {
                                    *notification = Some((
                                        format!("Manual rate saved: 1 {} = {} IRR as of {}", fx.quote.code(), fx.rate, date),
                                        true,
                                    ));
                                    state.manual_rate.clear();
                                }
                                Err(e) => {
                                    *notification = Some((format!("Failed to save rate: {}", e), false));
                                }
                            }
                        }
                        (Some(_), Err(_)) => {
                            *notification = Some(("Invalid date format, use YYYY-MM-DD".to_string(), false));
                        }
                        (None, _) => {
                            *notification = Some(("Rate must be a positive number".to_string(), false));
                        }
                    }
                }
            });

            ui.add_space(12.0);

            Theme::card_frame().show(ui, |ui| {
                ui.heading(RichText::new("Rate Converter").size(16.0));
                ui.separator();

                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut state.conv_amount).desired_width(110.0));
                });
                ui.horizontal(|ui| {
                    currency_combo(ui, "fx_conv_from", "from", &mut state.conv_from);
                    ui.label("to");
                    currency_combo(ui, "fx_conv_to", "to", &mut state.conv_to);
                });

                ui.add_space(8.0);
                let amount = Decimal::from_str(state.conv_amount.trim()).unwrap_or(Decimal::ZERO);
                match convert(amount, state.conv_from, state.conv_to, &rates) {
                    Some(result) => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format_money_by_code(state.conv_to.code(), result)).strong().size(18.0).color(Theme::primary()));
                        });
                        if state.conv_from == Currency::IRR || state.conv_to == Currency::IRR {
                            ui.label(
                                RichText::new(format!(
                                    "≈ {} ({}, Toman display)",
                                    format_toman(if state.conv_to == Currency::IRR { result } else { amount }, true),
                                    format_toman(if state.conv_to == Currency::IRR { result } else { amount }, false)
                                ))
                                .size(11.0)
                                .color(Theme::text_muted()),
                            );
                        }
                    }
                    None => {
                        ui.label(RichText::new("Add a rate for the target currency first.").color(Theme::danger()));
                    }
                }
            });
        });
    });

    ui.add_space(16.0);

    // --- History ---
    Theme::card_frame().show(ui, |ui| {
        ui.heading(RichText::new("Rate History").size(16.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("{} records stored", rates.len()))
                    .size(11.0)
                    .color(Theme::text_muted()),
            );
        });
        ui.separator();

        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                for r in rates.iter().take(20) {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("1 {} = {} IRR", r.quote.code(), r.rate))
                                .strong(),
                        );
                        ui.label(
                            RichText::new(format!("as of {}", r.as_of_date))
                                .size(11.0)
                                .color(Theme::text_muted()),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(match r.source {
                                    RateSource::Manual => "manual",
                                    RateSource::Tgju => "tgju.org",
                                    RateSource::Import => "import",
                                })
                                .size(11.0)
                                .color(Theme::primary()),
                            );
                            ui.label(
                                RichText::new(
                                    r.fetched_at
                                        .map(|t| t.format("%d %b %H:%M UTC").to_string())
                                        .unwrap_or_default(),
                                )
                                .size(11.0)
                                .color(Theme::text_muted()),
                            );
                        });
                    });
                    ui.separator();
                }
            });
    });
}

fn currency_combo(ui: &mut egui::Ui, id: &str, _label: &str, selected: &mut Currency) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(selected.code())
        .show_ui(ui, |ui| {
            for c in Currency::ALL {
                if ui.selectable_label(*selected == c, c.code()).clicked() {
                    *selected = c;
                }
            }
        });
}
