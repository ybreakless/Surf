use crate::core::model::{CompanyProfile, DeliveryOrder, Invoice};
use crate::core::money;

/// Ready-text languages for client communications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageLang {
    English,
    Arabic,
    Persian,
}

impl MessageLang {
    pub const ALL: [MessageLang; 3] = [
        MessageLang::English,
        MessageLang::Arabic,
        MessageLang::Persian,
    ];

    pub fn name(self) -> &'static str {
        match self {
            MessageLang::English => "English",
            MessageLang::Arabic => "العربية",
            MessageLang::Persian => "فارسی",
        }
    }

    /// Pulls the language stored in the database (defaults to English).
    pub fn from_code(code: &str) -> Self {
        match code {
            "ar" => MessageLang::Arabic,
            "fa" => MessageLang::Persian,
            _ => MessageLang::English,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            MessageLang::English => "en",
            MessageLang::Arabic => "ar",
            MessageLang::Persian => "fa",
        }
    }

    /// Greeting line including the customer display name.
    fn greeting_line(self, name: &str) -> String {
        match self {
            MessageLang::English => format!("Dear {},\n", name),
            MessageLang::Arabic => format!("عزيزي {}،\n", name),
            MessageLang::Persian => format!("جناب {} عزیز،\n", name),
        }
    }

    fn sign_off(self, company: &CompanyProfile) -> String {
        match self {
            MessageLang::English => format!(
                "Kind regards,\n{}\n{}\n{}",
                company.trading_name, company.phone, company.website
            ),
            MessageLang::Arabic => format!(
                "مع أطيب التحيات،\n{}\n{}\n{}",
                company.trading_name, company.phone, company.website
            ),
            MessageLang::Persian => format!(
                "با احترام،\n{}\n{}\n{}",
                company.trading_name, company.phone, company.website
            ),
        }
    }
}

/// Email subject for an invoice.
pub fn invoice_subject(lang: MessageLang, invoice: &Invoice, company: &CompanyProfile) -> String {
    match lang {
        MessageLang::English => format!(
            "Invoice {} from {}",
            invoice.invoice_number, company.trading_name
        ),
        MessageLang::Arabic => format!(
            "فاتورة رقم {} من {}",
            invoice.invoice_number, company.trading_name
        ),
        MessageLang::Persian => format!(
            "صورت‌حساب {} از {}",
            invoice.invoice_number, company.trading_name
        ),
    }
}

