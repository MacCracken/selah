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

/// Minimum length of a printable run kept by the stub OCR.
const MIN_TEXT_RUN: usize = 4;

/// Extract text regions from raw image bytes by scanning for ASCII-printable runs.
///
/// **WARNING: This is a stub implementation.** It scans the raw (compressed)
/// image bytes for ASCII-printable runs, which only finds embedded metadata
/// strings — *not* text rendered visually in the image. Confidence is capped
/// at 0.1 to reflect this limitation.
///
/// For real OCR, use [`HooshClient::ocr`](crate::HooshClient::ocr) (LLM vision
/// model via hoosh).
#[must_use]
pub fn extract_text_regions(data: &[u8]) -> OcrResult {
    let mut text = String::new();

    for run in data
        .split(|b| !(0x20..=0x7E).contains(b))
        .filter(|run| run.len() >= MIN_TEXT_RUN)
    {
        // Every byte is printable ASCII, so this never fails.
        if let Ok(run) = std::str::from_utf8(run) {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(run);
        }
    }

    // Low confidence: this is a stub, not real OCR.
    let confidence = if text.is_empty() { 0.0 } else { 0.1 };
    tracing::debug!(
        input_bytes = data.len(),
        text_bytes = text.len(),
        "stub OCR extracted text"
    );

    OcrResult {
        text,
        confidence,
        bounding_boxes: Vec::new(),
        is_stub: true,
    }
}

/// Sentence punctuation that may trail the final group of a number.
const TRAILING_PUNCT: &[char] = &['.', ',', ';', ':', '!', '?', ')', ']', '"', '\''];

/// Detect sensitive data in text using pattern matching.
///
/// Detects emails, phone numbers, credit card numbers (Luhn-validated,
/// including space-grouped forms like `4111 1111 1111 1111`) and IPv4
/// addresses. Surrounding punctuation is ignored, and each token is reported
/// at most once (priority: email, credit card, IP address, phone). Results
/// are returned in text order.
///
/// # Example
///
/// ```
/// let suggestions = selah::suggest_redactions("contact user@example.com for info");
/// assert_eq!(suggestions.len(), 1);
/// assert_eq!(suggestions[0].matched_text, "user@example.com");
/// ```
#[must_use]
pub fn suggest_redactions(text: &str) -> Vec<RedactionSuggestion> {
    let mut suggestions = Vec::new();
    let mut tokens = text.split_whitespace();

    while let Some(token) = tokens.next() {
        // Multi-token patterns first; on success they consume following tokens.
        let multi = match_grouped_card(text, token, tokens.clone())
            .map(|(m, n)| (RedactionTarget::CreditCard, m, n, 0.85))
            .or_else(|| {
                match_split_phone(text, token, tokens.clone())
                    .map(|(m, n)| (RedactionTarget::Phone, m, n, 0.7))
            });
        if let Some((target_type, matched, consumed, confidence)) = multi {
            suggestions.push(suggestion(target_type, matched, confidence));
            for _ in 0..consumed {
                tokens.next();
            }
            continue;
        }

        if let Some((target_type, matched, confidence)) = classify_token(token) {
            suggestions.push(suggestion(target_type, matched, confidence));
        }
    }

    suggestions
}

#[inline]
fn suggestion(target_type: RedactionTarget, matched: &str, confidence: f64) -> RedactionSuggestion {
    RedactionSuggestion {
        region: Rect::default(),
        target_type,
        confidence,
        matched_text: matched.to_string(),
    }
}

/// Classify a single whitespace-delimited token.
fn classify_token(token: &str) -> Option<(RedactionTarget, &str, f64)> {
    if token.contains('@') {
        // Strip wrappers/trailing punctuation: `<a@b.co>`, `(a@b.co)`, `a@b.co.`
        let t = token.trim_matches(|c: char| !c.is_alphanumeric());
        if is_email(t) {
            return Some((RedactionTarget::Email, t, 0.9));
        }
    }

    let card = token.trim_matches(|c: char| !c.is_ascii_digit());
    if is_credit_card(card) {
        return Some((RedactionTarget::CreditCard, card, 0.85));
    }

    let ip = token
        .trim_matches(|c: char| !c.is_ascii_digit() && c != '.')
        .trim_matches('.');
    if is_ipv4(ip) {
        return Some((RedactionTarget::IpAddress, ip, 0.8));
    }

    let phone = token
        .trim_start_matches(|c: char| !(c.is_ascii_digit() || c == '+' || c == '('))
        .trim_end_matches(|c: char| !c.is_ascii_digit());
    if is_phone(phone) {
        return Some((RedactionTarget::Phone, phone, 0.7));
    }

    None
}

/// `local@domain.tld` with a non-empty local part and an alphabetic TLD of
/// at least two characters.
fn is_email(t: &str) -> bool {
    let Some((local, domain)) = t.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain
            .rsplit_once('.')
            .is_some_and(|(_, tld)| tld.len() >= 2 && tld.bytes().all(|b| b.is_ascii_alphabetic()))
}

