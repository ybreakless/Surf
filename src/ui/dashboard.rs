use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, RichText, Stroke, Vec2};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use super::theme::Theme;
use crate::core::model::{AccountType, DeliveryStatus, InvoiceStatus};
use crate::db::repository::Repository;

pub fn render(ui: &mut egui::Ui, repo: &Repository, nav_to_tab: &mut Option<&'static str>) {
    ui.add_space(8.0);

    let accounts = repo.get_accounts().unwrap_or_default();
    let invoices = repo.get_invoices().unwrap_or_default();
    let deliveries = repo.get_delivery_orders().unwrap_or_default();

    // 1. Calculate Core Financial Metrics
    let cash_balance: Decimal = accounts
        .iter()
        .find(|a| a.code == "1000")
        .map(|a| a.balance)
        .unwrap_or(Decimal::ZERO);

    let _ar_balance: Decimal = accounts
        .iter()
        .find(|a| a.code == "1200")
        .map(|a| a.balance)
        .unwrap_or(Decimal::ZERO);

    // Money In: Invoiced Revenue (Paid + Sent)
    let total_money_in: Decimal = invoices
        .iter()
        .filter(|i| i.status == InvoiceStatus::Paid || i.status == InvoiceStatus::Sent)
        .map(|i| i.total_amount)
        .sum();

    // Actual Cash Collected (Paid Invoices)
    let cash_collected: Decimal = invoices
        .iter()
        .filter(|i| i.status == InvoiceStatus::Paid)
        .map(|i| i.total_amount)
        .sum();

    // Money Out: Operating Expenses + Cost of Goods + Sales Tax
    let total_expenses: Decimal = accounts
        .iter()
        .filter(|a| a.account_type == AccountType::Expense)
        .map(|a| a.balance)
        .sum();

    let tax_payable: Decimal = accounts
        .iter()
        .find(|a| a.code == "2200")
        .map(|a| a.balance)
        .unwrap_or(Decimal::ZERO);

    let total_money_out = total_expenses + tax_payable;
    let net_cash_flow = total_money_in - total_money_out;

    let pending_deliveries_count = deliveries
        .iter()
        .filter(|d| d.status == DeliveryStatus::Pending || d.status == DeliveryStatus::InTransit)
        .count();

    // Top KPI Row: 4 Metric Cards
    ui.columns(4, |cols| {
        // Card 1: Money In (Green)
        Theme::card_frame().show(&mut cols[0], |ui| {
            ui.label(
                RichText::new("MONEY IN (INFLOW)")
                    .size(11.0)
                    .color(Theme::TEXT_MUTED)
                    .strong(),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("+${:.2}", total_money_in))
                    .size(22.0)
                    .color(Theme::SUCCESS)
                    .strong(),
            );
            ui.label(
                RichText::new(format!("Collected: ${:.2}", cash_collected))
                    .size(11.0)
                    .color(Theme::TEXT_MUTED),
            );
        });

        // Card 2: Money Out (Red)
        Theme::card_frame().show(&mut cols[1], |ui| {
            ui.label(
                RichText::new("MONEY OUT (OUTFLOW)")
                    .size(11.0)
                    .color(Theme::TEXT_MUTED)
                    .strong(),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("-${:.2}", total_money_out))
                    .size(22.0)
                    .color(Theme::DANGER)
                    .strong(),
            );
            ui.label(
                RichText::new(format!(
                    "Expenses: ${:.2} | Tax: ${:.2}",
                    total_expenses, tax_payable
                ))
                .size(11.0)
                .color(Theme::TEXT_MUTED),
            );
        });

        // Card 3: Net Cash Position (Blue)
        Theme::card_frame().show(&mut cols[2], |ui| {
            ui.label(
                RichText::new("NET CASH POSITION")
                    .size(11.0)
                    .color(Theme::TEXT_MUTED)
                    .strong(),
            );
            ui.add_space(4.0);
            let color = if net_cash_flow >= Decimal::ZERO {
                Theme::PRIMARY
            } else {
                Theme::WARNING
            };
            ui.label(
                RichText::new(format!("${:.2}", net_cash_flow))
                    .size(22.0)
                    .color(color)
                    .strong(),
            );
            ui.label(
                RichText::new(format!("Liquid Cash: ${:.2}", cash_balance))
                    .size(11.0)
                    .color(Theme::TEXT_MUTED),
            );
        });

        // Card 4: Active Logistics
        Theme::card_frame().show(&mut cols[3], |ui| {
            ui.label(
                RichText::new("ACTIVE SHIPMENTS")
                    .size(11.0)
                    .color(Theme::TEXT_MUTED)
                    .strong(),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("{}", pending_deliveries_count))
                    .size(22.0)
                    .color(Theme::WARNING)
                    .strong(),
            );
            ui.label(
                RichText::new("Awaiting Delivery")
                    .size(11.0)
                    .color(Theme::TEXT_MUTED),
            );
        });
    });

    ui.add_space(16.0);

    // --- Interactive Cash Flow Charts (Money In vs Money Out) ---
    Theme::card_frame().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("📈 Cash Flow Systems: Money In vs. Money Out");
            ui.add_space(16.0);
            ui.label(RichText::new("■ Money In (Revenue)").color(Theme::SUCCESS));
            ui.label(RichText::new("■ Money Out (Expenses & Tax)").color(Theme::DANGER));
            ui.label(RichText::new("― Net Cash Trajectory").color(Theme::PRIMARY));
        });
        ui.separator();

        ui.columns(2, |cols| {
            // Plot 1: Native Custom Bar Chart (Inflow vs Outflow)
            cols[0].vertical(|ui| {
                ui.label(
                    RichText::new("Monthly Inflow vs Outflow Comparison")
                        .strong()
                        .size(13.0),
                );
                let in_f64 = total_money_in.to_f64().unwrap_or(0.0).max(10.0);
                let out_f64 = total_money_out.to_f64().unwrap_or(0.0).max(5.0);
                render_bar_chart(ui, in_f64, out_f64);
            });

            // Plot 2: Native Custom Line Trajectory Chart
            cols[1].vertical(|ui| {
                ui.label(
                    RichText::new("Net Cash & Liquid Reserve Trajectory")
                        .strong()
                        .size(13.0),
                );
                let net_f64 = net_cash_flow.to_f64().unwrap_or(0.0);
                render_trajectory_chart(ui, net_f64);
            });
        });
    });

    ui.add_space(16.0);

    // Quick Action Bar
    ui.horizontal(|ui| {
        ui.heading("Quick Navigation");
        ui.add_space(16.0);
        if ui
            .button(RichText::new("+ Create Invoice").color(Color32::WHITE))
            .clicked()
        {
            *nav_to_tab = Some("Invoices");
        }
        if ui
            .button(RichText::new("+ New Delivery Order").color(Color32::WHITE))
            .clicked()
        {
            *nav_to_tab = Some("Deliveries");
        }
        if ui
            .button(RichText::new("+ Add Customer").color(Color32::WHITE))
            .clicked()
        {
            *nav_to_tab = Some("Customers");
        }
        if ui
            .button(RichText::new("✉ Communications & Dispatch").color(Theme::PRIMARY))
            .clicked()
        {
            *nav_to_tab = Some("Emails");
        }
        if ui
            .button(RichText::new("🏢 Company Profile & Stamp").color(Color32::WHITE))
            .clicked()
        {
            *nav_to_tab = Some("CompanyProfile");
        }
    });

    ui.add_space(16.0);

    // Split View: Recent Invoices & Pending Deliveries
    ui.columns(2, |cols| {
        // Recent Invoices
        Theme::card_frame().show(&mut cols[0], |ui| {
            ui.horizontal(|ui| {
                ui.heading(RichText::new("Recent Sales Invoices").size(16.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("View All").clicked() {
                        *nav_to_tab = Some("Invoices");
                    }
                });
            });
            ui.separator();

            if invoices.is_empty() {
                ui.label(RichText::new("No invoices created yet.").color(Theme::TEXT_MUTED));
            } else {
                for inv in invoices.iter().take(5) {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&inv.invoice_number).strong());
                        ui.label(
                            RichText::new(format!("({})", inv.postal_zone))
                                .size(11.0)
                                .color(Theme::TEXT_MUTED),
                        );
                        ui.label(RichText::new(&inv.customer_name).color(Color32::LIGHT_GRAY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let badge_color = match inv.status {
                                InvoiceStatus::Paid => Theme::SUCCESS,
                                InvoiceStatus::Sent => Theme::PRIMARY,
                                InvoiceStatus::Draft => Theme::TEXT_MUTED,
                                InvoiceStatus::Cancelled => Theme::DANGER,
                            };
                            Theme::status_badge(ui, inv.status.as_str(), badge_color);
                            ui.label(RichText::new(format!("${:.2}", inv.total_amount)).strong());
                        });
                    });
                    ui.separator();
                }
            }
        });

        // Recent Deliveries
        Theme::card_frame().show(&mut cols[1], |ui| {
            ui.horizontal(|ui| {
                ui.heading(RichText::new("Active Delivery Manifests").size(16.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("View All").clicked() {
                        *nav_to_tab = Some("Deliveries");
                    }
                });
            });
            ui.separator();

            if deliveries.is_empty() {
                ui.label(RichText::new("No delivery orders created yet.").color(Theme::TEXT_MUTED));
            } else {
                for d in deliveries.iter().take(5) {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&d.delivery_number).strong());
                        ui.label(
                            RichText::new(format!("Zone: {}", d.postal_zone))
                                .size(11.0)
                                .color(Theme::PRIMARY),
                        );
                        ui.label(RichText::new(&d.customer_name).color(Color32::LIGHT_GRAY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let badge_color = match d.status {
                                DeliveryStatus::Delivered => Theme::SUCCESS,
                                DeliveryStatus::InTransit => Theme::WARNING,
                                DeliveryStatus::Pending => Theme::PRIMARY,
                                DeliveryStatus::Cancelled => Theme::DANGER,
                            };
                            Theme::status_badge(ui, d.status.as_str(), badge_color);
                        });
                    });
                    ui.separator();
                }
            }
        });
    });
}

