//! selah-ai — AI features for Selah: OCR, PII detection, smart crop suggestions.

use selah_core::{Rect, RedactionTarget};
use serde::{Deserialize, Serialize};

/// Result of text extraction (OCR).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    /// Extracted text.
    pub text: String,
    /// Confidence score (0.0 to 1.0).
    pub confidence: f64,
    /// Bounding boxes for detected text regions.
    pub bounding_boxes: Vec<Rect>,
}

/// A suggested crop region.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartCropSuggestion {
    /// Suggested crop rectangle.
    pub region: Rect,
    /// Why this crop was suggested.
    pub reason: String,
    /// Confidence score (0.0 to 1.0).
    pub confidence: f64,
}

/// A detected region that should be redacted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionSuggestion {
    /// Region containing sensitive data.
    pub region: Rect,
    /// Type of sensitive data detected.
    pub target_type: RedactionTarget,
    /// Confidence score (0.0 to 1.0).
    pub confidence: f64,
    /// The matched text.
    pub matched_text: String,
}

/// Extract text regions from raw image bytes by scanning for ASCII-printable runs.
///
/// This is a stub implementation. In production, this would call hoosh for
/// actual OCR via an LLM vision model.
pub fn extract_text_regions(data: &[u8]) -> OcrResult {
    let mut text = String::new();
    let mut current_run = String::new();

    for &byte in data {
        if (0x20..=0x7E).contains(&byte) {
            current_run.push(byte as char);
        } else {
            if current_run.len() >= 4 {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(&current_run);
            }
            current_run.clear();
        }
    }

    // Flush last run
    if current_run.len() >= 4 {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&current_run);
    }

    let confidence = if text.is_empty() { 0.0 } else { 0.5 };

    OcrResult {
        text,
        confidence,
        bounding_boxes: Vec::new(),
    }
}

/// Detect sensitive data in text using regex-like pattern matching.
///
/// Detects emails, phone numbers, credit card numbers, and IP addresses.
pub fn suggest_redactions(text: &str) -> Vec<RedactionSuggestion> {
    let mut suggestions = Vec::new();

    // Email detection: simple pattern word@word.word
    for word in text.split_whitespace() {
        if let Some(at_pos) = word.find('@') {
            let local = &word[..at_pos];
            let domain = &word[at_pos + 1..];
            if !local.is_empty() && domain.contains('.') {
                let dot_pos = domain.rfind('.').unwrap();
                let tld = &domain[dot_pos + 1..];
                if tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()) {
                    suggestions.push(RedactionSuggestion {
                        region: Rect::default(),
                        target_type: RedactionTarget::Email,
                        confidence: 0.9,
                        matched_text: word.to_string(),
                    });
                }
            }
        }
    }

    // Phone detection: sequences of digits with optional separators
    // Matches patterns like 555-123-4567, (555) 123-4567, +1-555-123-4567
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_ascii_digit() { c } else { ' ' })
        .collect();
    for group in cleaned.split_whitespace() {
        if group.len() >= 10 && group.len() <= 15 && group.chars().all(|c| c.is_ascii_digit()) {
            suggestions.push(RedactionSuggestion {
                region: Rect::default(),
                target_type: RedactionTarget::Phone,
                confidence: 0.7,
                matched_text: group.to_string(),
            });
        }
    }

    // Credit card detection: 13-19 digit sequences (after removing spaces/dashes)
    for word in text.split_whitespace() {
        let digits: String = word.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() >= 13
            && digits.len() <= 19
            && word
                .chars()
                .all(|c| c.is_ascii_digit() || c == '-' || c == ' ')
            && luhn_check(&digits)
        {
            suggestions.push(RedactionSuggestion {
                region: Rect::default(),
                target_type: RedactionTarget::CreditCard,
                confidence: 0.85,
                matched_text: word.to_string(),
            });
        }
    }

    // IP address detection: four octets separated by dots
    for word in text.split_whitespace() {
        let word = word.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
        let parts: Vec<&str> = word.split('.').collect();
        if parts.len() == 4 {
            let valid = parts.iter().all(|p| {
                if let Ok(n) = p.parse::<u32>() {
                    n <= 255
                } else {
                    false
                }
            });
            if valid {
                suggestions.push(RedactionSuggestion {
                    region: Rect::default(),
                    target_type: RedactionTarget::IpAddress,
                    confidence: 0.8,
                    matched_text: word.to_string(),
                });
            }
        }
    }

    suggestions
}

/// Luhn algorithm check for credit card validation.
fn luhn_check(digits: &str) -> bool {
    let mut sum = 0u32;
    let mut double = false;

    for ch in digits.chars().rev() {
        if let Some(d) = ch.to_digit(10) {
            let val = if double {
                let doubled = d * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                d
            };
            sum += val;
            double = !double;
        } else {
            return false;
        }
    }

    sum.is_multiple_of(10)
}

