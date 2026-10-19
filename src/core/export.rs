use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use image::{ImageBuffer, Rgba};

use super::model::{CompanyProfile, Invoice};

pub struct InvoiceExporter;

impl InvoiceExporter {
    /// Generates a professional, vector-based A4 PDF invoice with company details, tables, and stamp
    pub fn export_to_pdf(
        invoice: &Invoice,
        company: &CompanyProfile,
        output_dir: &Path,
    ) -> Result<PathBuf, String> {
        fs::create_dir_all(output_dir)
            .map_err(|e| format!("Failed to create output dir: {}", e))?;
        let filename = format!("{}.pdf", invoice.invoice_number.replace('/', "_"));
        let pdf_path = output_dir.join(filename);

        let pdf_content = Self::generate_pdf_bytes(invoice, company);
        let mut file =
            File::create(&pdf_path).map_err(|e| format!("Failed to create PDF file: {}", e))?;
        file.write_all(&pdf_content)
            .map_err(|e| format!("Failed to write PDF: {}", e))?;

        Ok(pdf_path)
    }

    /// Generates a high-resolution PNG image of the invoice (A4 ratio, high DPI)
    pub fn export_to_image(
        invoice: &Invoice,
        company: &CompanyProfile,
        output_dir: &Path,
    ) -> Result<PathBuf, String> {
        fs::create_dir_all(output_dir)
            .map_err(|e| format!("Failed to create output dir: {}", e))?;
        let filename = format!("{}.png", invoice.invoice_number.replace('/', "_"));
        let img_path = output_dir.join(filename);

        let png_bytes = Self::render_png_bytes(invoice, company)?;
        fs::write(&img_path, &png_bytes)
            .map_err(|e| format!("Failed to save invoice image: {}", e))?;
        Ok(img_path)
    }

    /// Renders the invoice document to PNG bytes without touching the disk.
    /// Used for the in-app preview.
    pub fn render_png_bytes(
        invoice: &Invoice,
        company: &CompanyProfile,
    ) -> Result<Vec<u8>, String> {
        let width = 1240;
        let height = 1754;
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(width, height, Rgba([250, 250, 252, 255]));

        // Draw professional document canvas
        Self::draw_invoice_canvas(&mut img, invoice, company);

        let mut bytes = std::io::Cursor::new(Vec::new());
        img.write_to(&mut bytes, image::ImageFormat::Png)
            .map_err(|e| format!("Failed to encode invoice preview: {}", e))?;
        Ok(bytes.into_inner())
    }

    /// Raw RGBA pixel buffer for direct egui texture upload (in-app preview).
    pub fn render_rgba_pixels(
        invoice: &Invoice,
        company: &CompanyProfile,
    ) -> (Vec<u8>, [usize; 2]) {
        let width = 1240usize;
        let height = 1754usize;
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(width as u32, height as u32, Rgba([250, 250, 252, 255]));
        Self::draw_invoice_canvas(&mut img, invoice, company);
        (img.into_raw(), [width, height])
    }