/// Ready-to-send invoice message (email body / WhatsApp / Telegram text).
pub fn compose_invoice_message(
    lang: MessageLang,
    invoice: &Invoice,
    company: &CompanyProfile,
) -> String {
    let amount = money::format_money_by_code(&invoice.currency, invoice.total_amount);
    let mut body = String::new();
    match lang {
        MessageLang::English => {
            body.push_str(&lang.greeting_line(&invoice.customer_name));
            body.push_str(&format!(
                "Thank you for your business. Please find invoice {} attached, issued on {}.\n\n",
                invoice.invoice_number, invoice.issue_date
            ));
            body.push_str("INVOICE SUMMARY\n");
            body.push_str(&format!("Invoice #: {}\n", invoice.invoice_number));
            body.push_str(&format!("Due date: {}\n", invoice.due_date));
            body.push_str(&format!("Total due: {}\n", amount));
            body.push('\n');
            body.push_str("BANK WIRE REMITTANCE DETAILS\n");
            body.push_str(&format!(
                "Bank: {} | IBAN: {} | SWIFT/BIC: {}\n",
                company.bank_name, company.bank_iban, company.bank_swift
            ));
            body.push_str(&format!(
                "Beneficiary: {} | Payment reference: {}\n\n",
                company.legal_name, invoice.invoice_number
            ));
            body.push_str("If you have any questions, please feel free to reach out.\n\n");
        }
        MessageLang::Arabic => {
            body.push_str(&lang.greeting_line(&invoice.customer_name));
            body.push_str(&format!(
                "نشكر لكم ثقتكم. تجدون مرفقًا الفاتورة {} الصادرة بتاريخ {}.\n\n",
                invoice.invoice_number, invoice.issue_date
            ));
            body.push_str("ملخص الفاتورة\n");
            body.push_str(&format!("رقم الفاتورة: {}\n", invoice.invoice_number));
            body.push_str(&format!("تاريخ الاستحقاق: {}\n", invoice.due_date));
            body.push_str(&format!("المبلغ المستحق: {}\n", amount));
            body.push('\n');
            body.push_str("معلومات الحوالة البنكية\n");
            body.push_str(&format!(
                "البنك: {} | الآيبان: {} | السويفت: {}\n",
                company.bank_name, company.bank_iban, company.bank_swift
            ));
            body.push_str(&format!(
                "المستفيد: {} | مرجع الدفع: {}\n\n",
                company.legal_name, invoice.invoice_number
            ));
            body.push_str("للاستفسار، لا تترددوا في التواصل معنا.\n\n");
        }
        MessageLang::Persian => {
            body.push_str(&lang.greeting_line(&invoice.customer_name));
            body.push_str(&format!(
                "از همکاری شما سپاسگزاریم. صورت‌حساب {} با تاریخ صدور {} پیوست است.\n\n",
                invoice.invoice_number, invoice.issue_date
            ));
            body.push_str("خلاصه صورت‌حساب\n");
            body.push_str(&format!("شماره: {}\n", invoice.invoice_number));
            body.push_str(&format!("تاریخ سررسید: {}\n", invoice.due_date));
            body.push_str(&format!("مبلغ قابل پرداخت: {}\n", amount));
            body.push('\n');
            body.push_str("اطلاعات حواله بانکی\n");
            body.push_str(&format!(
                "بانک: {} | شبا: {} | سوئیفت: {}\n",
                company.bank_name, company.bank_iban, company.bank_swift
            ));
            body.push_str(&format!(
                "ذینفع: {} | مرجع پرداخت: {}\n\n",
                company.legal_name, invoice.invoice_number
            ));
            body.push_str("برای هر سؤالی در خدمت شما هستیم.\n\n");
        }
    }
    body.push_str(&lang.sign_off(company));
    body
}

/// Email subject for a shipping status update.
pub fn shipping_subject(lang: MessageLang, delivery: &DeliveryOrder, event: &str) -> String {
    match lang {
        MessageLang::English => format!(
            "Shipping Update: Delivery Order {} is {}",
            delivery.delivery_number, event
        ),
        MessageLang::Arabic => format!(
            "تحديث شحن: أمر التسليم {} - {}",
            delivery.delivery_number, event
        ),
        MessageLang::Persian => format!(
            "به‌روزرسانی ارسال: سفارش تحویل {} - {}",
            delivery.delivery_number, event
        ),
    }
}