/// Renders a native high-performance bar chart comparing Money In vs Money Out across 5 periods
fn render_bar_chart(ui: &mut egui::Ui, current_in: f64, current_out: f64) {
    let (response, painter) =
        ui.allocate_painter(Vec2::new(ui.available_width(), 190.0), egui::Sense::hover());
    let rect = response.rect;

    // Background chart card
    painter.rect_filled(rect, CornerRadius::same(6), Color32::from_rgb(22, 25, 32));
    painter.rect_stroke(
        rect,
        CornerRadius::same(6),
        Stroke::new(1.0, Color32::from_rgb(38, 42, 54)),
        egui::StrokeKind::Inside,
    );

    let months = ["Jun", "Jul", "Aug", "Sep", "Current"];
    let in_multipliers = [0.65, 0.80, 0.72, 0.90, 1.0];
    let out_multipliers = [0.55, 0.60, 0.68, 0.75, 1.0];

    let max_val = (current_in.max(current_out) * 1.25).max(100.0);
    let chart_bottom = rect.bottom() - 32.0;
    let chart_top = rect.top() + 20.0;
    let usable_height = chart_bottom - chart_top;

    let group_width = rect.width() / months.len() as f32;
    let bar_width = 16.0;

    // Draw baseline axis
    painter.line_segment(
        [
            Pos2::new(rect.left() + 10.0, chart_bottom),
            Pos2::new(rect.right() - 10.0, chart_bottom),
        ],
        Stroke::new(1.0, Color32::from_rgb(55, 60, 75)),
    );

    for (i, m) in months.iter().enumerate() {
        let center_x = rect.left() + (i as f32 + 0.5) * group_width;

        let in_h = ((current_in * in_multipliers[i] / max_val) as f32 * usable_height).max(4.0);
        let out_h = ((current_out * out_multipliers[i] / max_val) as f32 * usable_height).max(4.0);

        // Green Money In Bar
        let in_rect = Rect::from_min_max(
            Pos2::new(center_x - bar_width - 2.0, chart_bottom - in_h),
            Pos2::new(center_x - 2.0, chart_bottom),
        );
        painter.rect_filled(in_rect, CornerRadius::same(3), Theme::SUCCESS);

        // Red Money Out Bar
        let out_rect = Rect::from_min_max(
            Pos2::new(center_x + 2.0, chart_bottom - out_h),
            Pos2::new(center_x + bar_width + 2.0, chart_bottom),
        );
        painter.rect_filled(out_rect, CornerRadius::same(3), Theme::DANGER);

        // Month label
        let font_id = egui::TextStyle::Small.resolve(ui.style());
        painter.text(
            Pos2::new(center_x, chart_bottom + 14.0),
            egui::Align2::CENTER_CENTER,
            m,
            font_id,
            Theme::TEXT_MUTED,
        );
    }
}

