use eframe::egui::{self, RichText};

use super::theme::{Theme, ThemeMode};
use crate::core::templates::MessageLang;
use crate::db::repository::Repository;

pub struct SettingsState {
    pub telegram_token: String,
    pub telegram_chat_id: String,
    pub saved_flash: bool,
    pub token_obsured: bool,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            telegram_token: String::new(),
            telegram_chat_id: String::new(),
            saved_flash: false,
            token_obsured: true,
        }
    }
}

pub fn render(
    ui: &mut egui::Ui,
    repo: &Repository,
    state: &mut SettingsState,
    notification: &mut Option<(String, bool)>,
) {
    ui.add_space(8.0);

    // --- Appearance ---
    Theme::card_frame().show(ui, |ui| {
        ui.heading("Appearance");
        ui.separator();
        ui.label(
            RichText::new(
                "Switch between the black & white (light) and dark look. The choice is remembered.",
            )
            .size(11.0)
            .color(Theme::text_muted()),
        );
        ui.add_space(6.0);
        let current = Theme::mode();
        for mode in [ThemeMode::Dark, ThemeMode::Light] {
            let is_active = current == mode;
            if ui
                .add(
                    egui::Button::new(RichText::new(mode.label()).color(Theme::text_strong()))
                        .fill(if is_active {
                            Theme::primary()
                        } else {
                            Theme::bg_card_light()
                        })
                        .min_size(egui::vec2(ui.available_width(), 26.0)),
                )
                .clicked()
                && !is_active
            {
                Theme::set_mode(mode);
                Theme::apply_visuals(ui.ctx());
                let _ = repo.set_setting("ui_theme", &mode.as_u8().to_string());
            }
        }
    });

    ui.add_space(12.0);

    // --- Telegram bot (real file delivery) ---
    Theme::card_frame().show(ui, |ui| {
        ui.heading("Telegram Bot (invoice & document delivery)");
        ui.separator();
        ui.label(
            RichText::new(format!(
                "1. In Telegram, open @BotFather -> /newbot -> copy the token. \
                 2. Message your bot once (any text). 3. Put your numeric chat id below \
                 (or send a document once to @userinfobot to learn your chat id.). Token/chat id are \
                 stored only in your local {} database.",
                repo.db_path().display()
            ))
            .size(11.0)
            .color(Theme::text_muted()),
        );
        ui.add_space(8.0);

        if state.telegram_token.is_empty() {
            state.telegram_token = repo.get_setting("telegram_bot_token").unwrap_or_default();
        }
        if state.telegram_chat_id.is_empty() {
            state.telegram_chat_id = repo.get_setting("telegram_chat_id").unwrap_or_default();
        }

        egui::Grid::new("telegram_settings_grid")
            .num_columns(2)
            .spacing([12.0, 10.0])
            .show(ui, |ui| {
                ui.label("Bot token:");
                let token_field = if state.token_obsured {
                    RichText::new(mask_secret(&state.telegram_token))
                } else {
                    RichText::new(state.telegram_token.clone())
                };
                // Allow editing; when masked, first click reveals.
                if state.token_obsured {
                    if ui
                        .button(token_field.size(12.0).color(Theme::text_muted()))
                        .clicked()
                    {
                        state.token_obsured = false;
                    }
                } else {
                    ui.text_edit_singleline(&mut state.telegram_token);
                }
                ui.end_row();

                ui.label("Chat id:");
                ui.text_edit_singleline(&mut state.telegram_chat_id);
                ui.end_row();
            });

        ui.add_space(6.0);
        if ui.button("Save Telegram settings").clicked() {
            let res = repo
                .set_setting("telegram_bot_token", state.telegram_token.trim())
                .and_then(|_| {
                    repo.set_setting("telegram_chat_id", state.telegram_chat_id.trim())
                });
            match res {
                Ok(()) => {
                    *notification =
                        Some(("Telegram settings saved.".to_string(), true));
                }
                Err(e) => {
                    *notification =
                        Some((format!("Could not save settings: {}", e), false));
                }
            }
        }
    });

    ui.add_space(12.0);

    // --- Default share language ---
    Theme::card_frame().show(ui, |ui| {
        ui.heading("Default share language");
        ui.separator();
        ui.label(
            RichText::new("Language used for the ready-to-send text generated by the WhatsApp / Telegram buttons under invoices.")
                .size(11.0)
                .color(Theme::text_muted()),
        );
        ui.add_space(6.0);
        let current_code = repo.get_setting("share_language").unwrap_or_else(|| "en".into());
        let current_lang = MessageLang::from_code(&current_code);
        egui::ComboBox::from_id_salt("settings_share_lang")
            .selected_text(current_lang.name())
            .width(160.0)
            .show_ui(ui, |ui| {
                for l in MessageLang::ALL {
                    let is_sel = current_lang == l;
                    if ui.selectable_label(is_sel, l.name()).clicked() {
                        let _ = repo.set_setting("share_language", l.code());
                    }
                }
            });
    });

    ui.add_space(12.0);

    // --- Storage & exports ---
    Theme::card_frame().show(ui, |ui| {
        ui.heading("Storage");
        ui.separator();
        egui::Grid::new("storage_settings_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(RichText::new("Database file:").strong());
                ui.label(
                    RichText::new(repo.db_path().display().to_string())
                        .color(Theme::text_muted())
                        .size(12.0),
                );
                ui.end_row();

                ui.label(RichText::new("Exports folder:").strong());
                ui.label(
                    RichText::new(crate::services::share::exports_dir().display().to_string())
                        .color(Theme::text_muted())
                        .size(12.0),
                );
                ui.end_row();

                ui.label(RichText::new("Device id:").strong());
                ui.label(RichText::new(&repo.device_id).color(Theme::text_muted()));
                ui.end_row();
            });
    });
}

fn mask_secret(token: &str) -> String {
    let n = token.chars().count();
    if n < 6 {
        return token.to_string();
    }
    let prefix: String = token.chars().take(5).collect();
    let suffix: String = token.chars().skip(n - 3).collect();
    format!("{}...{}", prefix, suffix)
}
