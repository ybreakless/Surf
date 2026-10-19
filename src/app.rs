use eframe::egui::{self, Color32, CornerRadius, RichText};
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
use crate::ui::sync_view::{self, SyncViewState};
use crate::ui::theme::Theme;

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
}

pub struct SurfApp {
    repo: Repository,
    active_tab: Tab,
    customer_state: CustomerFormState,
    invoice_state: InvoiceFormState,
    delivery_state: DeliveryFormState,
    sync_state: SyncViewState,
    company_state: CompanyFormState,
    email_state: EmailCenterState,
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
        Self {
            repo,
            active_tab: Tab::Dashboard,
            customer_state: CustomerFormState::default(),
            invoice_state: InvoiceFormState::default(),
            delivery_state: DeliveryFormState::default(),
            sync_state: SyncViewState::default(),
            company_state: CompanyFormState::default(),
            email_state: EmailCenterState::default(),
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
        // Top Navigation Bar
        egui::Panel::top("surf_top_bar")
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(18, 20, 26))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgb(38, 42, 54)))
                    .inner_margin(egui::Margin::symmetric(16, 10)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Logo
                    ui.heading(
                        RichText::new("🏄 SURF")
                            .color(Theme::PRIMARY)
                            .strong()
                            .size(19.0),
                    );
                    ui.label(
                        RichText::new("ACCOUNTING")
                            .color(Color32::WHITE)
                            .strong()
                            .size(13.0),
                    );

                    ui.add_space(20.0);

                    // Navigation Tabs
                    let tabs = [
                        (Tab::Dashboard, "📊 Dashboard"),
                        (Tab::Invoices, "📄 Invoices"),
                        (Tab::Deliveries, "🚚 Deliveries"),
                        (Tab::Customers, "👥 Customers"),
                        (Tab::GeneralLedger, "📒 General Ledger"),
                        (Tab::FxRates, "💱 FX Rates"),
                        (Tab::Reports, "📈 Reports"),
                        (Tab::Automations, "⚙ Automations"),
                        (Tab::Emails, "✉ Communications"),
                        (Tab::CompanyProfile, "🏢 Company Profile"),
                        (Tab::DeviceSync, "🔄 Sync"),
                    ];

                    for (tab, title) in tabs {
                        let is_active = self.active_tab == tab;
                        let text_color = if is_active {
                            Color32::WHITE
                        } else {
                            Theme::TEXT_MUTED
                        };

                        let btn = ui.add(
                            egui::Button::new(
                                RichText::new(title).color(text_color).strong().size(12.5),
                            )
                            .fill(if is_active {
                                Theme::PRIMARY
                            } else {
                                Color32::TRANSPARENT
                            })
                            .corner_radius(CornerRadius::same(5)),
                        );

                        if btn.clicked() {
                            self.active_tab = tab;
                        }
                    }

                    // Right side system status
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("Device: {}", self.repo.device_id))
                                .size(11.0)
                                .color(Theme::TEXT_MUTED),
                        );
                        ui.label(
                            RichText::new("● Local-First Engine")
                                .color(Theme::SUCCESS)
                                .size(11.0),
                        );
                    });
                });
            });

        // Main Content Area
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(22, 24, 30))
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ui, |ui| {
                // Notification Banner
                let mut clear_notif = false;
                if let Some((msg, is_success, time)) = &self.notification {
                    if time.elapsed().as_secs() > 7 {
                        clear_notif = true;
                    } else {
                        let banner_bg = if *is_success {
                            Color32::from_rgb(20, 45, 30)
                        } else {
                            Color32::from_rgb(50, 20, 20)
                        };
                        let border_color = if *is_success {
                            Theme::SUCCESS
                        } else {
                            Theme::DANGER
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
                                ui.label(RichText::new(msg).color(Color32::WHITE));
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
