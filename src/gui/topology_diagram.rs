use eframe::egui::{self, Color32};

use crate::core::process_config::ProcessGroup;

// ─── Colour helpers ───────────────────────────────────────────────────────────

/// Give each group a stable hue based on its position in the group list.
pub fn group_color(group_index: usize) -> Color32 {
    const COLORS: &[Color32] = &[
        Color32::from_rgb(0, 150, 255), // sky-blue
        Color32::GOLD,
        Color32::from_rgb(100, 220, 100), // light-green
        Color32::from_rgb(255, 140, 0),   // orange
        Color32::from_rgb(180, 100, 255), // purple
        Color32::from_rgb(0, 210, 210),   // cyan
        Color32::LIGHT_RED,
        Color32::from_rgb(255, 180, 180), // light-pink
    ];
    COLORS[group_index % COLORS.len()]
}

/// Alternating muted colours for the CCD / P-core / E-core outer boxes.
pub fn group_section_color(index: usize) -> Color32 {
    const COLS: &[Color32] = &[
        Color32::from_rgb(80, 120, 180), // muted blue
        Color32::from_rgb(160, 100, 60), // muted orange
        Color32::from_rgb(60, 140, 80),  // muted green
        Color32::from_rgb(140, 70, 140), // muted purple
    ];
    COLS[index % COLS.len()]
}

// ─── Geometry constants ───────────────────────────────────────────────────────

pub const THREAD_W: f32 = 52.0;
pub const THREAD_H: f32 = 68.0;
pub const THREAD_GAP: f32 = 3.0; // gap between HT siblings inside a core box
pub const CORE_PAD: f32 = 5.0; // padding inside physical-core box
pub const CORE_LABEL_H: f32 = 13.0; // "C0  5.27 GHz" line
pub const CACHE_LINE_H: f32 = 11.0; // height per private cache label line
pub const CORE_GAP: f32 = 6.0; // gap between physical-core boxes
pub const GROUP_PAD: f32 = 10.0; // padding inside the outer group box
pub const GROUP_FOOTER_H: f32 = 14.0; // base footer height (group name row)
pub const GROUP_FOOTER_CACHE_H: f32 = 12.0; // height per shared-cache label row

// ─── Core group map ───────────────────────────────────────────────────────────

/// For each logical core index, determine which configured group (by index) has
/// that core in its affinity set.
pub fn build_core_group_map(groups: &[ProcessGroup], num_cores: usize) -> Vec<Option<usize>> {
    let mut map = vec![None; num_cores];
    for (gi, g) in groups.iter().enumerate() {
        if let Some(ref aff) = g.affinity {
            for &c in &aff.core_list {
                if c < num_cores {
                    map[c] = Some(gi);
                }
            }
        }
    }
    map
}

// ─── Topology diagram drawing ─────────────────────────────────────────────────

