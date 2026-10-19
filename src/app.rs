use eframe::egui::{self, CornerRadius, RichText};
use std::time::Instant;

use crate::db::repository::Repository;
use crate::ui::company::{self, CompanyFormState};
use crate::ui::customers::{self, CustomerFormState};
use crate::ui::dashboard;
use crate::ui::delivery::{self, DeliveryFormState};
use crate::ui::emails::{self, EmailCenterState};
use crate::ui::fx::{self, FxState};
use crate::ui::invoices::{self, InvoiceFormState};
use crate::ui::ledger;
use crate::ui::settings::{self, SettingsState};
use crate::ui::sync_view::{self, SyncViewState};
use crate::ui::theme::{Theme, ThemeMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Invoices,
    Deliveries,
    Customers,
    GeneralLedger,
    FxRates,
    Reports,
    Automations,
    Emails,
    CompanyProfile,
    DeviceSync,
    Settings,
}

#[derive(Debug, Clone, Copy)]
struct NavItem {
    tab: Tab,
    title: &'static str,
    icon: &'static str,
}

const NAV_ITEMS: [NavItem; 11] = [
    NavItem {
        tab: Tab::Dashboard,
        title: "Dashboard",
        icon: "📊",
    },
    NavItem {
        tab: Tab::Invoices,
        title: "Invoices",
        icon: "📄",
    },
    NavItem {
        tab: Tab::Deliveries,
        title: "Deliveries",
        icon: "🚚",
    },
    NavItem {
        tab: Tab::Customers,
        title: "Customers",
        icon: "👥",
    },
    NavItem {
        tab: Tab::GeneralLedger,
        title: "General Ledger",
        icon: "📒",
    },
    NavItem {
        tab: Tab::FxRates,
        title: "FX Rates",
        icon: "💱",
    },
    NavItem {
        tab: Tab::Reports,
        title: "Reports",
        icon: "📈",
    },
    NavItem {
        tab: Tab::Automations,
        title: "Automations",
        icon: "⚙",
    },
    NavItem {
        tab: Tab::Emails,
        title: "Communications",
        icon: "✉",
    },
    NavItem {
        tab: Tab::CompanyProfile,
        title: "Company Profile",
        icon: "🏢",
    },
    NavItem {
        tab: Tab::DeviceSync,
        title: "Sync",
        icon: "🔄",
    },
];

pub struct SurfApp {
    repo: Repository,
    active_tab: Tab,
    customer_state: CustomerFormState,
    invoice_state: InvoiceFormState,
    delivery_state: DeliveryFormState,
    sync_state: SyncViewState,
    company_state: CompanyFormState,
    email_state: EmailCenterState,
    settings_state: SettingsState,
    fx_state: FxState,
    reports_state: crate::ui::reports::ReportsState,
    automations_state: crate::ui::automations::AutomationsState,
    automation_worker:
        Option<std::sync::mpsc::Receiver<crate::services::automations::AutomationMsg>>,
    automation_started: bool,
    notification: Option<(String, bool, Instant)>,
}

impl SurfApp {
    pub fn new(repo: Repository) -> Self {
        // Restore the remembered theme mode from the database (default: dark).
        let saved_mode = ThemeMode::from_u8(
            repo.get_setting("ui_theme")
                .and_then(|v| v.parse::<u8>().ok())
                .unwrap_or(0),
        );
        Theme::set_mode(saved_mode);
        Self {
            repo,
            active_tab: Tab::Dashboard,
            customer_state: CustomerFormState::default(),
            invoice_state: InvoiceFormState::default(),
            delivery_state: DeliveryFormState::default(),
            sync_state: SyncViewState::default(),
            company_state: CompanyFormState::default(),
            email_state: EmailCenterState::default(),
            settings_state: SettingsState::default(),
            fx_state: crate::ui::fx::FxState::new(),
            reports_state: crate::ui::reports::ReportsState::default(),
            automations_state: crate::ui::automations::AutomationsState::default(),
            automation_worker: None,
            automation_started: false,
            notification: None,
        }
    }
}

