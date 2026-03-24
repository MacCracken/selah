//! AI features for Selah: OCR, PII detection, smart crop suggestions.

use crate::core::RedactionTarget;
use crate::geometry::Rect;
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
    /// Whether this result came from the stub (byte-scanning) implementation
    /// rather than a real OCR engine. When true, bounding boxes will be empty
    /// and the text is from embedded metadata, not visual content.
    #[serde(default)]
    pub is_stub: bool,
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
/// **WARNING: This is a stub implementation.** It scans the raw (compressed)
/// image bytes for ASCII-printable runs, which only finds embedded metadata
/// strings — *not* text rendered visually in the image. Confidence is capped
/// at 0.1 to reflect this limitation.
///
/// For real OCR, integrate with hoosh (LLM vision model on port 8088).
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

    // Low confidence: this is a stub, not real OCR.
    let confidence = if text.is_empty() { 0.0 } else { 0.1 };

    OcrResult {
        text,
        confidence,
        bounding_boxes: Vec::new(),
        is_stub: true,
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

    // Phone detection: look for digit sequences with phone-number separators.
    // Requires at least one separator (dash, dot, space, parens) to distinguish
    // from generic numeric IDs and timestamps.
    for word in text.split_whitespace() {
        // Only consider tokens that contain at least one phone separator
        let has_separator = word.chars().any(|c| c == '-' || c == '.' || c == '(' || c == ')');
        if !has_separator {
            continue;
        }
        let digits: String = word.chars().filter(|c| c.is_ascii_digit()).collect();
        let non_phone: bool = word
            .chars()
            .any(|c| !c.is_ascii_digit() && !"-.()+# ".contains(c));
        if non_phone {
            continue;
        }
        if digits.len() >= 10 && digits.len() <= 15 {
            suggestions.push(RedactionSuggestion {
                region: Rect::default(),
                target_type: RedactionTarget::Phone,
                confidence: 0.7,
                matched_text: word.to_string(),
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
pub fn luhn_check(digits: &str) -> bool {
    let mut sum = 0u32;
    let mut double = false;

    for ch in digits.chars().rev() {
        if let Some(d) = ch.to_digit(10) {
            let val = if double {
                let doubled = d * 2;
                if doubled > 9 {
                    doubled - 9
                } else {
                    doubled
                }
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

    let w = width as f32;
    let h = height as f32;

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
        assert!(result.is_stub);
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
    fn test_detect_phone_with_separator() {
        let suggestions = suggest_redactions("call 555-123-4567 now");
        let phones: Vec<_> = suggestions
            .iter()
            .filter(|s| s.target_type == RedactionTarget::Phone)
            .collect();
        assert_eq!(phones.len(), 1);
        assert_eq!(phones[0].matched_text, "555-123-4567");
    }

    #[test]
    fn test_no_phone_without_separator() {
        // Plain digit sequences should NOT match to avoid false positives on IDs/timestamps
        let suggestions = suggest_redactions("id 5551234567 here");
        let phones: Vec<_> = suggestions
            .iter()
            .filter(|s| s.target_type == RedactionTarget::Phone)
            .collect();
        assert_eq!(phones.len(), 0);
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
            assert!(s.region.x() >= 0.0);
            assert!(s.region.y() >= 0.0);
            assert!(s.region.x() + s.region.width() <= w as f32);
            assert!(s.region.y() + s.region.height() <= h as f32);
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

    #[test]
    fn test_email_no_tld() {
        let suggestions = suggest_redactions("bad email: user@localhost");
        let emails: Vec<_> = suggestions
            .iter()
            .filter(|s| s.target_type == RedactionTarget::Email)
            .collect();
        assert_eq!(emails.len(), 0);
    }

    #[test]
    fn test_email_short_tld() {
        // Single-char TLD should not match
        let suggestions = suggest_redactions("test user@example.x");
        let emails: Vec<_> = suggestions
            .iter()
            .filter(|s| s.target_type == RedactionTarget::Email)
            .collect();
        assert_eq!(emails.len(), 0);
    }

    #[test]
    fn test_email_numeric_tld() {
        // Numeric TLD should not match
        let suggestions = suggest_redactions("test user@example.123");
        let emails: Vec<_> = suggestions
            .iter()
            .filter(|s| s.target_type == RedactionTarget::Email)
            .collect();
        assert_eq!(emails.len(), 0);
    }

    #[test]
    fn test_credit_card_with_dashes() {
        let suggestions = suggest_redactions("card: 4111-1111-1111-1111");
        let cards: Vec<_> = suggestions
            .iter()
            .filter(|s| s.target_type == RedactionTarget::CreditCard)
            .collect();
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].matched_text, "4111-1111-1111-1111");
    }

    #[test]
    fn test_ip_zero() {
        let suggestions = suggest_redactions("addr 0.0.0.0 here");
        let ips: Vec<_> = suggestions
            .iter()
            .filter(|s| s.target_type == RedactionTarget::IpAddress)
            .collect();
        assert_eq!(ips.len(), 1);
        assert_eq!(ips[0].matched_text, "0.0.0.0");
    }

    #[test]
    fn test_ip_broadcast() {
        let suggestions = suggest_redactions("addr 255.255.255.255 here");
        let ips: Vec<_> = suggestions
            .iter()
            .filter(|s| s.target_type == RedactionTarget::IpAddress)
            .collect();
        assert_eq!(ips.len(), 1);
    }

    #[test]
    fn test_ip_partial_invalid() {
        // Three octets should not match
        let suggestions = suggest_redactions("not ip: 192.168.1");
        let ips: Vec<_> = suggestions
            .iter()
            .filter(|s| s.target_type == RedactionTarget::IpAddress)
            .collect();
        assert_eq!(ips.len(), 0);
    }

    #[test]
    fn test_multiple_pii_types() {
        let text = "email user@test.com phone 555-123-4567 ip 10.0.0.1";
        let suggestions = suggest_redactions(text);
        let types: Vec<_> = suggestions.iter().map(|s| &s.target_type).collect();
        assert!(types.contains(&&RedactionTarget::Email));
        assert!(types.contains(&&RedactionTarget::Phone));
        assert!(types.contains(&&RedactionTarget::IpAddress));
    }

    #[test]
    fn test_luhn_check_non_digit() {
        assert!(!luhn_check("4111abcd11111111"));
    }

    #[test]
    fn test_smart_crop_single_pixel() {
        let suggestions = suggest_smart_crop(1, 1);
        assert_eq!(suggestions.len(), 5);
    }
}
