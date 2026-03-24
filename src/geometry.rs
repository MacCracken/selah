//! Geometry types backed by hisab.

use hisab::Vec2;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A rectangle defined by position and size, backed by hisab::Rect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect(hisab::Rect);

impl Rect {
    /// Create a new rectangle from position and size.
    ///
    /// # Example
    ///
    /// ```
    /// use selah::Rect;
    /// use hisab::Vec2;
    ///
    /// let r = Rect::new(10.0, 20.0, 100.0, 50.0);
    /// assert_eq!(r.x(), 10.0);
    /// assert_eq!(r.width(), 100.0);
    /// assert!(r.contains_point(Vec2::new(50.0, 40.0)));
    /// ```
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        // Use direct field construction to preserve negative width/height (for arrows)
        Self(hisab::Rect {
            min: Vec2::new(x, y),
            max: Vec2::new(x + width, y + height),
        })
    }

    pub fn x(&self) -> f32 {
        self.0.min.x
    }
    pub fn y(&self) -> f32 {
        self.0.min.y
    }
    pub fn width(&self) -> f32 {
        self.0.max.x - self.0.min.x
    }
    pub fn height(&self) -> f32 {
        self.0.max.y - self.0.min.y
    }
    pub fn area(&self) -> f32 {
        self.width().abs() * self.height().abs()
    }
    pub fn center(&self) -> Vec2 {
        self.0.center()
    }

    pub fn contains_point(&self, p: Vec2) -> bool {
        p.x >= self.x()
            && p.x <= self.x() + self.width()
            && p.y >= self.y()
            && p.y <= self.y() + self.height()
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x() < other.x() + other.width()
            && self.x() + self.width() > other.x()
            && self.y() < other.y() + other.height()
            && self.y() + self.height() > other.y()
    }

    pub fn as_hisab(&self) -> &hisab::Rect {
        &self.0
    }
}

impl Default for Rect {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }
}

// Custom serde: serialize as {x, y, width, height} for backward compatibility
impl Serialize for Rect {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("Rect", 4)?;
        s.serialize_field("x", &self.x())?;
        s.serialize_field("y", &self.y())?;
        s.serialize_field("width", &self.width())?;
        s.serialize_field("height", &self.height())?;
        s.end()
    }
}

impl<'de> Deserialize<'de> for Rect {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct RectRepr {
            x: f64,
            y: f64,
            width: f64,
            height: f64,
        }
        let r = RectRepr::deserialize(deserializer)?;
        Ok(Rect::new(
            r.x as f32,
            r.y as f32,
            r.width as f32,
            r.height as f32,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_new_and_accessors() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(r.x(), 10.0);
        assert_eq!(r.y(), 20.0);
        assert_eq!(r.width(), 100.0);
        assert_eq!(r.height(), 50.0);
    }

    #[test]
    fn test_rect_area() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0);
        assert_eq!(r.area(), 5000.0);
    }

    #[test]
    fn test_rect_center() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0);
        let c = r.center();
        assert_eq!(c.x, 50.0);
        assert_eq!(c.y, 25.0);
    }

    #[test]
    fn test_rect_contains_point() {
        let r = Rect::new(10.0, 10.0, 100.0, 50.0);
        assert!(r.contains_point(Vec2::new(50.0, 30.0)));
        assert!(r.contains_point(Vec2::new(10.0, 10.0))); // on edge
        assert!(r.contains_point(Vec2::new(110.0, 60.0))); // on far edge
        assert!(!r.contains_point(Vec2::new(5.0, 30.0))); // outside left
        assert!(!r.contains_point(Vec2::new(50.0, 65.0))); // outside bottom
    }

    #[test]
    fn test_rect_intersects() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, 50.0, 100.0, 100.0);
        let c = Rect::new(200.0, 200.0, 50.0, 50.0);
        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn test_rect_default_is_zero() {
        let r = Rect::default();
        assert_eq!(r.x(), 0.0);
        assert_eq!(r.area(), 0.0);
    }

    #[test]
    fn test_rect_serde_roundtrip() {
        let r = Rect::new(10.5, 20.5, 100.0, 50.0);
        let json = serde_json::to_string(&r).unwrap();
        let r2: Rect = serde_json::from_str(&json).unwrap();
        assert!((r.x() - r2.x()).abs() < 0.01);
        assert!((r.y() - r2.y()).abs() < 0.01);
        assert!((r.width() - r2.width()).abs() < 0.01);
        assert!((r.height() - r2.height()).abs() < 0.01);
    }

    #[test]
    fn test_rect_serde_negative_width_arrow() {
        // Arrows use negative width/height to indicate direction
        let r = Rect::new(100.0, 100.0, -50.0, -30.0);
        let json = serde_json::to_string(&r).unwrap();
        let r2: Rect = serde_json::from_str(&json).unwrap();
        assert!((r.width() - r2.width()).abs() < 0.01);
        assert!((r.height() - r2.height()).abs() < 0.01);
    }

    #[test]
    fn test_rect_serde_from_f64_json() {
        // Backward compat: deserialize from f64 JSON (old format)
        let json = r#"{"x":10.123456789,"y":20.987654321,"width":100.5,"height":50.25}"#;
        let r: Rect = serde_json::from_str(json).unwrap();
        assert!((r.x() - 10.1234).abs() < 0.01);
        assert!((r.width() - 100.5).abs() < 0.01);
    }

    #[test]
    fn test_rect_intersects_touching_edges() {
        // Rects that share an edge but don't overlap
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(100.0, 0.0, 100.0, 100.0);
        // x=100 == x=100+0 is not < so no intersection
        assert!(!a.intersects(&b));
    }

    #[test]
    fn test_rect_intersects_self() {
        let a = Rect::new(10.0, 10.0, 50.0, 50.0);
        assert!(a.intersects(&a));
    }

    #[test]
    fn test_rect_intersects_contained() {
        let outer = Rect::new(0.0, 0.0, 200.0, 200.0);
        let inner = Rect::new(50.0, 50.0, 10.0, 10.0);
        assert!(outer.intersects(&inner));
        assert!(inner.intersects(&outer));
    }

    #[test]
    fn test_rect_contains_point_just_outside() {
        let r = Rect::new(10.0, 10.0, 100.0, 50.0);
        assert!(!r.contains_point(Vec2::new(9.99, 30.0)));
        assert!(!r.contains_point(Vec2::new(110.01, 30.0)));
        assert!(!r.contains_point(Vec2::new(50.0, 9.99)));
        assert!(!r.contains_point(Vec2::new(50.0, 60.01)));
    }

    #[test]
    fn test_rect_area_negative_dimensions() {
        // Negative dimensions (arrows) still produce positive area
        let r = Rect::new(100.0, 100.0, -50.0, -30.0);
        assert_eq!(r.area(), 1500.0);
    }

    #[test]
    fn test_rect_as_hisab() {
        let r = Rect::new(10.0, 20.0, 30.0, 40.0);
        let h = r.as_hisab();
        assert_eq!(h.min.x, 10.0);
        assert_eq!(h.min.y, 20.0);
        assert_eq!(h.max.x, 40.0);
        assert_eq!(h.max.y, 60.0);
    }
}