    /// Generates standard PDF-1.4 vector bytes for the invoice
    fn generate_pdf_bytes(invoice: &Invoice, company: &CompanyProfile) -> Vec<u8> {
        let mut stream = String::new();

        // Background card
        stream.push_str("q 0.98 0.98 0.99 rg 0 0 595 842 re f Q\n");

        // Top Accent Banner (Surf Blue)
        stream.push_str("q 0.18 0.49 0.90 rg 0 822 595 20 re f Q\n");

        // Header Text: Company & Invoice Info
        stream.push_str("BT /F1 22 Tf 0.1 0.1 0.15 rg 40 780 Td (TAX INVOICE) Tj ET\n");
        stream.push_str(&format!(
            "BT /F1 12 Tf 0.18 0.49 0.90 rg 40 762 Td ({}) Tj ET\n",
            escape_pdf(&invoice.invoice_number)
        ));

        // Status & Postal Zone badge
        stream.push_str(&format!(
            "BT /F2 10 Tf 0.4 0.4 0.5 rg 40 746 Td (Status: {}  |  Logistics Zone: {}) Tj ET\n",
            invoice.status.as_str(),
            escape_pdf(&invoice.postal_zone)
        ));

        // Company Details (Right-aligned / upper right)
        stream.push_str(&format!(
            "BT /F1 13 Tf 0.1 0.1 0.1 rg 340 780 Td ({}) Tj ET\n",
            escape_pdf(&company.legal_name)
        ));
        stream.push_str(&format!(
            "BT /F2 9 Tf 0.35 0.35 0.4 rg 340 765 Td ({}) Tj ET\n",
            escape_pdf(&company.address)
        ));
        stream.push_str(&format!(
            "BT /F2 9 Tf 0.35 0.35 0.4 rg 340 752 Td ({}, {}) Tj ET\n",
            escape_pdf(&company.city),
            escape_pdf(&company.country)
        ));
        stream.push_str(&format!(
            "BT /F2 9 Tf 0.35 0.35 0.4 rg 340 739 Td (Tax ID / TRN: {}) Tj ET\n",
            escape_pdf(&company.tax_id)
        ));
        stream.push_str(&format!(
            "BT /F2 9 Tf 0.35 0.35 0.4 rg 340 726 Td (Email: {}  |  Tel: {}) Tj ET\n",
            escape_pdf(&company.email),
            escape_pdf(&company.phone)
        ));

        // Horizontal Separator Rule
        stream.push_str("q 0.85 0.86 0.90 RG 1 w 40 710 m 555 710 l S Q\n");

        // Bill To Box & Dates Box
        stream.push_str(
            "q 0.95 0.96 0.98 rg 40 625 250 75 re f 0.85 0.86 0.90 RG 1 w 40 625 250 75 re S Q\n",
        );
        stream.push_str("BT /F1 10 Tf 0.18 0.49 0.90 rg 52 684 Td (BILLED TO:) Tj ET\n");
        stream.push_str(&format!(
            "BT /F1 11 Tf 0.1 0.1 0.1 rg 52 668 Td ({}) Tj ET\n",
            escape_pdf(&invoice.customer_name)
        ));
        stream.push_str(&format!(
            "BT /F2 9 Tf 0.35 0.35 0.4 rg 52 654 Td (Postal Code: {}  |  Route: {}) Tj ET\n",
            escape_pdf(&invoice.postal_code),
            escape_pdf(&invoice.postal_zone)
        ));
        stream.push_str(&format!(
            "BT /F2 9 Tf 0.35 0.35 0.4 rg 52 640 Td (Currency: {}) Tj ET\n",
            escape_pdf(&invoice.currency)
        ));

        stream.push_str(
            "q 0.95 0.96 0.98 rg 310 625 245 75 re f 0.85 0.86 0.90 RG 1 w 310 625 245 75 re S Q\n",
        );
        stream.push_str("BT /F1 10 Tf 0.18 0.49 0.90 rg 322 684 Td (INVOICE DETAILS:) Tj ET\n");
        stream.push_str(&format!(
            "BT /F2 9 Tf 0.2 0.2 0.2 rg 322 668 Td (Issue Date: {}) Tj ET\n",
            invoice.issue_date
        ));
        stream.push_str(&format!(
            "BT /F2 9 Tf 0.2 0.2 0.2 rg 322 654 Td (Due Date: {}) Tj ET\n",
            invoice.due_date
        ));
        stream.push_str(&format!(
            "BT /F2 9 Tf 0.2 0.2 0.2 rg 322 640 Td (Payment Terms: {}) Tj ET\n",
            escape_pdf(&invoice.notes)
        ));

        // Excel-Style Table Header
        let table_y = 595;
        stream.push_str(&format!(
            "q 0.15 0.18 0.25 rg 40 {} 515 24 re f Q\n",
            table_y
        ));
        stream.push_str(&format!(
            "BT /F1 9 Tf 1 1 1 rg 48 {} Td (#) Tj ET\n",
            table_y + 7
        ));
        stream.push_str(&format!(
            "BT /F1 9 Tf 1 1 1 rg 75 {} Td (Item / Description) Tj ET\n",
            table_y + 7
        ));
        stream.push_str(&format!(
            "BT /F1 9 Tf 1 1 1 rg 290 {} Td (Qty) Tj ET\n",
            table_y + 7
        ));
        stream.push_str(&format!(
            "BT /F1 9 Tf 1 1 1 rg 345 {} Td (Unit) Tj ET\n",
            table_y + 7
        ));
        stream.push_str(&format!(
            "BT /F1 9 Tf 1 1 1 rg 400 {} Td (Price) Tj ET\n",
            table_y + 7
        ));
        stream.push_str(&format!(
            "BT /F1 9 Tf 1 1 1 rg 485 {} Td (Total) Tj ET\n",
            table_y + 7
        ));

        // Table Rows
        let mut curr_y = table_y - 20;
        for (i, item) in invoice.items.iter().enumerate() {
            let row_bg = if i % 2 == 0 {
                "0.98 0.98 0.99"
            } else {
                "1 1 1"
            };
            stream.push_str(&format!(
                "q {} rg 40 {} 515 20 re f 0.90 0.91 0.94 RG 0.5 w 40 {} 515 20 re S Q\n",
                row_bg, curr_y, curr_y
            ));
            stream.push_str(&format!(
                "BT /F2 9 Tf 0.2 0.2 0.2 rg 48 {} Td ({}) Tj ET\n",
                curr_y + 5,
                i + 1
            ));
            stream.push_str(&format!(
                "BT /F1 9 Tf 0.1 0.1 0.1 rg 75 {} Td ({}) Tj ET\n",
                curr_y + 5,
                escape_pdf(&item.description)
            ));
            stream.push_str(&format!(
                "BT /F2 9 Tf 0.2 0.2 0.2 rg 290 {} Td ({}) Tj ET\n",
                curr_y + 5,
                item.quantity
            ));
            stream.push_str(&format!(
                "BT /F2 9 Tf 0.2 0.2 0.2 rg 345 {} Td ({}) Tj ET\n",
                curr_y + 5,
                escape_pdf(&item.unit)
            ));
            stream.push_str(&format!(
                "BT /F2 9 Tf 0.2 0.2 0.2 rg 400 {} Td (${:.2}) Tj ET\n",
                curr_y + 5,
                item.unit_price
            ));
            stream.push_str(&format!(
                "BT /F1 9 Tf 0.1 0.1 0.1 rg 485 {} Td (${:.2}) Tj ET\n",
                curr_y + 5,
                item.amount
            ));
            curr_y -= 20;
        }

        // Summary Totals Box (Right aligned)
        let total_box_y = curr_y - 85;
        stream.push_str(&format!(
            "q 0.95 0.96 0.98 rg 350 {} 205 90 re f 0.85 0.86 0.90 RG 1 w 350 {} 205 90 re S Q\n",
            total_box_y, total_box_y
        ));
        stream.push_str(&format!(
            "BT /F2 9 Tf 0.3 0.3 0.3 rg 365 {} Td (Subtotal:) Tj ET\n",
            total_box_y + 68
        ));
        stream.push_str(&format!(
            "BT /F1 9 Tf 0.1 0.1 0.1 rg 485 {} Td (${:.2}) Tj ET\n",
            total_box_y + 68,
            invoice.subtotal
        ));

        stream.push_str(&format!(
            "BT /F2 9 Tf 0.3 0.3 0.3 rg 365 {} Td (Sales Tax ({}%):) Tj ET\n",
            total_box_y + 48,
            invoice.tax_rate
        ));
        stream.push_str(&format!(
            "BT /F1 9 Tf 0.1 0.1 0.1 rg 485 {} Td (${:.2}) Tj ET\n",
            total_box_y + 48,
            invoice.tax_amount
        ));

        stream.push_str(&format!(
            "q 0.80 0.82 0.88 RG 1 w 360 {} 185 0 re S Q\n",
            total_box_y + 36
        ));

        stream.push_str(&format!(
            "BT /F1 12 Tf 0.18 0.49 0.90 rg 365 {} Td (GRAND TOTAL:) Tj ET\n",
            total_box_y + 14
        ));
        stream.push_str(&format!(
            "BT /F1 13 Tf 0.10 0.65 0.35 rg 475 {} Td (${:.2}) Tj ET\n",
            total_box_y + 14,
            invoice.total_amount
        ));

        // Official Company Stamp / Seal (Left side)
        let stamp_y = total_box_y - 20;
        stream.push_str(&format!(
            "q 0.85 0.20 0.20 RG 2 w 120 {} 45 0 360 arc S Q\n",
            stamp_y
        ));
        stream.push_str(&format!(
            "q 0.85 0.20 0.20 RG 1 w 120 {} 42 0 360 arc S Q\n",
            stamp_y
        ));
        stream.push_str(&format!(
            "BT /F1 7 Tf 0.85 0.20 0.20 rg 88 {} Td (OFFICIAL COMPANY SEAL) Tj ET\n",
            stamp_y + 16
        ));
        stream.push_str(&format!(
            "BT /F1 8 Tf 0.85 0.20 0.20 rg 92 {} Td (VERIFIED & ISSUED) Tj ET\n",
            stamp_y
        ));
        stream.push_str(&format!(
            "BT /F2 6 Tf 0.85 0.20 0.20 rg 95 {} Td ({}) Tj ET\n",
            stamp_y - 14,
            escape_pdf(&company.trading_name)
        ));

        // Payment / Bank Wire Instructions
        stream.push_str(
            "q 0.95 0.96 0.98 rg 40 85 515 65 re f 0.85 0.86 0.90 RG 1 w 40 85 515 65 re S Q\n",
        );
        stream.push_str("BT /F1 9 Tf 0.18 0.49 0.90 rg 52 135 Td (WIRE TRANSFER & PAYMENT INSTRUCTIONS) Tj ET\n");
        stream.push_str(&format!(
            "BT /F2 8 Tf 0.25 0.25 0.30 rg 52 120 Td (Bank: {}  |  IBAN: {}) Tj ET\n",
            escape_pdf(&company.bank_name),
            escape_pdf(&company.bank_iban)
        ));
        stream.push_str(&format!(
            "BT /F2 8 Tf 0.25 0.25 0.30 rg 52 107 Td (SWIFT / BIC: {}  |  Beneficiary: {}) Tj ET\n",
            escape_pdf(&company.bank_swift),
            escape_pdf(&company.legal_name)
        ));
        stream.push_str(&format!("BT /F2 8 Tf 0.4 0.4 0.4 rg 52 94 Td (Please reference invoice number '{}' on transfer remittance advice.) Tj ET\n", escape_pdf(&invoice.invoice_number)));

        // Footer Note
        stream.push_str("BT /F2 8 Tf 0.5 0.5 0.6 rg 195 40 Td (Generated by Surf Accounting - Pure Rust High-Performance Ledger) Tj ET\n");

        assemble_pdf(&stream)
    }

