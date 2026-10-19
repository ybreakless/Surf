use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use uuid::Uuid;

use Surf::core::accounting::AccountingEngine;
use Surf::core::model::{Invoice, InvoiceItem, InvoiceStatus};
use Surf::core::numbering::NumberingEngine;
use Surf::core::sync::SyncBundle;
use Surf::db::repository::Repository;
use Surf::services::fx_provider;

#[test]
fn test_intelligent_numbering_engine() {
    // 1. Customer code tests
    let code1 = NumberingEngine::derive_customer_code("Aref General Trading");
    assert_eq!(code1, "AGT");

    let code2 = NumberingEngine::derive_customer_code("Oceanic");
    assert_eq!(code2, "OCE");

    // 2. Postal zone routing tests
    let zone_us = NumberingEngine::derive_postal_zone("10001", "New York");
    assert_eq!(zone_us, "100");

    let zone_sf = NumberingEngine::derive_postal_zone("94103-1234", "San Francisco");
    assert_eq!(zone_sf, "941");

    let zone_uk = NumberingEngine::derive_postal_zone("EC1A 1BB", "London");
    assert_eq!(zone_uk, "EC1A");

    let zone_fallback = NumberingEngine::derive_postal_zone("", "Dubai");
    assert_eq!(zone_fallback, "DUB");

    // 3. Document formatting tests
    let inv_num = NumberingEngine::format_invoice_number("100", "AGT", 1);
    assert!(inv_num.starts_with("INV-100-AGT-"));
    assert!(inv_num.ends_with("001"));

    let do_num = NumberingEngine::format_delivery_number("100", "AGT", 1);
    assert!(do_num.starts_with("DO-100-AGT-"));
    assert!(do_num.ends_with("001"));
}

#[test]
fn test_double_entry_invoice_posting() {
    let repo = Repository::in_memory().unwrap();
    let accounts = repo.get_accounts().unwrap();

    let ar = accounts.iter().find(|a| a.code == "1200").unwrap();
    let rev = accounts.iter().find(|a| a.code == "4000").unwrap();
    let tax = accounts.iter().find(|a| a.code == "2200").unwrap();
    let cash = accounts.iter().find(|a| a.code == "1000").unwrap();

    let invoice = Invoice {
        id: Uuid::new_v4(),
        invoice_number: "INV-100-AGT-26001".to_string(),
        customer_id: Uuid::new_v4(),
        customer_name: "Aref General Trading".to_string(),
        postal_code: "10001".to_string(),
        postal_zone: "100".to_string(),
        issue_date: chrono::Utc::now().date_naive(),
        due_date: chrono::Utc::now().date_naive(),
        status: InvoiceStatus::Sent,
        items: vec![InvoiceItem {
            id: Uuid::new_v4(),
            description: "Industrial Valves".to_string(),
            quantity: dec!(10),
            unit: "Pcs".to_string(),
            unit_price: dec!(100),
            discount_pct: Decimal::ZERO,
            amount: dec!(1000),
        }],
        subtotal: dec!(1000),
        tax_rate: dec!(5),
        tax_amount: dec!(50),
        total_amount: dec!(1050),
        currency: "USD".to_string(),
        notes: "Net 30".to_string(),
        journal_entry_id: None,
        created_at: chrono::Utc::now(),
    };

    // Post Invoice: Debit AR $1050, Credit Revenue $1000, Credit Tax $50
    let je = AccountingEngine::create_invoice_journal_entry(&invoice, ar, rev, tax, 1).unwrap();
    assert_eq!(je.lines.len(), 3);
    AccountingEngine::validate_entry(&je).unwrap();

    repo.insert_journal_entry(&je, true).unwrap();

    // Check account balances
    let accs_after = repo.get_accounts().unwrap();
    let ar_after = accs_after.iter().find(|a| a.code == "1200").unwrap();
    let rev_after = accs_after.iter().find(|a| a.code == "4000").unwrap();
    let tax_after = accs_after.iter().find(|a| a.code == "2200").unwrap();

    assert_eq!(ar_after.balance, dec!(1050));
    assert_eq!(rev_after.balance, dec!(1000));
    assert_eq!(tax_after.balance, dec!(50));

    // Post Payment: Debit Cash $1050, Credit AR $1050
    let pay_je = AccountingEngine::create_payment_journal_entry(&invoice, cash, ar, 2).unwrap();
    repo.insert_journal_entry(&pay_je, true).unwrap();

    let accs_final = repo.get_accounts().unwrap();
    let cash_final = accs_final.iter().find(|a| a.code == "1000").unwrap();
    let ar_final = accs_final.iter().find(|a| a.code == "1200").unwrap();

    assert_eq!(cash_final.balance, dec!(1050));
    assert_eq!(ar_final.balance, dec!(0)); // AR cleared!
}