/// Ready-to-send shipping status message.
pub fn compose_shipping_message(
    lang: MessageLang,
    delivery: &DeliveryOrder,
    event: &str,
    carrier: &str,
    tracking: &str,
    company: &CompanyProfile,
) -> String {
    let mut body = String::new();
    match lang {
        MessageLang::English => {
            body.push_str(&lang.greeting_line(&delivery.customer_name));
            body.push_str(&format!(
                "We are writing to update you on your shipment under Delivery Order {}.\n\n",
                delivery.delivery_number
            ));
            body.push_str("LOGISTICS STATUS\n");
            body.push_str(&format!("Tracking: {} ({})\n", tracking, carrier));
            body.push_str(&format!("Status: {}\n", event));
            body.push_str(&format!("Scheduled date: {}\n", delivery.delivery_date));
            body.push_str(&format!("Destination: {}\n", delivery.delivery_address));
            body.push('\n');
            body.push_str("PACKAGE MANIFEST\n");
            for (i, it) in delivery.items.iter().enumerate() {
                body.push_str(&format!(
                    "{}. {} (Qty: {} {})\n",
                    i + 1,
                    it.description,
                    it.quantity,
                    it.unit
                ));
            }
            body.push_str(&format!("\nHandling notes: {}\n\n", delivery.driver_notes));
        }
        MessageLang::Arabic => {
            body.push_str(&lang.greeting_line(&delivery.customer_name));
            body.push_str(&format!(
                "نود إبلاغكم بتحديث حالة شحنتكم بموجب أمر التسليم {}.\n\n",
                delivery.delivery_number
            ));
            body.push_str("حالة الخدمات اللوجستية\n");
            body.push_str(&format!("التفوي: {} ({})\n", tracking, carrier));
            body.push_str(&format!("الحالة: {}\n", event));
            body.push_str(&format!("التاريخ المقرر: {}\n", delivery.delivery_date));
            body.push_str(&format!("الوجهة: {}\n", delivery.delivery_address));
            body.push('\n');
            body.push_str("بيان الحزمة\n");
            for (i, it) in delivery.items.iter().enumerate() {
                body.push_str(&format!(
                    "{}. {} (الكمية: {} {})\n",
                    i + 1,
                    it.description,
                    it.quantity,
                    it.unit
                ));
            }
            body.push_str(&format!("\nملاحظات التداول: {}\n\n", delivery.driver_notes));
        }
        MessageLang::Persian => {
            body.push_str(&lang.greeting_line(&delivery.customer_name));
            body.push_str(&format!(
                " برای اطلاع شما از وضعیت محموله خود در سفارش تحویل {}، این پیام ارسال شده است.\n\n",
                delivery.delivery_number
            ));
            body.push_str("وضعیت لجستیک\n");
            body.push_str(&format!("ردیابی: {} ({})\n", tracking, carrier));
            body.push_str(&format!("وضعیت: {}\n", event));
            body.push_str(&format!("تاریخ برنامه‌ریزی: {}\n", delivery.delivery_date));
            body.push_str(&format!("مقصد: {}\n", delivery.delivery_address));
            body.push('\n');
            body.push_str("بارنامه\n");
            for (i, it) in delivery.items.iter().enumerate() {
                body.push_str(&format!(
                    "{}. {} (تعداد: {} {})\n",
                    i + 1,
                    it.description,
                    it.quantity,
                    it.unit
                ));
            }
            body.push_str(&format!("\nیادداشت حمل: {}\n\n", delivery.driver_notes));
        }
    }
    body.push_str(&lang.sign_off(company));
    body
}

