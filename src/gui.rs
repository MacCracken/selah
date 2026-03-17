//! Interactive annotation GUI for Selah — built on egui + eframe.

use egui::{self, Color32, Pos2, Rect as EguiRect, Stroke, Vec2};
use selah_core::{Annotation, AnnotationKind, Color, Rect};
use std::path::PathBuf;

/// Annotation tool selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Rectangle,
    Circle,
    Arrow,
    Highlight,
    Text,
    Redaction,
}

impl Tool {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Rectangle => "Rectangle",
            Self::Circle => "Circle",
            Self::Arrow => "Arrow",
            Self::Highlight => "Highlight",
            Self::Text => "Text",
            Self::Redaction => "Redaction",
        }
    }

    pub fn shortcut(&self) -> &'static str {
        match self {
            Self::Select => "V",
            Self::Rectangle => "R",
            Self::Circle => "C",
            Self::Arrow => "A",
            Self::Highlight => "H",
            Self::Text => "T",
            Self::Redaction => "D",
        }
    }
}

/// Main GUI application state.
pub struct SelahGui {
    image_path: PathBuf,
    source_data: Vec<u8>,
    image_texture: Option<egui::TextureHandle>,
    image_size: [u32; 2],

    canvas: selah_annotate::AnnotationCanvas,
    active_tool: Tool,
    annotation_color: [f32; 3],

    // Viewport
    zoom: f32,
    pan: Vec2,

    // Drag interaction
    drag_start: Option<Pos2>,

    // Output
    save_path: Option<String>,
    status: String,
}

impl SelahGui {
    pub fn new(image_path: PathBuf, save_path: Option<String>) -> Result<Self, String> {
        let source_data =
            std::fs::read(&image_path).map_err(|e| format!("failed to read image: {e}"))?;
        let img = image::load_from_memory(&source_data)
            .map_err(|e| format!("failed to decode image: {e}"))?;
        let (w, h) = (img.width(), img.height());

        Ok(Self {
            image_path,
            source_data,
            image_texture: None,
            image_size: [w, h],
            canvas: selah_annotate::AnnotationCanvas::new(w, h),
            active_tool: Tool::Rectangle,
            annotation_color: [1.0, 0.0, 0.0],
            zoom: 1.0,
            pan: Vec2::ZERO,
            drag_start: None,
            save_path,
            status: "Ready — drag to annotate, Ctrl+S to save".into(),
        })
    }

    fn ensure_texture(&mut self, ctx: &egui::Context) {
        if self.image_texture.is_none() {
            let img = image::load_from_memory(&self.source_data).unwrap();
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let color_image =
                egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
            self.image_texture =
                Some(ctx.load_texture("source_image", color_image, egui::TextureOptions::LINEAR));
        }
    }

    fn screen_to_image(&self, screen_pos: Pos2, canvas_origin: Pos2) -> (f64, f64) {
        let x = (screen_pos.x - canvas_origin.x) / self.zoom;
        let y = (screen_pos.y - canvas_origin.y) / self.zoom;
        (x as f64, y as f64)
    }

    fn color_to_selah(&self) -> Color {
        Color::new(
            (self.annotation_color[0] * 255.0) as u8,
            (self.annotation_color[1] * 255.0) as u8,
            (self.annotation_color[2] * 255.0) as u8,
            255,
        )
    }

