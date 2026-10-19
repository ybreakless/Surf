use std::path::Path;

use uuid::Uuid;

use crate::db::repository::Repository;

/// Where exports live by default: exports/ under the working directory.
pub fn exports_dir() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_default().join("exports")
}

/// Strips everything but digits; wa.me links require international digits.
pub fn normalize_phone_digits(phone: &str) -> String {
    phone
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
}

/// Builds the WhatsApp share (deep-link) URL for a customer phone number.
/// Opening it opens WhatsApp with the message prefilled; the exported file
/// is then attached from the exports folder inside the WhatsApp app.
pub fn wa_me_link(phone: &str, message: &str) -> String {
    format!(
        "https://wa.me/{}?text={}",
        normalize_phone_digits(phone),
        percent_encode(message)
    )
}

pub fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Opens a file or URL with the platform default handler.
pub fn open_in_default_app(target: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .args([target])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to open '{}': {}", target, e))
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args([target])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to open '{}': {}", target, e))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", target])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to open '{}': {}", target, e))
    }
}

/// Builds a `multipart/form-data` body for the Telegram sendDocument call.
fn build_multipart_body(boundary: &str, chat_id: &str, filename: &str, data: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"chat_id\"\r\n\r\n");
    body.extend_from_slice(chat_id.trim().as_bytes());
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"document\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(data);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

/// Sends a document to a Telegram chat through a bot using the Bot API.
/// Requires `telegram_bot_token` and `telegram_chat_id` in app settings.
pub fn send_document_to_telegram(repo: &Repository, file_path: &Path) -> Result<(), String> {
    let token = repo
        .get_setting("telegram_bot_token")
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| {
            "Telegram bot is not configured — add the bot token in Settings.".to_string()
        })?;
    let chat_id = repo
        .get_setting("telegram_chat_id")
        .filter(|c| !c.trim().is_empty())
        .ok_or_else(|| "Telegram chat id is missing — add it in Settings.".to_string())?;

    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document")
        .replace(['"', '\\'], "_");
    let data =
        std::fs::read(file_path).map_err(|e| format!("Failed to read file for upload: {}", e))?;

    let boundary = format!("SurfBoundary{:032}", Uuid::new_v4().as_u128());
    let body = build_multipart_body(&boundary, &chat_id, &file_name, &data);

    let url = format!("https://api.telegram.org/bot{}/sendDocument", token.trim());
    let resp = ureq::post(&url)
        .timeout(std::time::Duration::from_secs(60))
        .set(
            "Content-Type",
            &format!("multipart/form-data; boundary={boundary}"),
        )
        .send_bytes(&body)
        .map_err(|e| format!("Telegram upload failed: {}", e))?;

    let resp_text = resp
        .into_string()
        .unwrap_or_else(|_| "\"ok\":false".to_string());
    if !resp_text.contains("\"ok\":true") {
        return Err(format!("Telegram rejected the upload: {}", resp_text));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wa_link_is_well_formed() {
        let link = wa_me_link(
            "+971 (50) 123-4567",
            "Hello, invoice INV-1 attached (AED 5,000.00).",
        );
        assert!(link.starts_with("https://wa.me/971501234567?text="));
        assert!(link.contains("Hello%2C%20invoice"));
    }

    #[test]
    fn digits_only() {
        assert_eq!(
            normalize_phone_digits("+98 912 345 67 89-x"),
            "989123456789"
        );
        assert_eq!(normalize_phone_digits(""), "");
    }

    #[test]
    fn encoding_round_trip() {
        let msg = "Invoice INV-X (Total $100.00)\nThanks!";
        let enc = percent_encode(msg);
        assert!(!enc.contains(' '));
        assert!(enc.contains("Invoice%20INV-X"));
        assert!(enc.contains("%0A"));
    }

    #[test]
    fn multipart_body_structure() {
        let body = build_multipart_body("BOUNDARY", "12345", "invoice.pdf", b"PDFDATA");
        let text = String::from_utf8(body).unwrap();
        assert!(text.contains("name=\"chat_id\"\r\n\r\n12345"));
        assert!(text.contains("filename=\"invoice.pdf\""));
        assert!(text.contains("PDFDATA"));
        assert!(text.ends_with("--BOUNDARY--\r\n"));
    }
}