impl eframe::App for SurfApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render_sidebar(ui);
        self.render_main(ui);
    }
}

impl SurfApp {
    fn toggle_theme(&mut self, ctx: &egui::Context, mode: ThemeMode) {
        Theme::set_mode(mode);
        Theme::apply_visuals(ctx);
        let _ = self.repo.set_setting("ui_theme", &mode.as_u8().to_string());
    }

    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("surf_side_bar")
            .frame(
                egui::Frame::new()
                    .fill(Theme::bg_sidebar())
                    .stroke(egui::Stroke::new(1.0, Theme::divider()))
                    .inner_margin(egui::Margin::same(10)),
            )
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    // Brand: plain "Surf" wordmark, no logo icon.
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Surf")
                            .size(26.0)
                            .strong()
                            .color(Theme::text_strong()),
                    );
                    ui.label(
                        RichText::new("Accounting & Logistics")
                            .size(11.0)
                            .color(Theme::text_muted()),
                    );
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Navigation list
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for nav in &NAV_ITEMS {
                        let active = self.active_tab == nav.tab;
                        let label = RichText::new(format!("{}  {}", nav.icon, nav.title))
                            .size(13.5)
                            .color(if active {
                                Theme::on_primary()
                            } else {
                                Theme::text_muted()
                            });
                        let btn = ui.add(
                            egui::Button::new(label)
                                .fill(Theme::nav_fill(active))
                                .corner_radius(CornerRadius::same(6))
                                .min_size(egui::vec2(ui.available_width(), 26.0)),
                        );
                        if btn.clicked() {
                            self.active_tab = nav.tab;
                        }
                    }
                });

                // Bottom status block pinned to the sidebar bottom.
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Black & white (light) / dark mode toggle
                    ui.label(
                        RichText::new("Appearance")
                            .size(10.0)
                            .color(Theme::text_muted()),
                    );
                    let is_light = Theme::is_light();
                    ui.horizontal(|ui| {
                        if ui
                            .add(
                                egui::Button::new(RichText::new("Dark").size(11.5))
                                    .fill(if is_light {
                                        Theme::bg_card_light()
                                    } else {
                                        Theme::primary()
                                    })
                                    .small(),
                            )
                            .clicked()
                        {
                            self.toggle_theme(ui.ctx(), ThemeMode::Dark);
                        }
                        if ui
                            .add(
                                egui::Button::new(RichText::new("Light").size(11.5))
                                    .fill(if is_light {
                                        Theme::primary()
                                    } else {
                                        Theme::bg_card_light()
                                    })
                                    .small(),
                            )
                            .clicked()
                        {
                            self.toggle_theme(ui.ctx(), ThemeMode::Light);
                        }
                    });

                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("● Local-First Engine")
                            .size(11.0)
                            .color(Theme::success()),
                    );
                    ui.label(
                        RichText::new(format!(
                            "Device: {}",
                            &self.repo.device_id[..8.min(self.repo.device_id.len())]
                        ))
                        .size(10.5)
                        .color(Theme::text_muted()),
                    );
                    ui.add_space(2.0);
                });
            });
    }

    fn render_main(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Theme::bg_page())
                    .inner_margin(egui::Margin::same(14)),
            )
            .show(ui, |ui| {
                // Notification Banner
                let mut clear_notif = false;
                if let Some((msg, is_success, time)) = &self.notification {
                    if time.elapsed().as_secs() > 7 {
                        clear_notif = true;
                    } else {
                        let banner_bg = Theme::bg_card_light();
                        let border_color = if *is_success {
                            Theme::success()
                        } else {
                            Theme::danger()
                        };

                        let frame = egui::Frame::new()
                            .fill(banner_bg)
                            .stroke(egui::Stroke::new(1.0, border_color))
                            .corner_radius(CornerRadius::same(6))
                            .inner_margin(egui::Margin::symmetric(12, 8));

                        frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(if *is_success { "✓" } else { "⚠" })
                                        .color(border_color)
                                        .strong(),
                                );
                                ui.label(RichText::new(msg).color(Theme::text_strong()));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.small_button("✕").clicked() {
                                            clear_notif = true;
                                        }
                                    },
                                );
                            });
                        });
                        ui.add_space(8.0);
                    }
                }
                if clear_notif {
                    self.notification = None;
                }

                // Render Active View
                let mut notif_buffer: Option<(String, bool)> = None;
                let mut nav_to_tab: Option<&'static str> = None;

                // Automation engine: spawn one start-up cycle, then poll results.
                if !self.automation_started {
                    self.automation_started = true;
                    self.automation_worker =
                        Some(crate::services::automations::spawn_automations_worker(
                            self.repo.db_path().to_path_buf(),
                        ));
                } else if let Some(rx) = &self.automation_worker
                    && let Ok(crate::services::automations::AutomationMsg::Done(summary)) =
                        rx.try_recv()
                {
                    self.automation_worker = None;
                    let all_ok = !summary.contains('\u{26A0}');
                    notif_buffer = Some((summary, all_ok));
                }

                match self.active_tab {
                    Tab::Dashboard => {
                        dashboard::render(ui, &self.repo, &mut nav_to_tab);
                    }
                    Tab::Invoices => {
                        invoices::render(
                            ui,
                            &self.repo,
                            &mut self.invoice_state,
                            &mut notif_buffer,
                        );
                    }
                    Tab::Deliveries => {
                        delivery::render(
                            ui,
                            &self.repo,
                            &mut self.delivery_state,
                            &mut notif_buffer,
                        );
                    }
                    Tab::Customers => {
                        customers::render(
                            ui,
                            &self.repo,
                            &mut self.customer_state,
                            &mut notif_buffer,
                        );
                    }
                    Tab::GeneralLedger => {
                        ledger::render(ui, &self.repo);
                    }
                    Tab::FxRates => {
                        fx::render(ui, &self.repo, &mut self.fx_state, &mut notif_buffer);
                    }
                    Tab::Reports => {
                        crate::ui::reports::render(ui, &self.repo, &mut self.reports_state);
                    }
                    Tab::Automations => {
                        crate::ui::automations::render(
                            ui,
                            &self.repo,
                            &mut self.automations_state,
                            &mut notif_buffer,
                        );
                    }
                    Tab::Emails => {
                        emails::render(ui, &self.repo, &mut self.email_state, &mut notif_buffer);
                    }
                    Tab::CompanyProfile => {
                        company::render(ui, &self.repo, &mut self.company_state, &mut notif_buffer);
                    }
                    Tab::DeviceSync => {
                        sync_view::render(ui, &self.repo, &mut self.sync_state, &mut notif_buffer);
                    }
                    Tab::Settings => {
                        settings::render(
                            ui,
                            &self.repo,
                            &mut self.settings_state,
                            &mut notif_buffer,
                        );
                    }
                }

                // Handle Tab Navigation request from Dashboard or Views
                if let Some(tab_name) = nav_to_tab {
                    self.active_tab = match tab_name {
                        "Invoices" => Tab::Invoices,
                        "Deliveries" => Tab::Deliveries,
                        "Customers" => Tab::Customers,
                        "GeneralLedger" => Tab::GeneralLedger,
                        "FxRates" => Tab::FxRates,
                        "Reports" => Tab::Reports,
                        "Automations" => Tab::Automations,
                        "Emails" => Tab::Emails,
                        "CompanyProfile" => Tab::CompanyProfile,
                        "DeviceSync" => Tab::DeviceSync,
                        "Settings" => Tab::Settings,
                        _ => Tab::Dashboard,
                    };
                }

                // Handle Notification Trigger
                if let Some((msg, is_success)) = notif_buffer {
                    self.notification = Some((msg, is_success, Instant::now()));
                }
            });
    }
}
