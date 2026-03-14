use eframe::egui::{self, Color32, RichText};

/// Compact pill showing a process name, sized tightly to its text.
/// Dimmed when the process is not currently running.
pub fn process_pill(ui: &mut egui::Ui, name: &str, is_running: bool) {
    let text_col = if is_running {
        Color32::from_gray(220)
    } else {
        Color32::from_gray(105)
    };
    let border_col = if is_running {
        Color32::from_gray(115)
    } else {
        Color32::from_gray(55)
    };

    // Measure the text first so we can size the pill to fit exactly.
    let font_id = egui::FontId::proportional(12.0);
    let galley = ui
        .painter()
        .layout_no_wrap(name.to_string(), font_id.clone(), text_col);

    let pad_x = 6.0;
    let pad_y = 3.0;
    let desired = egui::vec2(galley.size().x + pad_x * 2.0, galley.size().y + pad_y * 2.0);

    // Reserve the exact footprint in the layout.
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::hover());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        // ── Pill background ───────────────────────────────────────────────
        painter.rect_filled(rect, 5.0, Color32::from_gray(32));
        // ── Pill border ───────────────────────────────────────────────────
        painter.rect_stroke(
            rect,
            5.0,
            egui::Stroke::new(1.0, border_col),
            egui::StrokeKind::Inside,
        );
        // ── Pill text label ───────────────────────────────────────────────
        painter.galley(rect.min + egui::vec2(pad_x, pad_y), galley, text_col);
    }

    let _ = response;
}

/// Compact pill with an inline ✏ button that fires `edit_out` when clicked.
/// Used for Custom Process entries where clicking ✏ reassigns the process.
pub fn process_pill_edit(ui: &mut egui::Ui, name: &str, edit_out: &mut Option<String>) {
    let text_col = Color32::from_gray(200);
    let font_id = egui::FontId::proportional(12.0);
    let galley = ui
        .painter()
        .layout_no_wrap(name.to_string(), font_id, text_col);

    let pad_x = 6.0;
    let pad_y = 3.0;
    let btn_w = 14.0; // approximate width of the ✏ icon
    let gap = 4.0;
    let desired = egui::vec2(
        galley.size().x + pad_x * 2.0 + gap + btn_w,
        galley.size().y + pad_y * 2.0,
    );

    // Reserve the exact footprint in the layout.
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        // ── Pill background ───────────────────────────────────────────────
        painter.rect_filled(rect, 5.0, Color32::from_gray(32));
        // ── Pill border ───────────────────────────────────────────────────
        painter.rect_stroke(
            rect,
            5.0,
            egui::Stroke::new(1.0, Color32::from_gray(90)),
            egui::StrokeKind::Inside,
        );
        // ── Pill text label ───────────────────────────────────────────────
        painter.galley(rect.min + egui::vec2(pad_x, pad_y), galley, text_col);
    }

    // ── Edit (✏) button ───────────────────────────────────────────────────
    let btn_rect = egui::Rect::from_min_size(
        egui::pos2(rect.max.x - btn_w - pad_x * 0.5, rect.min.y),
        egui::vec2(btn_w + pad_x * 0.5, rect.height()),
    );
    let btn_resp = ui.interact(btn_rect, ui.id().with(name).with("x"), egui::Sense::click());
    if btn_resp.hovered() {
        // ── Edit button hover highlight ───────────────────────────────────
        ui.painter()
            .rect_filled(btn_rect, 3.0, Color32::from_gray(50));
    }
    // ── Edit button icon ──────────────────────────────────────────────────
    ui.painter().text(
        btn_rect.center(),
        egui::Align2::CENTER_CENTER,
        "✏",
        egui::FontId::proportional(10.0),
        if btn_resp.hovered() {
            Color32::from_gray(230)
        } else {
            Color32::from_gray(150)
        },
    );
    if btn_resp.clicked() {
        *edit_out = Some(name.to_string());
    }
}

/// Numeric stat badge: large coloured number with a small label underneath.
/// Used in the Status tab header row.
pub fn stat_badge(ui: &mut egui::Ui, label: &str, value: usize, col: Color32) {
    ui.vertical(|ui| {
        // ── Value (large, coloured) ───────────────────────────────────────
        ui.label(RichText::new(value.to_string()).heading().color(col));
        // ── Label (small, muted) ──────────────────────────────────────────
        ui.label(RichText::new(label).small());
    });
}

/// Small filled colour square followed by a text label.
/// Used in the topology diagram legend.
pub fn color_swatch(ui: &mut egui::Ui, col: Color32, text: &str) {
    ui.horizontal(|ui| {
        // ── Colour square ─────────────────────────────────────────────────
        let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0, col);
        // ── Swatch label ──────────────────────────────────────────────────
        ui.label(RichText::new(text).small());
        ui.add_space(6.0);
    });
}