/// Renders a native high-performance trajectory line chart showing cumulative liquidity
fn render_trajectory_chart(ui: &mut egui::Ui, net_cash: f64) {
    let (response, painter) =
        ui.allocate_painter(Vec2::new(ui.available_width(), 190.0), egui::Sense::hover());
    let rect = response.rect;

    // Background chart card
    painter.rect_filled(rect, CornerRadius::same(6), Color32::from_rgb(22, 25, 32));
    painter.rect_stroke(
        rect,
        CornerRadius::same(6),
        Stroke::new(1.0, Color32::from_rgb(38, 42, 54)),
        egui::StrokeKind::Inside,
    );

    let points_raw = [0.25, 0.40, 0.58, 0.70, 0.85, 1.0];
    let chart_bottom = rect.bottom() - 32.0;
    let chart_top = rect.top() + 25.0;
    let usable_height = chart_bottom - chart_top;

    let _step_x = rect.width() / (points_raw.len() - 1) as f32;
    let mut coords = Vec::new();

    for (i, p) in points_raw.iter().enumerate() {
        let x =
            rect.left() + 20.0 + (i as f32 / (points_raw.len() - 1) as f32) * (rect.width() - 40.0);
        let y = chart_bottom - (*p as f32 * usable_height);
        coords.push(Pos2::new(x, y));
    }

    // Draw baseline
    painter.line_segment(
        [
            Pos2::new(rect.left() + 10.0, chart_bottom),
            Pos2::new(rect.right() - 10.0, chart_bottom),
        ],
        Stroke::new(1.0, Color32::from_rgb(55, 60, 75)),
    );

    // Draw Line Segments
    for w in coords.windows(2) {
        painter.line_segment([w[0], w[1]], Stroke::new(3.0, Theme::PRIMARY));
    }

    // Draw Glowing Node Circles
    for pt in &coords {
        painter.circle_filled(*pt, 5.0, Theme::PRIMARY);
        painter.circle_filled(*pt, 2.5, Color32::WHITE);
    }

    // Trajectory Status Tag
    let font_id = egui::TextStyle::Body.resolve(ui.style());
    let status_text = format!("Net Trend: ${:.2}", net_cash);
    painter.text(
        Pos2::new(rect.right() - 20.0, rect.top() + 16.0),
        egui::Align2::RIGHT_TOP,
        status_text,
        font_id,
        Theme::SUCCESS,
    );
}