/// Draw one top-level group (CCD / P-cores / E-cores / All Cores) as a rounded
/// rectangle containing a grid of physical-core boxes, each containing thread bars.
pub fn draw_topology_group(
    ui: &mut egui::Ui,
    group: &crate::core::topology::TopLevelGroup,
    stats: &crate::core::process_overwatch::CpuStats,
    core_group_map: &[Option<usize>],
    stroke_col: Color32,
) {
    let cores_per_row = 4usize.min(group.physical_cores.len()).max(1);
    let num_rows = (group.physical_cores.len() + cores_per_row - 1) / cores_per_row;

    // Widest core (determines uniform column width)
    let max_threads = group
        .physical_cores
        .iter()
        .map(|c| c.threads.len())
        .max()
        .unwrap_or(1);
    let cell_w = CORE_PAD * 2.0
        + max_threads as f32 * THREAD_W
        + (max_threads.saturating_sub(1)) as f32 * THREAD_GAP;

    // Tallest core (due to variable private-cache count)
    let max_private_caches = group
        .physical_cores
        .iter()
        .map(|c| c.private_caches.len())
        .max()
        .unwrap_or(0);
    let cell_h =
        CORE_PAD * 2.0 + THREAD_H + CORE_LABEL_H + max_private_caches as f32 * CACHE_LINE_H;

    let group_inner_w = cores_per_row as f32 * cell_w + (cores_per_row - 1) as f32 * CORE_GAP;
    let group_inner_h = num_rows as f32 * cell_h + (num_rows - 1) as f32 * CORE_GAP;

    // Footer: group name + one line per shared cache level
    let footer_h = GROUP_FOOTER_H + group.shared_caches.len() as f32 * GROUP_FOOTER_CACHE_H;

    let group_w = GROUP_PAD * 2.0 + group_inner_w;
    let group_h = GROUP_PAD * 2.0 + group_inner_h + footer_h;

    let (group_rect, _) =
        ui.allocate_exact_size(egui::vec2(group_w, group_h), egui::Sense::hover());
    let painter = ui.painter();

    // Outer group box
    painter.rect_filled(group_rect, 8.0, Color32::from_gray(22));
    painter.rect_stroke(
        group_rect,
        8.0,
        egui::Stroke::new(2.0, stroke_col),
        egui::StrokeKind::Inside,
    );

    // ── Physical core cells ───────────────────────────────────────────────
    let content_origin = group_rect.min + egui::vec2(GROUP_PAD, GROUP_PAD);

    for (ci, phys_core) in group.physical_cores.iter().enumerate() {
        let row = ci / cores_per_row;
        let col = ci % cores_per_row;
        let core_x = content_origin.x + col as f32 * (cell_w + CORE_GAP);
        let core_y = content_origin.y + row as f32 * (cell_h + CORE_GAP);
        let core_rect =
            egui::Rect::from_min_size(egui::pos2(core_x, core_y), egui::vec2(cell_w, cell_h));

        // Core background + border
        painter.rect_filled(core_rect, 5.0, Color32::from_gray(32));
        painter.rect_stroke(
            core_rect,
            5.0,
            egui::Stroke::new(1.0, Color32::from_gray(70)),
            egui::StrokeKind::Inside,
        );

        // ── Thread bars ───────────────────────────────────────────────────
        let threads_total_w = phys_core.threads.len() as f32 * THREAD_W
            + (phys_core.threads.len().saturating_sub(1)) as f32 * THREAD_GAP;
        let threads_start_x = core_x + (cell_w - threads_total_w) / 2.0;
        let threads_top_y = core_y + CORE_PAD;

        for (ti, thread) in phys_core.threads.iter().enumerate() {
            let tx = threads_start_x + ti as f32 * (THREAD_W + THREAD_GAP);
            let thread_rect = egui::Rect::from_min_size(
                egui::pos2(tx, threads_top_y),
                egui::vec2(THREAD_W, THREAD_H),
            );

            let usage = stats
                .per_core
                .get(thread.logical_index)
                .copied()
                .unwrap_or(0.0);
            let frac = (usage / 100.0).clamp(0.0, 1.0);
            let fill_col = core_group_map
                .get(thread.logical_index)
                .and_then(|&g| g)
                .map_or(Color32::from_gray(55), group_color);

            painter.rect_filled(thread_rect, 3.0, Color32::from_gray(28));
            let filled_h = THREAD_H * frac;
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(thread_rect.min.x, thread_rect.max.y - filled_h),
                    thread_rect.max,
                ),
                2.0,
                fill_col,
            );
            painter.rect_stroke(
                thread_rect,
                3.0,
                egui::Stroke::new(1.0, Color32::from_gray(65)),
                egui::StrokeKind::Inside,
            );

            let lbl = match thread.kind {
                crate::core::topology::CoreKind::Pcore => {
                    format!("P{}", thread.logical_index)
                }
                crate::core::topology::CoreKind::Ecore => {
                    format!("E{}", thread.logical_index)
                }
                crate::core::topology::CoreKind::Unknown => {
                    format!("T{}", thread.logical_index)
                }
            };
            painter.text(
                thread_rect.center_top() + egui::vec2(0.0, 7.0),
                egui::Align2::CENTER_TOP,
                &lbl,
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );
            painter.text(
                thread_rect.center_bottom() - egui::vec2(0.0, 5.0),
                egui::Align2::CENTER_BOTTOM,
                format!("{:.0}%", usage),
                egui::FontId::proportional(10.0),
                Color32::from_gray(210),
            );
        }

        // ── Core label row (C<n>  freq) ───────────────────────────────────
        let core_label_y = threads_top_y + THREAD_H + CORE_LABEL_H / 2.0 + 1.0;
        let freq_str = crate::core::topology::format_freq_ghz(phys_core.max_freq_khz);
        let core_sublbl = if freq_str.is_empty() {
            format!("C{}", phys_core.physical_index)
        } else {
            format!("C{}  {freq_str}", phys_core.physical_index)
        };
        painter.text(
            egui::pos2(core_rect.center().x, core_label_y),
            egui::Align2::CENTER_CENTER,
            &core_sublbl,
            egui::FontId::proportional(10.0),
            Color32::from_gray(180),
        );

        // ── Private cache labels (L1/L2) ──────────────────────────────────
        for (i, cache) in phys_core.private_caches.iter().enumerate() {
            let cache_y = core_label_y + CORE_LABEL_H / 2.0 + 2.0 + i as f32 * CACHE_LINE_H;
            painter.text(
                egui::pos2(core_rect.center().x, cache_y),
                egui::Align2::CENTER_CENTER,
                cache.label(),
                egui::FontId::proportional(9.5),
                Color32::from_gray(140),
            );
        }
    }

    // ── Group footer: name + shared caches (L3+) ─────────────────────────
    let footer_top = group_rect.min.y + GROUP_PAD + group_inner_h + 4.0;

    painter.text(
        egui::pos2(group_rect.center().x, footer_top + GROUP_FOOTER_H / 2.0),
        egui::Align2::CENTER_CENTER,
        &group.label,
        egui::FontId::proportional(12.0),
        stroke_col,
    );

    for (i, cache) in group.shared_caches.iter().enumerate() {
        let y = footer_top + GROUP_FOOTER_H + i as f32 * GROUP_FOOTER_CACHE_H + 4.0;
        painter.text(
            egui::pos2(group_rect.center().x, y),
            egui::Align2::CENTER_CENTER,
            cache.label(),
            egui::FontId::proportional(10.0),
            Color32::from_gray(155),
        );
    }
}