    /// Renders an invoice image directly onto an RGB image buffer
    fn draw_invoice_canvas(
        img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        invoice: &Invoice,
        _company: &CompanyProfile,
    ) {
        let width = img.width();
        let height = img.height();

        // 1. Top Bar Accent
        for y in 0..30 {
            for x in 0..width {
                img.put_pixel(x, y, Rgba([46, 125, 230, 255]));
            }
        }

        // 2. White Sheet Card Margin
        let card_x = 40;
        let card_w = width - 80;
        let card_y = 60;
        let card_h = height - 120;

        // Card Border
        for x in card_x..card_x + card_w {
            img.put_pixel(x, card_y, Rgba([220, 225, 235, 255]));
            img.put_pixel(x, card_y + card_h, Rgba([220, 225, 235, 255]));
        }
        for y in card_y..card_y + card_h {
            img.put_pixel(card_x, y, Rgba([220, 225, 235, 255]));
            img.put_pixel(card_x + card_w, y, Rgba([220, 225, 235, 255]));
        }

        // 3. Draw Table Grid Lines & Boxes
        let table_y = 520;
        for x in 80..width - 80 {
            for y in table_y..table_y + 40 {
                img.put_pixel(x, y, Rgba([30, 35, 45, 255])); // Dark header row
            }
        }

        // Item Rows
        let row_height = 45;
        for (i, _) in invoice.items.iter().enumerate() {
            let row_y = table_y + 40 + (i as u32 * row_height);
            let fill_color = if i % 2 == 0 {
                Rgba([245, 247, 250, 255])
            } else {
                Rgba([255, 255, 255, 255])
            };
            for x in 80..width - 80 {
                for y in row_y..row_y + row_height {
                    if y < height - 200 {
                        img.put_pixel(x, y, fill_color);
                    }
                }
            }
        }

        // Draw Circular Official Stamp Seal (Red watermark ring)
        let center_x = 260;
        let center_y = height - 380;
        let radius = 100;
        draw_circle_ring(img, center_x, center_y, radius, 4, Rgba([210, 45, 45, 240]));
        draw_circle_ring(
            img,
            center_x,
            center_y,
            radius - 8,
            2,
            Rgba([210, 45, 45, 240]),
        );
    }
}

