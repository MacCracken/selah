//! `geometry`: `selah::Rect` (port plan §3 M1).
//!
//! `Rect` keeps hisab's `{min, max}` f32 layout, so every row that returns a rect gives its four layout words
//! (`min.x min.y max.x max.y`, read through `as_hisab`), and the port must match them bit for bit. Rows:
//! - `Rect::new g:x g:y g:w g:h → g:min.x g:min.y g:max.x g:max.y`;
//! - `Rect::x|y|width|height|area g×4 → g`, `Rect::center g×4 → g g`, `Rect::is_finite g×4 → b`;
//! - `Rect::normalized g×4 → g×4` (layout);
//! - `Rect::contains_point g×4 g:px g:py → b`;
//! - `Rect::intersects g×4 g×4 → b`;
//! - `Rect::to_json g×4 → s:<serde_json>`;
//! - `Rect::from_json s:<json> → g×4` (layout), or `E:serde s:<message>`.
//!
//! The inputs to every rect row are `Rect::new`'s four arguments. NaN outputs compare as "any NaN" (README).

use crate::{Out, Row, Xs, emit_tsv, tb, tg, ts};
use selah::Rect;

const V: [f32; 22] = [
    0.0, -0.0, 1.0, -1.0, 0.1, -0.7, 0.5, 2.5, 100.25, 1920.0, 1080.0, 33.3, 16_777_216.0, -16_777_216.0, 1e-45,
    1.175_494_4e-38, f32::MAX, f32::MIN, f32::INFINITY, f32::NEG_INFINITY, f32::NAN, 3.5,
];

fn rect_args(r: &mut Xs) -> [f32; 4] {
    let mut a = [0f32; 4];
    for v in &mut a {
        *v = V[(r.next() % V.len() as u64) as usize];
    }
    a
}

fn layout(rc: &Rect) -> Vec<String> {
    let h = rc.as_hisab();
    vec![tg(h.min.x), tg(h.min.y), tg(h.max.x), tg(h.max.y)]
}

fn args_tok(a: &[f32; 4]) -> Vec<String> {
    a.iter().map(|&x| tg(x)).collect()
}