#[test]
fn test_vendor_bill_posting_and_payment() {
    use Surf::core::model::{BillStatus, InvoiceItem, Vendor, VendorBill};

    let repo = Repository::in_memory().unwrap();

    // Vendor persistence roundtrip
    let vendor = Vendor {
        id: Uuid::new_v4(),
        name: "Gulf Steel Supply".to_string(),
        code: "GSS".to_string(),
        email: "sales@gulfsteel.ae".to_string(),
        phone: "+971 4 555 0100".to_string(),
        address: "Warehouse 12, Industrial Area".to_string(),
        city: "Sharjah".to_string(),
        country: "UAE".to_string(),
        postal_code: "51111".to_string(),
        currency: "AED".to_string(),
        payment_terms_days: 45,
        notes: "".to_string(),
        created_at: chrono::Utc::now(),
    };
    repo.insert_vendor(&vendor, true).unwrap();
    let vendors = repo.get_vendors().unwrap();
    assert_eq!(vendors.len(), 1);
    assert_eq!(vendors[0].payment_terms_days, 45);

    // Bill: 5000 subtotal + 5% tax = 5250
    let bill = VendorBill {
        id: Uuid::new_v4(),
        bill_number: NumberingEngine::format_bill_number(1),
        vendor_id: vendor.id,
        vendor_name: vendor.name.clone(),
        issue_date: chrono::Utc::now().date_naive(),
        due_date: chrono::Utc::now().date_naive() + chrono::Duration::days(45),
        status: BillStatus::Unpaid,
        items: vec![InvoiceItem {
            id: Uuid::new_v4(),
            description: "Steel Beams".to_string(),
            quantity: dec!(50),
            unit: "Pcs".to_string(),
            unit_price: dec!(100),
            discount_pct: Decimal::ZERO,
            amount: dec!(5000),
        }],
        subtotal: dec!(5000),
        tax_rate: dec!(5),
        tax_amount: dec!(250),
        total_amount: dec!(5250),
        currency: "AED".to_string(),
        notes: "".to_string(),
        journal_entry_id: None,
        created_at: chrono::Utc::now(),
    };

    // Post bill: Debit Expense 5000, Debit Input Tax 250, Credit AP 5250
    let accounts = repo.get_accounts().unwrap();
    let ap = accounts.iter().find(|a| a.code == "2000").unwrap();
    let expense = accounts.iter().find(|a| a.code == "6000").unwrap();
    let tax = accounts.iter().find(|a| a.code == "2200").unwrap();
    let cash = accounts.iter().find(|a| a.code == "1000").unwrap();

    let je = AccountingEngine::create_bill_journal_entry(&bill, ap, expense, tax, 1).unwrap();
    AccountingEngine::validate_entry(&je).unwrap();
    assert_eq!(je.lines.len(), 3);
    repo.insert_journal_entry(&je, true).unwrap();

    let accs_after = repo.get_accounts().unwrap();
    let ap_after = accs_after.iter().find(|a| a.code == "2000").unwrap();
    let exp_after = accs_after.iter().find(|a| a.code == "6000").unwrap();
    let tax_after = accs_after.iter().find(|a| a.code == "2200").unwrap();
    assert_eq!(ap_after.balance, dec!(5250));
    assert_eq!(exp_after.balance, dec!(5000));
    // Input tax (250 debit) offsets the seeded 0 credit-normal balance.
    assert_eq!(tax_after.balance, dec!(-250));

    // Pay bill: Debit AP, Credit Cash
    let mut paid = bill.clone();
    paid.status = BillStatus::Paid;
    let pay_je = AccountingEngine::create_bill_payment_journal_entry(&paid, cash, ap, 2).unwrap();
    repo.insert_journal_entry(&pay_je, true).unwrap();

    let accs_final = repo.get_accounts().unwrap();
    let ap_final = accs_final.iter().find(|a| a.code == "2000").unwrap();
    let cash_final = accs_final.iter().find(|a| a.code == "1000").unwrap();
    assert_eq!(ap_final.balance, dec!(0));
    assert_eq!(cash_final.balance, dec!(-5250));

    // Bill persistence roundtrip
    repo.insert_vendor_bill(&paid, true).unwrap();
    let bills = repo.get_vendor_bills().unwrap();
    assert_eq!(bills.len(), 1);
    assert_eq!(bills[0].status, BillStatus::Paid);
    assert_eq!(bills[0].vendor_name, "Gulf Steel Supply");
}