fn draw_circle_ring(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    cx: u32,
    cy: u32,
    r: u32,
    thickness: u32,
    color: Rgba<u8>,
) {
    let r_i32 = r as i32;
    let t_i32 = thickness as i32;
    for y in -r_i32..=r_i32 {
        for x in -r_i32..=r_i32 {
            let dist_sq = x * x + y * y;
            if dist_sq <= r_i32 * r_i32 && dist_sq >= (r_i32 - t_i32) * (r_i32 - t_i32) {
                let px = (cx as i32 + x) as u32;
                let py = (cy as i32 + y) as u32;
                if px < img.width() && py < img.height() {
                    img.put_pixel(px, py, color);
                }
            }
        }
    }
}

fn escape_pdf(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

/// Assembles standard PDF 1.4 file format structure
fn assemble_pdf(stream: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let stream_bytes = stream.as_bytes();
    let stream_len = stream_bytes.len();

    let mut offsets = Vec::new();

    // PDF Header
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    // Obj 1: Catalog
    offsets.push(out.len());
    out.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    // Obj 2: Pages
    offsets.push(out.len());
    out.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    // Obj 3: Page (A4: 595 x 842 pt)
    offsets.push(out.len());
    out.extend_from_slice(b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >> >>\nendobj\n");

    // Obj 4: Content Stream
    offsets.push(out.len());
    let stream_header = format!("4 0 obj\n<< /Length {} >>\nstream\n", stream_len);
    out.extend_from_slice(stream_header.as_bytes());
    out.extend_from_slice(stream_bytes);
    out.extend_from_slice(b"\nendstream\nendobj\n");

    // Obj 5: Font Helvetica-Bold
    offsets.push(out.len());
    out.extend_from_slice(
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>\nendobj\n",
    );

    // Obj 6: Font Helvetica
    offsets.push(out.len());
    out.extend_from_slice(
        b"6 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
    );

    // XRef Table
    let xref_offset = out.len();
    out.extend_from_slice(b"xref\n0 7\n");
    out.extend_from_slice(b"0000000000 65535 f \n");
    for off in offsets {
        out.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
    }

    // Trailer
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            xref_offset
        )
        .as_bytes(),
    );

    out
}