pub fn emit(out: &mut Out) {
    let mut rows = Vec::new();
    let mut add = |f: &str, inputs: Vec<String>, outputs: Vec<String>| {
        rows.push(Row { func: f.into(), class: "A", inputs, outputs, note: String::new() });
    };
    let mut rects: Vec<[f32; 4]> = vec![
        [0.0, 0.0, 0.0, 0.0],
        [10.0, 20.0, 100.0, 50.0],
        [100.0, 50.0, -40.0, -30.0],
        [0.1, 0.2, 0.3, -0.7],
        [16_777_216.0, 0.0, 1.0, 3.0],
        [-0.0, 0.0, 0.0, -0.0],
        [0.0, -0.0, -0.0, 0.0],
        [f32::MAX, 0.0, f32::MAX, 1.0],
        [5.0, 5.0, f32::NAN, 1.0],
        [5.0, 5.0, f32::INFINITY, f32::NEG_INFINITY],
    ];
    let mut r = Xs(0x6E0_3E7);
    for _ in 0..600 {
        rects.push(rect_args(&mut r));
    }
    for a in &rects {
        let rc = Rect::new(a[0], a[1], a[2], a[3]);
        add("Rect::new", args_tok(a), layout(&rc));
        add("Rect::x", args_tok(a), vec![tg(rc.x())]);
        add("Rect::y", args_tok(a), vec![tg(rc.y())]);
        add("Rect::width", args_tok(a), vec![tg(rc.width())]);
        add("Rect::height", args_tok(a), vec![tg(rc.height())]);
        add("Rect::area", args_tok(a), vec![tg(rc.area())]);
        let c = rc.center();
        add("Rect::center", args_tok(a), vec![tg(c.x), tg(c.y)]);
        add("Rect::is_finite", args_tok(a), vec![tb(rc.is_finite())]);
        add("Rect::normalized", args_tok(a), layout(&rc.normalized()));
        let j = serde_json::to_string(&rc).expect("rect json");
        add("Rect::to_json", args_tok(a), vec![ts(&j)]);
    }
    // contains_point: the grid's rects against points on, inside, outside and at the edges.
    let mut r = Xs(0xC0_47A1);
    for i in 0..1200 {
        let a = rects[i % rects.len()];
        let rc = Rect::new(a[0], a[1], a[2], a[3]);
        let (px, py) = match i % 4 {
            0 => (V[(r.next() % 22) as usize], V[(r.next() % 22) as usize]),
            1 => (rc.x(), rc.y()),
            2 => (rc.x() + rc.width(), rc.y() + rc.height()),
            _ => (rc.center().x, rc.center().y),
        };
        let mut ins = args_tok(&a);
        ins.push(tg(px));
        ins.push(tg(py));
        add("Rect::contains_point", ins, vec![tb(rc.contains_point(selah::Vec2::new(px, py)))]);
    }
    // intersects: random pairs, touching edges, nested, negative sizes.
    let mut pairs: Vec<([f32; 4], [f32; 4])> = vec![
        ([0.0, 0.0, 10.0, 10.0], [10.0, 0.0, 10.0, 10.0]),
        ([0.0, 0.0, 10.0, 10.0], [9.999, 0.0, 10.0, 10.0]),
        ([0.0, 0.0, 10.0, 10.0], [2.0, 2.0, 2.0, 2.0]),
        ([10.0, 10.0, -10.0, -10.0], [5.0, 5.0, 1.0, 1.0]),
        ([0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0]),
    ];
    let mut r = Xs(0x1E_75EC);
    for _ in 0..800 {
        pairs.push((rect_args(&mut r), rect_args(&mut r)));
    }
    for (a, b) in &pairs {
        let mut ins = args_tok(a);
        ins.extend(args_tok(b));
        add(
            "Rect::intersects",
            ins,
            vec![tb(Rect::new(a[0], a[1], a[2], a[3]).intersects(&Rect::new(b[0], b[1], b[2], b[3])))],
        );
    }
    // from_json: what serde accepts and refuses.
    let docs = [
        r#"{"x":10,"y":20,"width":100,"height":50}"#,
        r#"{"x":10.5,"y":-20.25,"width":-100,"height":0}"#,
        r#"{"height":4,"width":3,"y":2,"x":1}"#,
        r#"{"x":1,"y":2,"width":3,"height":4,"extra":"ignored"}"#,
        r#" { "x" : 1 , "y" : 2 , "width" : 3 , "height" : 4 } "#,
        r#"{"x":0.1,"y":0.2,"width":0.3,"height":0.7}"#,
        r#"{"x":1e2,"y":1E-2,"width":-0,"height":0.0}"#,
        r#"{"x":16777217,"y":0,"width":0.1,"height":0}"#,
        r#"{"x":3.4028235e38,"y":0,"width":0,"height":0}"#,
        r#"{"x":3.4028236e38,"y":0,"width":0,"height":0}"#,
        r#"{"x":1e39,"y":0,"width":0,"height":0}"#,
        r#"{"x":1e-50,"y":0,"width":0,"height":0}"#,
        r#"{"x":1.0000000596046448,"y":0,"width":0,"height":0}"#,
        r#"{"x":1,"y":2,"width":3}"#,
        r#"{"x":"1","y":2,"width":3,"height":4}"#,
        r#"{"x":null,"y":2,"width":3,"height":4}"#,
        r#"{"x":1,"y":2,"width":3,"height":4,"x":5}"#,
        r#"[1,2,3,4]"#,
        r#"{}"#,
        r#"null"#,
        r#"{"x":1,"y":2,"width":3,"height":4"#,
        r#"{"x":NaN,"y":2,"width":3,"height":4}"#,
        r#"{"x":1,"y":2,"width":3e38,"height":4}"#,
        r#"{"x":3e38,"y":2,"width":3e38,"height":4}"#,
    ];
    for d in docs {
        match serde_json::from_str::<Rect>(d) {
            Ok(rc) => add("Rect::from_json", vec![ts(d)], layout(&rc)),
            Err(e) => add("Rect::from_json", vec![ts(d)], vec!["E:serde".into(), ts(&e.to_string())]),
        }
    }
    emit_tsv(out, "geometry", "rect inputs are Rect::new's (x, y, width, height); rect outputs its layout (min.x, min.y, max.x, max.y)", &rows);
}