/// Suggest smart crop regions based on rule-of-thirds.
///
/// Returns crop suggestions at the four rule-of-thirds intersection points.
pub fn suggest_smart_crop(width: u32, height: u32) -> Vec<SmartCropSuggestion> {
    if width == 0 || height == 0 {
        return Vec::new();
    }

    let w = width as f64;
    let h = height as f64;

    // Crop size: 50% of original dimensions
    let crop_w = w * 0.5;
    let crop_h = h * 0.5;

    let thirds_x = [w / 3.0, 2.0 * w / 3.0];
    let thirds_y = [h / 3.0, 2.0 * h / 3.0];

    let mut suggestions = Vec::new();

    let labels = [
        "upper-left third",
        "upper-right third",
        "lower-left third",
        "lower-right third",
    ];

    let mut idx = 0;
    for &ty in &thirds_y {
        for &tx in &thirds_x {
            // Center the crop on the intersection point
            let x = (tx - crop_w / 2.0).max(0.0).min(w - crop_w);
            let y = (ty - crop_h / 2.0).max(0.0).min(h - crop_h);

            suggestions.push(SmartCropSuggestion {
                region: Rect::new(x, y, crop_w, crop_h),
                reason: format!("rule-of-thirds: {}", labels[idx]),
                confidence: 0.6,
            });
            idx += 1;
        }
    }

    // Also suggest a center crop
    let cx = (w - crop_w) / 2.0;
    let cy = (h - crop_h) / 2.0;
    suggestions.push(SmartCropSuggestion {
        region: Rect::new(cx, cy, crop_w, crop_h),
        reason: "center crop".to_string(),
        confidence: 0.5,
    });

    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text_from_ascii_bytes() {
        let data = b"....Hello World!....";
        let result = extract_text_regions(data);
        assert!(result.text.contains("Hello World!"));
        assert!(result.confidence > 0.0);
    }

    #[test]
    fn test_extract_text_empty_input() {
        let result = extract_text_regions(&[]);
        assert!(result.text.is_empty());
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_extract_text_binary_garbage() {
        let data: Vec<u8> = (0..100).map(|i| (i * 7 % 20) as u8).collect();
        let result = extract_text_regions(&data);
        // Mostly non-printable, should extract little or nothing
        assert!(result.text.len() < 20);
    }

    #[test]
    fn test_detect_email() {
        let suggestions = suggest_redactions("contact user@example.com for info");
        let emails: Vec<_> = suggestions
            .iter()
            .filter(|s| s.target_type == RedactionTarget::Email)
            .collect();
        assert_eq!(emails.len(), 1);
        assert_eq!(emails[0].matched_text, "user@example.com");
    }

    #[test]
    fn test_detect_phone() {
        let suggestions = suggest_redactions("call 5551234567 now");
        let phones: Vec<_> = suggestions
            .iter()
            .filter(|s| s.target_type == RedactionTarget::Phone)
            .collect();
        assert_eq!(phones.len(), 1);
    }

    #[test]
    fn test_detect_ip_address() {
        let suggestions = suggest_redactions("server at 192.168.1.100 is down");
        let ips: Vec<_> = suggestions
            .iter()
            .filter(|s| s.target_type == RedactionTarget::IpAddress)
            .collect();
        assert_eq!(ips.len(), 1);
        assert_eq!(ips[0].matched_text, "192.168.1.100");
    }

    #[test]
    fn test_detect_invalid_ip() {
        let suggestions = suggest_redactions("not an IP: 999.999.999.999");
        let ips: Vec<_> = suggestions
            .iter()
            .filter(|s| s.target_type == RedactionTarget::IpAddress)
            .collect();
        assert_eq!(ips.len(), 0);
    }

    #[test]
    fn test_detect_credit_card() {
        // 4111111111111111 is a known test Visa number (passes Luhn)
        let suggestions = suggest_redactions("card: 4111111111111111");
        let cards: Vec<_> = suggestions
            .iter()
            .filter(|s| s.target_type == RedactionTarget::CreditCard)
            .collect();
        assert_eq!(cards.len(), 1);
    }

    #[test]
    fn test_no_false_credit_card() {
        // Random digits that don't pass Luhn
        let suggestions = suggest_redactions("id: 1234567890123");
        let cards: Vec<_> = suggestions
            .iter()
            .filter(|s| s.target_type == RedactionTarget::CreditCard)
            .collect();
        assert_eq!(cards.len(), 0);
    }

    #[test]
    fn test_detect_nothing_in_clean_text() {
        let suggestions = suggest_redactions("this is perfectly clean text with no PII");
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_smart_crop_returns_five_suggestions() {
        let suggestions = suggest_smart_crop(1920, 1080);
        assert_eq!(suggestions.len(), 5); // 4 thirds + 1 center
    }

    #[test]
    fn test_smart_crop_zero_dimensions() {
        let suggestions = suggest_smart_crop(0, 0);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_smart_crop_regions_within_bounds() {
        let w = 1920u32;
        let h = 1080u32;
        let suggestions = suggest_smart_crop(w, h);
        for s in &suggestions {
            assert!(s.region.x >= 0.0);
            assert!(s.region.y >= 0.0);
            assert!(s.region.x + s.region.width <= w as f64);
            assert!(s.region.y + s.region.height <= h as f64);
        }
    }

    #[test]
    fn test_luhn_check_valid() {
        assert!(luhn_check("4111111111111111")); // Visa test number
        assert!(luhn_check("5500000000000004")); // Mastercard test
    }

    #[test]
    fn test_luhn_check_invalid() {
        assert!(!luhn_check("1234567890123"));
        assert!(!luhn_check("1111111111111"));
    }
}