    fn color_to_egui(color: &Color) -> Color32 {
        Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            let mods = i.modifiers;

            // Ctrl+S = Save
            if mods.ctrl && i.key_pressed(egui::Key::S) {
                self.save();
            }
            // Ctrl+Z = Undo (remove last annotation)
            if mods.ctrl && i.key_pressed(egui::Key::Z) && !mods.shift {
                let anns = self.canvas.get_annotations();
                if let Some(last) = anns.last() {
                    let id = last.id;
                    self.canvas.remove_annotation(id);
                    self.status = "Removed last annotation".into();
                }
            }
            // Delete = remove last annotation
            if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
                let anns = self.canvas.get_annotations();
                if let Some(last) = anns.last() {
                    let id = last.id;
                    self.canvas.remove_annotation(id);
                    self.status = "Removed last annotation".into();
                }
            }
            // Tool shortcuts (only without modifiers)
            if !mods.ctrl && !mods.alt {
                if i.key_pressed(egui::Key::R) {
                    self.active_tool = Tool::Rectangle;
                }
                if i.key_pressed(egui::Key::C) {
                    self.active_tool = Tool::Circle;
                }
                if i.key_pressed(egui::Key::A) {
                    self.active_tool = Tool::Arrow;
                }
                if i.key_pressed(egui::Key::H) {
                    self.active_tool = Tool::Highlight;
                }
                if i.key_pressed(egui::Key::T) {
                    self.active_tool = Tool::Text;
                }
                if i.key_pressed(egui::Key::D) {
                    self.active_tool = Tool::Redaction;
                }
                if i.key_pressed(egui::Key::V) {
                    self.active_tool = Tool::Select;
                }
            }
            // Zoom
            if i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals) {
                self.zoom = (self.zoom * 1.25).clamp(0.1, 32.0);
            }
            if i.key_pressed(egui::Key::Minus) {
                self.zoom = (self.zoom / 1.25).clamp(0.1, 32.0);
            }
            // 0 = reset zoom
            if i.key_pressed(egui::Key::Num0) && !mods.ctrl {
                self.zoom = 1.0;
                self.pan = Vec2::ZERO;
            }
        });
    }

    fn save(&mut self) {
        let out = self.save_path.clone().unwrap_or_else(|| {
            let p = &self.image_path;
            let stem = p.file_stem().unwrap_or_default().to_string_lossy();
            let ext = p.extension().unwrap_or_default().to_string_lossy();
            format!("{stem}_annotated.{ext}")
        });

        let annotations = self.canvas.get_annotations().to_vec();
        match selah_annotate::AnnotationCanvas::render_to_image(
            &self.source_data,
            &annotations,
            selah_core::ImageFormat::Png,
        ) {
            Ok(data) => match std::fs::write(&out, &data) {
                Ok(()) => {
                    self.status = format!("Saved {} annotation(s) to {out}", annotations.len());
                }
                Err(e) => self.status = format!("Save error: {e}"),
            },
            Err(e) => self.status = format!("Render error: {e}"),
        }
    }

    fn paint_annotations(&self, painter: &egui::Painter, canvas_origin: Pos2) {
        for ann in self.canvas.get_annotations() {
            let pos = &ann.position;
            let screen_rect = EguiRect::from_min_size(
                Pos2::new(
                    canvas_origin.x + pos.x as f32 * self.zoom,
                    canvas_origin.y + pos.y as f32 * self.zoom,
                ),
                Vec2::new(pos.width as f32 * self.zoom, pos.height as f32 * self.zoom),
            );
            let color = Self::color_to_egui(&ann.color);

            match ann.kind {
                AnnotationKind::Rectangle => {
                    painter.rect_stroke(
                        screen_rect,
                        0.0,
                        Stroke::new(2.0, color),
                        egui::StrokeKind::Outside,
                    );
                }
                AnnotationKind::Circle => {
                    let center = screen_rect.center();
                    let radius = screen_rect.width().min(screen_rect.height()) / 2.0;
                    painter.circle_stroke(center, radius, Stroke::new(2.0, color));
                }
                AnnotationKind::Arrow => {
                    let start = screen_rect.left_top();
                    let end = Pos2::new(screen_rect.right(), screen_rect.bottom());
                    painter.line_segment([start, end], Stroke::new(2.0, color));
                    // Arrowhead
                    let dx = end.x - start.x;
                    let dy = end.y - start.y;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len > 0.0 {
                        let ux = dx / len;
                        let uy = dy / len;
                        let head_len = 12.0;
                        let head_width = 6.0;
                        let p1 = Pos2::new(
                            end.x - head_len * ux + head_width * uy,
                            end.y - head_len * uy - head_width * ux,
                        );
                        let p2 = Pos2::new(
                            end.x - head_len * ux - head_width * uy,
                            end.y - head_len * uy + head_width * ux,
                        );
                        painter.line_segment([end, p1], Stroke::new(2.0, color));
                        painter.line_segment([end, p2], Stroke::new(2.0, color));
                    }
                }
                AnnotationKind::Highlight => {
                    let fill =
                        Color32::from_rgba_unmultiplied(ann.color.r, ann.color.g, ann.color.b, 77);
                    painter.rect_filled(screen_rect, 0.0, fill);
                }
                AnnotationKind::Redaction => {
                    painter.rect_filled(screen_rect, 0.0, Color32::BLACK);
                }
                AnnotationKind::Text => {
                    let text = ann.text.as_deref().unwrap_or("Text");
                    painter.text(
                        screen_rect.left_top() + Vec2::new(2.0, 2.0),
                        egui::Align2::LEFT_TOP,
                        text,
                        egui::FontId::proportional(16.0 * self.zoom),
                        color,
                    );
                }
                AnnotationKind::FreeForm => {
                    painter.rect_stroke(
                        screen_rect,
                        0.0,
                        Stroke::new(1.0, color),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        }
    }

    fn canvas_origin(&self, panel_rect: EguiRect) -> Pos2 {
        let img_w = self.image_size[0] as f32 * self.zoom;
        let img_h = self.image_size[1] as f32 * self.zoom;
        Pos2::new(
            panel_rect.center().x - img_w / 2.0 + self.pan.x,
            panel_rect.center().y - img_h / 2.0 + self.pan.y,
        )
    }
}

impl eframe::App for SelahGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ensure_texture(ctx);
        self.handle_keyboard(ctx);

        // ── Menu bar ──
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Save (Ctrl+S)").clicked() {
                        self.save();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Save Annotations...").clicked() {
                        let path = self.image_path.with_extension("annotations.json");
                        match self.canvas.save_to_file(&path) {
                            Ok(()) => {
                                self.status = format!("Saved layer to {}", path.display());
                            }
                            Err(e) => self.status = format!("Error: {e}"),
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo (Ctrl+Z)").clicked() {
                        let anns = self.canvas.get_annotations();
                        if let Some(last) = anns.last() {
                            let id = last.id;
                            self.canvas.remove_annotation(id);
                            self.status = "Removed last annotation".into();
                        }
                        ui.close_menu();
                    }
                    if ui.button("Clear All").clicked() {
                        self.canvas.clear();
                        self.status = "Cleared all annotations".into();
                        ui.close_menu();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.button("Zoom In (+)").clicked() {
                        self.zoom = (self.zoom * 1.25).clamp(0.1, 32.0);
                        ui.close_menu();
                    }
                    if ui.button("Zoom Out (-)").clicked() {
                        self.zoom = (self.zoom / 1.25).clamp(0.1, 32.0);
                        ui.close_menu();
                    }
                    if ui.button("Fit to Window (0)").clicked() {
                        self.zoom = 1.0;
                        self.pan = Vec2::ZERO;
                        ui.close_menu();
                    }
                });
            });
        });

        // ── Status bar ──
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!(
                        "{}x{} | {} annotations | {:.0}%",
                        self.image_size[0],
                        self.image_size[1],
                        self.canvas.count(),
                        self.zoom * 100.0,
                    ));
                });
            });
        });

        // ── Tool palette (left) ──
        egui::SidePanel::left("tool_palette")
            .default_width(48.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Tools");
                    ui.separator();
                    for tool in [
                        Tool::Select,
                        Tool::Rectangle,
                        Tool::Circle,
                        Tool::Arrow,
                        Tool::Highlight,
                        Tool::Text,
                        Tool::Redaction,
                    ] {
                        let selected = self.active_tool == tool;
                        let btn = egui::Button::new(tool.shortcut())
                            .min_size(Vec2::new(36.0, 36.0))
                            .selected(selected);
                        if ui.add(btn).on_hover_text(tool.name()).clicked() {
                            self.active_tool = tool;
                        }
                    }
                    ui.separator();
                    ui.heading("Color");
                    ui.color_edit_button_rgb(&mut self.annotation_color);
                });
            });

        // ── Annotation list (right) ──
        egui::SidePanel::right("annotation_panel")
            .default_width(200.0)
            .show(ctx, |ui| {
                ui.heading("Annotations");
                ui.separator();
                let mut to_remove = None;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let anns = self.canvas.get_annotations();
                    for (i, ann) in anns.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let c = Self::color_to_egui(&ann.color);
                            ui.colored_label(c, format!("#{} {}", i + 1, ann.kind));
                            if ui.small_button("x").clicked() {
                                to_remove = Some(ann.id);
                            }
                        });
                    }
                });
                if let Some(id) = to_remove {
                    self.canvas.remove_annotation(id);
                    self.status = "Removed annotation".into();
                }
                ui.separator();
                if ui.button("Clear All").clicked() {
                    self.canvas.clear();
                    self.status = "Cleared all annotations".into();
                }
                if ui.button("Save Image (Ctrl+S)").clicked() {
                    self.save();
                }
            });

        // ── Canvas (center) ──
        egui::CentralPanel::default().show(ctx, |ui| {
            let available = ui.available_size();
            let (response, painter) = ui.allocate_painter(available, egui::Sense::click_and_drag());
            let rect = response.rect;

            // Dark workspace background
            painter.rect_filled(rect, 0.0, Color32::from_gray(40));

            // Calculate image position
            let canvas_origin = self.canvas_origin(rect);
            let img_w = self.image_size[0] as f32 * self.zoom;
            let img_h = self.image_size[1] as f32 * self.zoom;
            let img_rect = EguiRect::from_min_size(canvas_origin, Vec2::new(img_w, img_h));

            // Draw image texture
            if let Some(tex) = &self.image_texture {
                painter.image(
                    tex.id(),
                    img_rect,
                    EguiRect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }

            // Pan with middle-click drag
            if response.dragged_by(egui::PointerButton::Middle) {
                self.pan += response.drag_delta();
            }

            // Zoom with scroll wheel
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.1 {
                let factor = if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 };
                self.zoom = (self.zoom * factor).clamp(0.1, 32.0);
            }

            // ── Annotation creation via drag ──
            if self.active_tool != Tool::Select {
                if response.drag_started_by(egui::PointerButton::Primary)
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    self.drag_start = Some(pos);
                }

                // Preview while dragging
                if let Some(start) = self.drag_start
                    && let Some(current) = response.interact_pointer_pos()
                {
                    let preview_rect = EguiRect::from_two_pos(start, current);
                    let preview_color = Color32::from_rgba_unmultiplied(
                        (self.annotation_color[0] * 255.0) as u8,
                        (self.annotation_color[1] * 255.0) as u8,
                        (self.annotation_color[2] * 255.0) as u8,
                        128,
                    );
                    painter.rect_stroke(
                        preview_rect,
                        0.0,
                        Stroke::new(1.0, preview_color),
                        egui::StrokeKind::Outside,
                    );
                }

                if response.drag_stopped_by(egui::PointerButton::Primary)
                    && let Some(start) = self.drag_start.take()
                    && let Some(end) = response.interact_pointer_pos()
                {
                    let (x0, y0) = self.screen_to_image(start, canvas_origin);
                    let (x1, y1) = self.screen_to_image(end, canvas_origin);
                    let rx = x0.min(x1);
                    let ry = y0.min(y1);
                    let rw = (x1 - x0).abs();
                    let rh = (y1 - y0).abs();

                    if rw > 1.0 && rh > 1.0 {
                        let kind = match self.active_tool {
                            Tool::Rectangle => AnnotationKind::Rectangle,
                            Tool::Circle => AnnotationKind::Circle,
                            Tool::Arrow => AnnotationKind::Arrow,
                            Tool::Highlight => AnnotationKind::Highlight,
                            Tool::Text => AnnotationKind::Text,
                            Tool::Redaction => AnnotationKind::Redaction,
                            Tool::Select => unreachable!(),
                        };

                        // Arrows use start->end direction, not normalized rect
                        let position = if kind == AnnotationKind::Arrow {
                            Rect::new(x0, y0, x1 - x0, y1 - y0)
                        } else {
                            Rect::new(rx, ry, rw, rh)
                        };

                        let ann = Annotation::new(kind, position, self.color_to_selah());
                        self.canvas.add_annotation(ann);
                        self.status = format!(
                            "Added {} annotation",
                            self.active_tool.name().to_lowercase()
                        );
                    }
                }
            }

            // Paint existing annotations
            self.paint_annotations(&painter, canvas_origin);

            // Image border
            painter.rect_stroke(
                img_rect,
                0.0,
                Stroke::new(1.0, Color32::from_gray(100)),
                egui::StrokeKind::Outside,
            );
        });
    }
}

