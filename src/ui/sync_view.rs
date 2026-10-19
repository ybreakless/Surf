use eframe::egui::{self, RichText};
use std::fs;

use super::theme::Theme;
use crate::core::sync::SyncBundle;
use crate::db::repository::Repository;

pub struct SyncViewState {
    pub import_json_buffer: String,
    pub export_file_path: String,
    pub status_message: Option<String>,
}

impl Default for SyncViewState {
    fn default() -> Self {
        Self {
            import_json_buffer: String::new(),
            export_file_path: "surf_sync_package.json".to_string(),
            status_message: None,
        }
    }
}

pub fn render(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut SyncViewState,
    notification: &mut Option<(String, bool)>,
) {
    ui.add_space(8.0);

    let changes = repo.get_all_sync_changes().unwrap_or_default();

    // Top Device Identity Card
    Theme::card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("DEVICE IDENTITY")
                        .size(11.0)
                        .color(Theme::text_muted())
                        .strong(),
                );
                ui.add_space(2.0);
                ui.heading(RichText::new(&repo.device_id).color(Theme::primary()));
            });

            ui.add_space(32.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("RECORDED CHANGES")
                        .size(11.0)
                        .color(Theme::text_muted())
                        .strong(),
                );
                ui.add_space(2.0);
                ui.label(
                    RichText::new(format!("{} Events", changes.len()))
                        .size(18.0)
                        .strong(),
                );
            });

            ui.add_space(32.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("PLATFORM TARGET")
                        .size(11.0)
                        .color(Theme::text_muted())
                        .strong(),
                );
                ui.add_space(2.0);
                ui.label(
                    RichText::new("Linux (Host) • Ready for Win & Android")
                        .size(14.0)
                        .color(Theme::success()),
                );
            });
        });
    });

    ui.add_space(16.0);

    ui.columns(2, |cols| {
        // --- Left: Export Synchronization Package ---
        Theme::card_frame().show(&mut cols[0], |ui| {
            ui.heading("Export Sync Package");
            ui.separator();
            ui.label(
                "Export your business transactions, customers, and invoices to sync with your Windows PC or Android phone.",
            );
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Export File Path:");
                ui.text_edit_singleline(&mut state.export_file_path);
            });

            ui.add_space(12.0);
            if ui.button(RichText::new("Export Sync File (.json)").color(Theme::text_strong()).strong()).clicked() {
                match repo.export_sync_bundle() {
                    Ok(bundle) => match bundle.to_json() {
                        Ok(json) => match fs::write(&state.export_file_path, json) {
                            Ok(_) => {
                                *notification = Some((
                                    format!("Sync package exported successfully to '{}'!", state.export_file_path),
                                    true,
                                ));
                            }
                            Err(e) => {
                                state.status_message = Some(format!("Failed to write file: {}", e));
                            }
                        },
                        Err(e) => {
                            state.status_message = Some(format!("JSON serialization error: {}", e));
                        }
                    },
                    Err(e) => {
                        state.status_message = Some(format!("Database export error: {}", e));
                    }
                }
            }

            ui.add_space(12.0);
            ui.label(RichText::new("Cross-Device Workflow:").strong().size(12.0));
            ui.label(
                RichText::new(
                    "1. Export sync package here.\n2. Transfer via local WiFi, Syncthing, or USB to your other machine or phone.\n3. Import on target device to merge changes automatically.",
                )
                .size(11.0)
                .color(Theme::text_muted()),
            );
        });

        // --- Right: Import & Merge Synchronization Package ---
        Theme::card_frame().show(&mut cols[1], |ui| {
            ui.heading("Import & Merge Package");
            ui.separator();
            ui.label("Paste the sync package JSON below or load it to merge remote updates.");
            ui.add_space(8.0);

            ui.add(
                egui::TextEdit::multiline(&mut state.import_json_buffer)
                    .hint_text("Paste sync package JSON content here...")
                    .desired_rows(8)
                    .desired_width(f32::INFINITY),
            );

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Apply & Merge Package").color(Theme::text_strong()).strong()).clicked() {
                    match SyncBundle::from_json(&state.import_json_buffer) {
                        Ok(bundle) => match repo.apply_sync_bundle(&bundle) {
                            Ok(count) => {
                                *notification = Some((
                                    format!("Successfully merged {} changes from device '{}'!", count, bundle.source_device_id),
                                    true,
                                ));
                                state.import_json_buffer.clear();
                            }
                            Err(e) => {
                                state.status_message = Some(format!("Failed to apply sync changes: {}", e));
                            }
                        },
                        Err(e) => {
                            state.status_message = Some(format!("Invalid sync package format: {}", e));
                        }
                    }
                }

                if ui.button("Clear").clicked() {
                    state.import_json_buffer.clear();
                    state.status_message = None;
                }
            });

            if let Some(msg) = &state.status_message {
                ui.add_space(8.0);
                ui.label(RichText::new(msg).color(Theme::danger()));
            }
        });
    });
}