/// Short share text used by WhatsApp / Telegram buttons under an invoice.
pub fn compose_short_invoice_share(lang: MessageLang, invoice: &Invoice) -> String {
    let amount = money::format_money_by_code(&invoice.currency, invoice.total_amount);
    match lang {
        MessageLang::English => format!(
            "Hello, invoice {} is attached ({}). Due {}. Payment reference available on the document.",
            invoice.invoice_number, amount, invoice.due_date
        ),
        MessageLang::Arabic => format!(
            "مرحبًا، مرفقًا الفاتورة {} ({}). تاريخ الاستحقاق {}. مرجع الدفع على الملف.",
            invoice.invoice_number, amount, invoice.due_date
        ),
        MessageLang::Persian => format!(
            "سلام، صورت‌حساب {} پیوست شده است ({}). سررسید {}. مرجع پرداخت داخل فایل آمده است.",
            invoice.invoice_number, amount, invoice.due_date
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::{
        CompanyProfile, Customer, DeliveryItem, DeliveryOrder, DeliveryStatus, Invoice,
        InvoiceItem, InvoiceStatus,
    };
    use chrono::{NaiveDate, Utc};
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn sample_company() -> CompanyProfile {
        CompanyProfile {
            trading_name: "Surf Trading".into(),
            phone: "+971500000000".into(),
            website: "example.com".into(),
            bank_name: "EAB".into(),
            bank_iban: "AE00".into(),
            bank_swift: "EABK".into(),
            legal_name: "Surf Trading LLC".into(),
            ..Default::default()
        }
    }

    fn sample_customer() -> Customer {
        Customer {
            id: Uuid::new_v4(),
            name: "Al Fateh Trading".into(),
            code: "AFT".into(),
            email: "buy@example.com".into(),
            phone: "+971500000001".into(),
            address: "Shop 5".into(),
            city: "Dubai".into(),
            country: "UAE".into(),
            postal_code: "00000".into(),
            postal_zone: "100".into(),
            currency: "USD".into(),
            notes: String::new(),
            created_at: Utc::now(),
        }
    }

    fn sample_invoice() -> Invoice {
        let customer = sample_customer();
        Invoice {
            id: Uuid::new_v4(),
            invoice_number: "INV-100-AAA-0001".into(),
            customer_id: customer.id,
            customer_name: customer.name.clone(),
            postal_code: "00000".into(),
            postal_zone: "100".into(),
            issue_date: NaiveDate::from_ymd_opt(2026, 10, 19).unwrap(),
            due_date: NaiveDate::from_ymd_opt(2026, 11, 18).unwrap(),
            status: InvoiceStatus::Sent,
            items: vec![InvoiceItem {
                id: Uuid::new_v4(),
                description: "Copper wire".into(),
                quantity: dec!(2),
                unit: "Pcs".into(),
                unit_price: dec!(100),
                discount_pct: dec!(0),
                amount: dec!(200),
            }],
            subtotal: dec!(200),
            tax_rate: dec!(5),
            tax_amount: dec!(10),
            total_amount: dec!(210),
            currency: "USD".into(),
            notes: String::new(),
            journal_entry_id: None,
            created_at: Utc::now(),
        }
    }

    fn sample_delivery() -> DeliveryOrder {
        DeliveryOrder {
            id: Uuid::new_v4(),
            delivery_number: "DO-100-AAA-0001".into(),
            invoice_id: None,
            invoice_number: None,
            customer_id: Uuid::new_v4(),
            customer_name: "Test Buyer".into(),
            delivery_address: "Warehouse 12".into(),
            postal_code: "00000".into(),
            postal_zone: "100".into(),
            delivery_date: NaiveDate::from_ymd_opt(2026, 10, 20).unwrap(),
            status: DeliveryStatus::Pending,
            items: vec![DeliveryItem {
                id: Uuid::new_v4(),
                description: "Copper wire".into(),
                quantity: dec!(2),
                unit: "Pcs".into(),
            }],
            driver_notes: "Keep dry".into(),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn all_langs_produce_invoice_text() {
        let inv = sample_invoice();
        let company = sample_company();
        for lang in MessageLang::ALL {
            let body = compose_invoice_message(lang, &inv, &company);
            assert!(
                body.contains("INV-100-AAA-0001"),
                "missing number for {:?}",
                lang
            );
            assert!(
                body.matches(company.bank_iban.as_str()).count() >= 1,
                "missing bank for {:?}",
                lang
            );
            let subject = invoice_subject(lang, &inv, &company);
            assert!(subject.contains("INV-100-AAA-0001"));
        }
        // Language-specific markers
        assert!(compose_invoice_message(MessageLang::Persian, &inv, &company).contains("صورت‌حساب"));
        assert!(compose_invoice_message(MessageLang::Arabic, &inv, &company).contains("فاتورة"));
        assert!(compose_invoice_message(MessageLang::English, &inv, &company).starts_with("Dear"));
    }

    #[test]
    fn shipping_text_contains_details() {
        let d = sample_delivery();
        let company = sample_company();
        for lang in MessageLang::ALL {
            let body =
                compose_shipping_message(lang, &d, "Dispatched", "Surf Freight", "TRK-1", &company);
            assert!(body.contains("DO-100-AAA-0001"));
            assert!(body.contains("Copper wire"));
        }
        assert!(
            shipping_subject(MessageLang::English, &d, "Dispatched").contains("DO-100-AAA-0001")
        );
    }

    #[test]
    fn short_share_is_compact_and_localized() {
        let inv = sample_invoice();
        let en = compose_short_invoice_share(MessageLang::English, &inv);
        assert!(en.chars().count() < 200);
        assert!(en.contains("210.00"));
    }

    #[test]
    fn lang_code_roundtrip() {
        for lang in MessageLang::ALL {
            assert_eq!(MessageLang::from_code(lang.code()), lang);
        }
    }
}
