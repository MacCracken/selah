//! `error`: `SelahError`'s Display (port plan §3 M1). Row: `SelahError::fmt E:<variant> s:<message> → s:<text>`.

use crate::m_core::err_parts;
use crate::{Out, Row, emit_tsv, ts};
use selah::SelahError;

pub fn emit(out: &mut Out) {
    let mut rows = Vec::new();
    let mut errs: Vec<SelahError> = Vec::new();
    for m in ["x", "", "a: b", "région \"q\"\n2", "timed out after 30s"] {
        errs.push(SelahError::CaptureFailed(m.into()));
        errs.push(SelahError::InvalidRegion(m.into()));
        errs.push(SelahError::UnsupportedFormat(m.into()));
        errs.push(SelahError::AnnotationError(m.into()));
        errs.push(SelahError::Api(m.into()));
        errs.push(SelahError::Io(std::io::Error::other(m)));
    }
    for e in &errs {
        let (v, m) = err_parts(e);
        rows.push(Row {
            func: "SelahError::fmt".into(),
            class: "A",
            inputs: vec![format!("E:{v}"), ts(&m)],
            outputs: vec![ts(&e.to_string())],
            note: String::new(),
        });
    }
    emit_tsv(out, "error", "E:<variant> s:<message> → s:<Display>", &rows);
}