#[test]
fn test_product_catalog_persistence_and_kinds() {
    use Surf::core::model::{Product, ProductKind};

    let repo = Repository::in_memory().unwrap();

    let sellable = Product {
        id: Uuid::new_v4(),
        name: "Consulting Hour".to_string(),
        kind: ProductKind::Sale,
        unit: "Hour".to_string(),
        default_price: dec!(150),
        tax_rate: dec!(5),
        income_account_code: "4000".to_string(),
        expense_account_code: "6000".to_string(),
        active: true,
        created_at: chrono::Utc::now(),
    };
    let purchasable = Product {
        id: Uuid::new_v4(),
        name: "Packaging Material".to_string(),
        kind: ProductKind::Purchase,
        unit: "Box".to_string(),
        default_price: dec!(12),
        tax_rate: Decimal::ZERO,
        income_account_code: "4000".to_string(),
        expense_account_code: "6000".to_string(),
        active: true,
        created_at: chrono::Utc::now(),
    };
    repo.insert_product(&sellable, true).unwrap();
    repo.insert_product(&purchasable, true).unwrap();

    let products = repo.get_products().unwrap();
    assert_eq!(products.len(), 2);

    let consulting = products
        .iter()
        .find(|p| p.name == "Consulting Hour")
        .unwrap();
    assert_eq!(consulting.kind, ProductKind::Sale);
    assert!(consulting.kind.sellable());
    assert!(!consulting.kind.purchasable());
    assert_eq!(consulting.default_price, dec!(150));

    let packaging = products
        .iter()
        .find(|p| p.name == "Packaging Material")
        .unwrap();
    assert!(packaging.kind.purchasable());
    assert!(!packaging.kind.sellable());
}

#[test]
fn test_sync_bundle_roundtrip() {
    let repo_a = Repository::in_memory().unwrap();
    let repo_b = Repository::in_memory().unwrap();

    let customer = Surf::core::model::Customer {
        id: Uuid::new_v4(),
        name: "Sync Test Corp".to_string(),
        code: "STC".to_string(),
        email: "sync@test.com".to_string(),
        phone: "+1-555-0199".to_string(),
        address: "123 Market St".to_string(),
        city: "San Francisco".to_string(),
        country: "USA".to_string(),
        postal_code: "94103".to_string(),
        postal_zone: "941".to_string(),
        currency: "USD".to_string(),
        notes: "".to_string(),
        created_at: chrono::Utc::now(),
    };

    repo_a.insert_customer(&customer, true).unwrap();

    // Export sync bundle from device A
    let bundle = repo_a.export_sync_bundle().unwrap();
    let json = bundle.to_json().unwrap();

    // Import sync bundle into device B
    let imported_bundle = SyncBundle::from_json(&json).unwrap();
    let count = repo_b.apply_sync_bundle(&imported_bundle).unwrap();
    assert_eq!(count, 1);

    // Verify Customer exists in device B
    let customers_b = repo_b.get_customers().unwrap();
    assert_eq!(customers_b.len(), 1);
    assert_eq!(customers_b[0].name, "Sync Test Corp");
    assert_eq!(customers_b[0].postal_zone, "941");
}

#[test]
fn test_fx_rate_persistence_and_cross_conversion() {
    let repo = Repository::in_memory().unwrap();

    // tgju free-market sample values (2026-10-19): 1 USD = 2,638,000 IRR etc.
    let today = chrono::Utc::now().date_naive();
    for (code, iri) in [
        ("USD", 2_638_000i64),
        ("AED", 721_260),
        ("EUR", 2_956_900),
        ("CNY", 395_600),
    ] {
        let fx = Surf::core::fx::FxRate {
            id: Uuid::new_v4(),
            base: Surf::core::money::Currency::IRR,
            quote: Surf::core::money::Currency::parse(code).unwrap(),
            rate: Decimal::from(iri),
            as_of_date: today,
            source: Surf::core::fx::RateSource::Tgju,
            source_url: Some("https://www.tgju.org/profile/price_aed".to_string()),
            raw_value: Some(iri.to_string()),
            fetched_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
        };
        repo.upsert_fx_rate(&fx).unwrap();
    }

    let stored = repo.get_all_fx_rates().unwrap();
    assert_eq!(stored.len(), 4);

    // Cross conversion: 1000 AED -> USD = 1000 * 721260 / 2638000
    let rates = repo.get_all_fx_rates().unwrap();
    let usd = Surf::core::money::format_money_by_code(
        "USD",
        Surf::core::fx::convert(
            Decimal::from(1000),
            Surf::core::money::Currency::AED,
            Surf::core::money::Currency::USD,
            &rates,
        )
        .unwrap(),
    );
    // 1000 AED = 721,260,000 IRR = 273.4118 USD
    assert!(usd.contains("273.41"), "unexpected USD format: {usd}");

    // Upserting on the same (quote, date) does not duplicate rows.
    let count_before = repo.get_all_fx_rates().unwrap().len();
    repo.upsert_fx_rate(&Surf::core::fx::FxRate {
        id: Uuid::new_v4(),
        quote: Surf::core::money::Currency::AED,
        ..Surf::core::fx::FxRate {
            id: Uuid::new_v4(),
            base: Surf::core::money::Currency::IRR,
            quote: Surf::core::money::Currency::AED,
            rate: Decimal::from(800_000),
            as_of_date: today,
            source: Surf::core::fx::RateSource::Manual,
            source_url: None,
            raw_value: None,
            fetched_at: Some(chrono::Utc::now()),
            created_at: chrono::Utc::now(),
        }
    })
    .unwrap();
    assert_eq!(
        repo.get_all_fx_rates().unwrap().len(),
        count_before,
        "upsert should replace, not append"
    );
}