/// 13–19 digits, optionally dash-separated, passing the Luhn check.
fn is_credit_card(t: &str) -> bool {
    let mut digits = [0u8; 19];
    let mut n = 0;
    for b in t.bytes() {
        match b {
            b'0'..=b'9' => {
                if n == digits.len() {
                    return false;
                }
                digits[n] = b;
                n += 1;
            }
            b'-' => {}
            _ => return false,
        }
    }
    n >= 13 && luhn_check(&digits[..n])
}

/// Dotted-quad IPv4 address with octets in 0–255.
fn is_ipv4(t: &str) -> bool {
    let mut parts = 0;
    for part in t.split('.') {
        parts += 1;
        if parts > 4
            || part.is_empty()
            || part.len() > 3
            || !part.bytes().all(|b| b.is_ascii_digit())
            || part.parse::<u16>().map_or(true, |n| n > 255)
        {
            return false;
        }
    }
    parts == 4
}

/// 10–15 digits containing at least one phone separator (`-`, `.`, `(`, `)`).
///
/// Requiring a separator distinguishes phone numbers from generic numeric
/// IDs and timestamps.
fn is_phone(t: &str) -> bool {
    let mut digits = 0;
    let mut has_separator = false;
    for b in t.bytes() {
        match b {
            b'0'..=b'9' => digits += 1,
            b'-' | b'.' | b'(' | b')' => has_separator = true,
            b'+' | b'#' => {}
            _ => return false,
        }
    }
    has_separator && (10..=15).contains(&digits)
}

/// Return the slice of `text` spanning from the start of `first` to the end
/// of `last` (both must be subslices of `text`), preserving the original
/// separators between them.
fn span<'a>(text: &'a str, first: &'a str, last: &'a str) -> &'a str {
    let base = text.as_ptr() as usize;
    let start = (first.as_ptr() as usize).wrapping_sub(base);
    let end = (last.as_ptr() as usize)
        .wrapping_sub(base)
        .saturating_add(last.len());
    text.get(start..end).unwrap_or(first)
}

/// Match a space-grouped card number such as `4111 1111 1111 1111` or
/// `3782 822463 10005`: a 4-digit first group followed by 3–6 digit groups,
/// 13–19 digits in total, passing Luhn. Returns the matched span and the
/// number of *additional* tokens consumed. The longest valid match wins.
fn match_grouped_card<'a>(
    text: &'a str,
    first: &'a str,
    rest: std::str::SplitWhitespace<'a>,
) -> Option<(&'a str, usize)> {
    if first.len() != 4 || !first.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut digits = [0u8; 19];
    digits[..4].copy_from_slice(first.as_bytes());
    let mut n = 4;
    let mut groups = 1;
    let mut best = None;

    for token in rest.take(4) {
        let group = token.trim_end_matches(TRAILING_PUNCT);
        let ends_sentence = group.len() != token.len();
        if !(3..=6).contains(&group.len())
            || !group.bytes().all(|b| b.is_ascii_digit())
            || n + group.len() > digits.len()
        {
            break;
        }
        digits[n..n + group.len()].copy_from_slice(group.as_bytes());
        n += group.len();
        groups += 1;
        if groups >= 3 && n >= 13 && luhn_check(&digits[..n]) {
            best = Some((span(text, first, group), groups - 1));
        }
        if ends_sentence {
            break;
        }
    }
    best
}

/// Match a US-style phone number split across two tokens: `(555) 123-4567`.
fn match_split_phone<'a>(
    text: &'a str,
    first: &'a str,
    mut rest: std::str::SplitWhitespace<'a>,
) -> Option<(&'a str, usize)> {
    let area = first.trim_start_matches(|c: char| c != '(');
    let b = area.as_bytes();
    if b.len() != 5 || b[0] != b'(' || b[4] != b')' || !b[1..4].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let local = rest.next()?.trim_end_matches(TRAILING_PUNCT);
    let l = local.as_bytes();
    let ok = l.len() == 8
        && matches!(l[3], b'-' | b'.')
        && l[..3].iter().chain(&l[4..]).all(u8::is_ascii_digit);
    ok.then(|| (span(text, area, local), 1))
}

/// Luhn checksum over ASCII digit bytes (credit card validation).
fn luhn_check(digits: &[u8]) -> bool {
    let mut sum = 0u32;
    let mut double = false;

    for &b in digits.iter().rev() {
        if !b.is_ascii_digit() {
            return false;
        }
        let d = u32::from(b - b'0');
        sum += if double {
            let doubled = d * 2;
            if doubled > 9 { doubled - 9 } else { doubled }
        } else {
            d
        };
        double = !double;
    }

    sum.is_multiple_of(10)
}

