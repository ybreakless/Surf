use eframe::egui::{self, ViewportBuilder};
use std::path::PathBuf;

use Surf::app::SurfApp;
use Surf::db::repository::Repository;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // CLI maintenance subcommands (usable from cron for daily FX updates).
    match args.get(1).map(|s| s.as_str()) {
        Some("rates-update") => {
            let db_path = args
                .get(2)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("surf_data.db"));
            if !db_path.exists() {
                eprintln!("Initializing new Surf database at {}", db_path.display());
            }
            let repo =
                Repository::new(&db_path).expect("Failed to initialize Surf accounting database");
            match Surf::services::fx_provider::update_rates_blocking(&repo) {
                Ok((saved, failed)) => {
                    println!(
                        "FX update complete: {} saved, {} failed.",
                        saved,
                        failed.len()
                    );
                    for f in failed {
                        eprintln!("  WARN {f}");
                    }
                    if saved == 0 {
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("FX update failed: {e}");
                    std::process::exit(1);
                }
            }
            return Ok(());
        }
        Some("help") | Some("--help") | Some("-h") => {
            println!("Surf - Professional Accounting & Logistics");
            println!();
            println!("Usage:");
            println!("  Surf                 launch the accounting desktop app");
            println!(
                "  Surf rates-update [db]   fetch today's IRR rates from tgju.org and store them"
            );
            return Ok(());
        }
        _ => {}
    }

    // Persistent SQLite database file
    let db_path = PathBuf::from("surf_data.db");
    let repo = Repository::new(&db_path).expect("Failed to initialize Surf accounting database");

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Surf - Professional Accounting & Logistics")
            .with_inner_size([1280.0, 780.0])
            .with_min_inner_size([980.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Surf Accounting",
        options,
        Box::new(|cc| {
            // Apply sleek dark theme visuals
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(SurfApp::new(repo)))
        }),
    )
}