/// Launch the Selah annotation GUI.
pub fn run_gui(image_path: PathBuf, output: Option<String>) -> Result<(), eframe::Error> {
    let title = format!("Selah — {}", image_path.display());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(&title)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Selah",
        options,
        Box::new(move |_cc| {
            let app = SelahGui::new(image_path, output).expect("Failed to load image");
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_names_and_shortcuts() {
        let tools = [
            Tool::Select,
            Tool::Rectangle,
            Tool::Circle,
            Tool::Arrow,
            Tool::Highlight,
            Tool::Text,
            Tool::Redaction,
        ];
        for tool in tools {
            assert!(!tool.name().is_empty());
            assert!(!tool.shortcut().is_empty());
        }
    }

    #[test]
    fn tool_equality() {
        assert_eq!(Tool::Rectangle, Tool::Rectangle);
        assert_ne!(Tool::Rectangle, Tool::Circle);
    }

    #[test]
    fn seven_tools_total() {
        let count = [
            Tool::Select,
            Tool::Rectangle,
            Tool::Circle,
            Tool::Arrow,
            Tool::Highlight,
            Tool::Text,
            Tool::Redaction,
        ]
        .len();
        assert_eq!(count, 7);
    }

    #[test]
    fn color_conversion_round_trip() {
        let color = Color::new(128, 64, 32, 255);
        let egui_color = SelahGui::color_to_egui(&color);
        assert_eq!(
            egui_color,
            Color32::from_rgba_unmultiplied(128, 64, 32, 255)
        );
    }
}
