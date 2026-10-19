use eframe::egui::{self, Color32, RichText};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use super::theme::Theme;
use crate::db::repository::Repository;
use crate::services::automations::{self, AutomationMsg};

#[derive(Default)]
pub struct AutomationsState {
    pub worker: Option<Receiver<AutomationMsg>>,
    pub last_summary: Option<String>,
}

pub fn render(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut AutomationsState,
    notification: &mut Option<(String, bool)>,
) {
    ui.add_space(8.0);

    // Poll background run
    if let Some(rx) = &state.worker {
        let mut done = None;
        if let Ok(msg) = rx.try_recv() {
            match msg {
                AutomationMsg::Done(summary) => {
                    done = Some(summary);
                }
            }
        }
        if let Some(summary) = done {
            state.worker = None;
            state.last_summary = Some(summary.clone());
            let all_ok = !summary.contains('\u{26A0}');
            *notification = Some((summary.clone(), all_ok));
        } else {
            ui.label(
                RichText::new("Running automation cycle in background...").color(Theme::PRIMARY),
            );
            ui.ctx().request_repaint_after(Duration::from_millis(400));
        }
    }

    Theme::card_frame().show(ui, |ui| {
        ui.heading("Automatic Tasks");
        ui.label(
            RichText::new(
                "Daily engine that runs at app start (and on demand): fetches missing IRR rates from tgju.org, \
                 creates the day's database snapshot, bills due recurring templates, and scans for overdue invoices.",
            )
            .size(11.0)
            .color(Theme::TEXT_MUTED),
        );
        ui.separator();

        ui.horizontal(|ui| {
            let running = state.worker.is_some();
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("Run All Now").color(Color32::WHITE).strong(),
                    ),
                )
                .clicked()
                && !running
            {
                state.worker =
                    Some(automations::spawn_automations_worker(repo.db_path().to_path_buf()));
            }
            if let Some(s) = &state.last_summary {
                ui.label(RichText::new(s).color(Theme::TEXT_MUTED).size(11.0));
            }
        });
    });

    ui.add_space(16.0);

    Theme::card_frame().show(ui, |ui| {
        ui.heading(RichText::new("Automation Log").size(16.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let count = repo.get_automation_logs(999).unwrap_or_default().len();
            ui.label(
                RichText::new(format!("{} runs recorded", count))
                    .size(11.0)
                    .color(Theme::TEXT_MUTED),
            );
        });
        ui.separator();

        let logs = repo.get_automation_logs(40).unwrap_or_default();
        if logs.is_empty() {
            ui.label(
                RichText::new("No runs recorded yet \u{2014} press Run All Now")
                    .color(Theme::TEXT_MUTED),
            );
        } else {
            egui::ScrollArea::vertical()
                .max_height(380.0)
                .show(ui, |ui| {
                    for (task, ok, result, ran_at) in logs {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(if ok { "\u{2714}" } else { "\u{26A0}" })
                                    .color(if ok { Theme::SUCCESS } else { Theme::DANGER })
                                    .strong(),
                            );
                            ui.label(RichText::new(task).strong().size(12.0));
                            ui.label(RichText::new(result).color(Color32::WHITE).size(12.0));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(fmt_time(&ran_at))
                                            .size(11.0)
                                            .color(Theme::TEXT_MUTED),
                                    );
                                },
                            );
                        });
                        ui.separator();
                    }
                });
        }
    });
}

fn fmt_time(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|d| d.format("%d %b %H:%M").to_string())
        .unwrap_or_else(|_| rfc3339.to_string())
}