/// Suggest smart crop regions based on rule-of-thirds.
///
/// Returns crop suggestions at the four rule-of-thirds intersection points,
/// plus a center crop.
#[must_use]
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

    let labels = [
        "upper-left third",
        "upper-right third",
        "lower-left third",
        "lower-right third",
    ];
    let points = thirds_y
        .iter()
        .flat_map(|&ty| thirds_x.iter().map(move |&tx| (tx, ty)));

    let mut suggestions = Vec::with_capacity(labels.len() + 1);
    for ((tx, ty), label) in points.zip(labels) {
        // Center the crop on the intersection point, keeping it inside the image
        let x = (tx - crop_w / 2.0).clamp(0.0, w - crop_w);
        let y = (ty - crop_h / 2.0).clamp(0.0, h - crop_h);

        suggestions.push(SmartCropSuggestion {
            region: Rect::new(x, y, crop_w, crop_h),
            reason: format!("rule-of-thirds: {label}"),
            confidence: 0.6,
        });
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
        assert!(luhn_check(b"4111111111111111")); // Visa test number
        assert!(luhn_check(b"5500000000000004")); // Mastercard test
    }

    #[test]
    fn test_luhn_check_invalid() {
        assert!(!luhn_check(b"1234567890123"));
        assert!(!luhn_check(b"1111111111111"));
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
        assert!(!luhn_check(b"4111abcd11111111"));
    }

    #[test]
    fn test_smart_crop_single_pixel() {
        let suggestions = suggest_smart_crop(1, 1);
        assert_eq!(suggestions.len(), 5);
    }

    fn of_type(text: &str, t: RedactionTarget) -> Vec<String> {
        suggest_redactions(text)
            .into_iter()
            .filter(|s| s.target_type == t)
            .map(|s| s.matched_text)
            .collect()
    }

    #[test]
    fn test_email_with_surrounding_punctuation() {
        assert_eq!(
            of_type("mail me at user@example.com.", RedactionTarget::Email),
            ["user@example.com"]
        );
        assert_eq!(
            of_type("<admin@corp.io>, (ops@corp.io)", RedactionTarget::Email),
            ["admin@corp.io", "ops@corp.io"]
        );
    }

    #[test]
    fn test_ip_at_end_of_sentence() {
        assert_eq!(
            of_type("server is 10.0.0.1.", RedactionTarget::IpAddress),
            ["10.0.0.1"]
        );
        assert_eq!(
            of_type("host=192.168.0.7,", RedactionTarget::IpAddress),
            ["192.168.0.7"]
        );
    }

    #[test]
    fn test_ip_not_double_reported_as_phone() {
        let s = suggest_redactions("ip 192.168.100.200");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].target_type, RedactionTarget::IpAddress);
    }

    #[test]
    fn test_ip_rejects_extra_octets_and_long_parts() {
        assert!(of_type("1.2.3.4.5", RedactionTarget::IpAddress).is_empty());
        assert!(of_type("1.2.3.0004", RedactionTarget::IpAddress).is_empty());
    }

    #[test]
    fn test_credit_card_space_grouped() {
        assert_eq!(
            of_type(
                "card 4111 1111 1111 1111 exp 12/30",
                RedactionTarget::CreditCard
            ),
            ["4111 1111 1111 1111"]
        );
        // Amex 4-6-5 grouping, trailing period
        assert_eq!(
            of_type("amex 3782 822463 10005.", RedactionTarget::CreditCard),
            ["3782 822463 10005"]
        );
    }

    #[test]
    fn test_credit_card_grouped_failing_luhn_not_matched() {
        assert!(of_type("1234 5678 9012 3456", RedactionTarget::CreditCard).is_empty());
        // Plain 4-digit years are not cards
        assert!(of_type("2020 2021", RedactionTarget::CreditCard).is_empty());
    }

    #[test]
    fn test_credit_card_with_trailing_punctuation() {
        assert_eq!(
            of_type("card:4111111111111111.", RedactionTarget::CreditCard),
            ["4111111111111111"]
        );
    }

    #[test]
    fn test_amex_dashed_is_card_not_phone() {
        let s = suggest_redactions("3782-822463-10005");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].target_type, RedactionTarget::CreditCard);
    }

    #[test]
    fn test_phone_split_area_code() {
        assert_eq!(
            of_type("call (555) 123-4567 today", RedactionTarget::Phone),
            ["(555) 123-4567"]
        );
    }

    #[test]
    fn test_phone_with_trailing_punctuation() {
        assert_eq!(
            of_type("call 555-123-4567.", RedactionTarget::Phone),
            ["555-123-4567"]
        );
    }

    #[test]
    fn test_suggestions_in_text_order() {
        let s = suggest_redactions("10.0.0.1 then a@b.co then 555-123-4567");
        let kinds: Vec<_> = s.iter().map(|s| s.target_type.clone()).collect();
        assert_eq!(
            kinds,
            [
                RedactionTarget::IpAddress,
                RedactionTarget::Email,
                RedactionTarget::Phone
            ]
        );
    }

    #[test]
    fn test_suggest_redactions_unicode_no_panic() {
        // Multibyte chars around and inside tokens must not panic
        let _ =
            suggest_redactions("ünïcödé@例え.jp (５５５) ４１１１ — 🎉 user@例子.公司 «a@b.cd»");
        assert_eq!(of_type("«a@b.cd»", RedactionTarget::Email), ["a@b.cd"]);
    }

    #[test]
    fn test_extract_text_preserves_runs_and_separators() {
        let result = extract_text_regions(b"\x00abcd\x01\x02efgh\x00ab\x00ijkl");
        assert_eq!(result.text, "abcd efgh ijkl");
    }
}