#[test]
fn test_company_profile_and_base_currency_persistence() {
    let repo = Repository::in_memory().unwrap();

    // Base currency defaults to IRR and round-trips.
    assert_eq!(repo.get_base_currency(), "IRR");
    repo.set_base_currency("aed").unwrap();
    assert_eq!(repo.get_base_currency(), "AED"); // normalized case
    repo.set_base_currency("GBP").unwrap(); // unsupported currency falls back to IRR
    assert_eq!(repo.get_base_currency(), "IRR");

    // Company profile persists with custom edits.
    let mut profile = repo.get_company_profile();
    assert_eq!(profile.legal_name, "Surf Commercial Services LLC");
    profile.legal_name = "Aref General Trading LLC".to_string();
    repo.save_company_profile(&profile).unwrap();
    let reloaded = repo.get_company_profile();
    assert_eq!(reloaded.legal_name, "Aref General Trading LLC");
    assert_eq!(reloaded.tax_id, profile.tax_id);
}

#[test]
fn test_tgju_fixture_parsing() {
    // Mirrors the live structure of a tgju.org profile page.
    let html = r#"<div class="info-panel"><span class="price" data-col="info.last_trade.PDrCotVal">2,638,000</span></div>"#;
    assert_eq!(fx_provider::parse_tgju_html(html).unwrap(), 2_638_000);

    assert_eq!(
        fx_provider::parse_tgju_html(
            r#"<span data-col="info.last_trade.PDrCotVal">721,260</span>"#
        )
        .unwrap(),
        721_260
    );
    assert!(fx_provider::parse_tgju_html("<div>no price here</div>").is_err());
}

#[test]
fn test_invoice_pdf_and_image_export() {
    use Surf::core::export::InvoiceExporter;
    use Surf::core::model::{CompanyProfile, Invoice, InvoiceItem, InvoiceStatus};

    let invoice = Invoice {
        id: Uuid::new_v4(),
        invoice_number: "INV-DXB-AGT-2026-001".to_string(),
        customer_id: Uuid::new_v4(),
        customer_name: "Aref General Trading".to_string(),
        postal_code: "10001".to_string(),
        postal_zone: "DXB".to_string(),
        issue_date: chrono::Utc::now().date_naive(),
        due_date: chrono::Utc::now().date_naive(),
        status: InvoiceStatus::Sent,
        items: vec![InvoiceItem {
            id: Uuid::new_v4(),
            description: "Commercial Turbine Valves".to_string(),
            quantity: Decimal::from(5),
            unit: "Pcs".to_string(),
            unit_price: Decimal::from(450),
            discount_pct: Decimal::from(5),
            amount: Decimal::from(2137), // 5 * 450 * 0.95 = 2137.5 -> rounded
        }],
        subtotal: Decimal::from(2137),
        tax_rate: Decimal::from(5),
        tax_amount: Decimal::from(106),
        total_amount: Decimal::from(2243),
        currency: "USD".to_string(),
        notes: "Net 30. Wire transfer only.".to_string(),
        journal_entry_id: None,
        created_at: chrono::Utc::now(),
    };

    let company = CompanyProfile::default();
    let tmp_dir = std::env::temp_dir().join("surf_export_test");

    // 1. PDF Export Test
    let pdf_path = InvoiceExporter::export_to_pdf(&invoice, &company, &tmp_dir).unwrap();
    assert!(pdf_path.exists(), "PDF file must exist");
    let pdf_bytes = std::fs::read(&pdf_path).unwrap();
    assert!(
        pdf_bytes.starts_with(b"%PDF-1.4"),
        "Must be a valid PDF 1.4 file"
    );
    assert!(pdf_bytes.len() > 100, "PDF must not be empty");

    // 2. PNG Image Export Test
    let png_path = InvoiceExporter::export_to_image(&invoice, &company, &tmp_dir).unwrap();
    assert!(png_path.exists(), "PNG file must exist");
    let png_bytes = std::fs::read(&png_path).unwrap();
    assert!(
        png_bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "Must be a valid PNG file header"
    );
    assert!(png_bytes.len() > 1000, "PNG must contain image data");

    // Clean up
    let _ = std::fs::remove_file(pdf_path);
    let _ = std::fs::remove_file(png_path);
    let _ = std::fs::remove_dir(tmp_dir);
}
