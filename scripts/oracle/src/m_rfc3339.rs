//! `rfc3339`: selah's timestamps (port plan §3 M1). A history entry's `timestamp` is chrono 0.4.45's
//! `DateTime<Utc>`, which serde writes as RFC 3339 with `Z` and the shortest of 0, 3, 6 or 9 fraction digits, and
//! reads through chrono's RFC 3339 parser. Rows:
//! - `DateTime::to_json i:<secs> i:<nanos> → s:<json, with quotes>`;
//! - `DateTime::from_json s:<json> → i:<secs> i:<nanos> | E:serde s:<message>`.

use crate::{Out, Row, Xs, emit_tsv, ti, ts};
use chrono::{DateTime, Utc};

pub fn emit(out: &mut Out) {
    let mut rows = Vec::new();
    let mut add = |f: &str, inputs: Vec<String>, outputs: Vec<String>| {
        rows.push(Row { func: f.into(), class: "A", inputs, outputs, note: String::new() });
    };
    let mut secs: Vec<i64> = vec![
        0, 1, -1, 59, 60, 3599, 3600, 86_399, 86_400, -86_400, 951_782_400, 951_868_799, 1_709_164_800, 4_102_444_800,
        4_107_542_400, 253_402_300_799, -62_135_596_800, -2_208_988_800, 1_763_112_000, 2_147_483_647, 2_147_483_648,
    ];
    let mut r = Xs(0xDA7E);
    for _ in 0..200 {
        secs.push((r.next() % 4_102_444_800) as i64);
    }
    let nanos: [u32; 13] = [0, 1, 10, 100, 1000, 999, 100_000, 1_000_000, 120_000_000, 123_456_789, 500_000_000, 999_999_999, 10_000_000];
    for (i, &s) in secs.iter().enumerate() {
        for (j, &n) in nanos.iter().enumerate() {
            if (i + j) % 3 != 0 && i > 20 {
                continue;
            }
            let dt: DateTime<Utc> = DateTime::from_timestamp(s, n).expect("in range");
            add("DateTime::to_json", vec![ti(s), ti(n.into())], vec![ts(&serde_json::to_string(&dt).expect("json"))]);
        }
    }
    let docs = [
        r#""2026-11-14T10:00:00Z""#,
        r#""2026-11-14T10:00:00.123Z""#,
        r#""2026-11-14T10:00:00.123456789Z""#,
        r#""2026-11-14T10:00:00.1Z""#,
        r#""2026-11-14T10:00:00.1234567891Z""#,
        r#""2026-11-14T10:00:00+00:00""#,
        r#""2026-11-14T10:00:00+01:00""#,
        r#""2026-11-14T10:00:00-05:30""#,
        r#""2026-11-14T10:00:00+0100""#,
        r#""2026-11-14t10:00:00z""#,
        r#""2026-11-14 10:00:00Z""#,
        r#""2026-11-14T10:00Z""#,
        r#""2026-11-14T10:00:00""#,
        r#""2026-11-14T23:59:60Z""#,
        r#""2023-02-29T00:00:00Z""#,
        r#""2024-02-29T00:00:00Z""#,
        r#""2026-13-01T00:00:00Z""#,
        r#""0000-01-01T00:00:00Z""#,
        r#""9999-12-31T23:59:59.999999999Z""#,
        r#""1969-12-31T23:59:59Z""#,
        r#""2026-11-14T10:00:00Z ""#,
        r#"" 2026-11-14T10:00:00Z""#,
        r#""2026-11-14T10:00:00.Z""#,
        r#""2026-11-14T24:00:00Z""#,
        r#""+2026-11-14T10:00:00Z""#,
        r#""2026-11-14T10:00:00+24:00""#,
        r#""""#,
        "1763112000",
        "null",
    ];
    for d in docs {
        let o = match serde_json::from_str::<DateTime<Utc>>(d) {
            Ok(dt) => vec![ti(dt.timestamp()), ti(dt.timestamp_subsec_nanos().into())],
            Err(e) => vec!["E:serde".into(), ts(&e.to_string())],
        };
        add("DateTime::from_json", vec![ts(d)], o);
    }
    emit_tsv(out, "rfc3339", "i:<unix seconds> i:<nanoseconds>", &rows);
}
