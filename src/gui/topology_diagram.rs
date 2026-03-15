use iced::Color;

use crate::core::process_config::ProcessGroup;

// ─── Colour helpers ───────────────────────────────────────────────────────────

/// Give each group a stable hue based on its position in the group list.
pub fn group_color(group_index: usize) -> Color {
    const COLORS: &[Color] = &[
        Color::from_rgb(0.0, 0.58, 1.0),   // sky-blue
        Color::from_rgb(1.0, 0.84, 0.0),   // gold
        Color::from_rgb(0.39, 0.86, 0.39), // light-green
        Color::from_rgb(1.0, 0.54, 0.0),   // orange
        Color::from_rgb(0.70, 0.39, 1.0),  // purple
        Color::from_rgb(0.0, 0.82, 0.82),  // cyan
        Color::from_rgb(1.0, 0.3, 0.3),    // light-red
        Color::from_rgb(1.0, 0.70, 0.70),  // light-pink
    ];
    COLORS[group_index % COLORS.len()]
}

/// Alternating muted colours for the CCD / P-core / E-core outer boxes.
pub fn group_section_color(index: usize) -> Color {
    const COLS: &[Color] = &[
        Color::from_rgb(0.31, 0.47, 0.70), // muted blue
        Color::from_rgb(0.62, 0.39, 0.23), // muted orange
        Color::from_rgb(0.23, 0.54, 0.31), // muted green
        Color::from_rgb(0.54, 0.27, 0.54), // muted purple
    ];
    COLS[index % COLS.len()]
}

// ─── Geometry constants ───────────────────────────────────────────────────────
// Retained for future canvas-based rendering.
#[allow(dead_code)]
pub const THREAD_W: f32 = 52.0;
#[allow(dead_code)]
pub const THREAD_H: f32 = 68.0;
#[allow(dead_code)]
pub const THREAD_GAP: f32 = 3.0; // gap between HT siblings inside a core box
#[allow(dead_code)]
pub const CORE_PAD: f32 = 5.0; // padding inside physical-core box
#[allow(dead_code)]
pub const CORE_LABEL_H: f32 = 13.0; // "C0  5.27 GHz" line
#[allow(dead_code)]
pub const CACHE_LINE_H: f32 = 11.0; // height per private cache label line
#[allow(dead_code)]
pub const CORE_GAP: f32 = 6.0; // gap between physical-core boxes
#[allow(dead_code)]
pub const GROUP_PAD: f32 = 10.0; // padding inside the outer group box
#[allow(dead_code)]
pub const GROUP_FOOTER_H: f32 = 14.0; // base footer height (group name row)
#[allow(dead_code)]
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

use iced::widget::{column, container, row, text, Column, Row};
use iced::{Alignment, Background, Element, Length};

use crate::gui::Message;

// ─── Topology diagram drawing (Iced version) ──────────────────────────────────

/// Draw one top-level group (CCD / P-cores / E-cores / All Cores) as a rounded
/// rectangle containing a grid of physical-core boxes, each containing thread bars.
pub fn draw_topology_group<'a>(
    group: &'a crate::core::topology::TopLevelGroup,
    stats: &crate::core::process_overwatch::CpuStats,
    core_group_map: &[Option<usize>],
    stroke_col: Color,
) -> Element<'a, Message> {
    let cores_per_row = 4usize.min(group.physical_cores.len()).max(1);

    // We'll build a grid of cores
    let core_widgets: Vec<Element<'a, Message>> = group
        .physical_cores
        .iter()
        .map(|phys_core| {
            // Build thread bars for this core
            let thread_bars: Row<Message> = phys_core
                .threads
                .iter()
                .map(|thread| {
                    let usage = stats
                        .per_core
                        .get(thread.logical_index)
                        .copied()
                        .unwrap_or(0.0);
                    let fill_col = core_group_map
                        .get(thread.logical_index)
                        .and_then(|&g| g)
                        .map_or(Color::from_rgb(0.21, 0.21, 0.21), group_color);

                    // Visualize each logical thread as a vertical bar:
                    // - background colour comes from its group assignment
                    // - the bar's height encodes the CPU usage percentage (max 60 px)
                    let bar_height = (usage / 100.0).clamp(0.0, 1.0) * 60.0;
                    let bar_height = bar_height.max(2.0); // always show at least a sliver

                    container(
                        text(format!("T{}", thread.logical_index))
                            .size(10)
                            .color(Color::WHITE),
                    )
                    .width(Length::Fixed(40.0))
                    .height(Length::Fixed(bar_height))
                    .style(move |_| iced::widget::container::Style {
                        background: Some(Background::Color(fill_col)),
                        ..Default::default()
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .fold(Row::new().spacing(3), |row, elem| row.push(elem));

            // Physical core container
            container(
                column![
                    row![text(format!("C{}", phys_core.physical_index))
                        .size(10)
                        .color(Color::from_rgb(0.7, 0.7, 0.7))]
                    .align_y(Alignment::Center),
                    thread_bars
                ]
                .spacing(5)
                .align_x(Alignment::Center),
            )
            .padding(5)
            .into()
        })
        .collect();

    // Arrange cores in rows
    let mut core_rows: Vec<Row<Message>> = Vec::new();
    let mut current_row = Row::new().spacing(6);
    let mut count = 0;
    for core_widget in core_widgets.into_iter() {
        current_row = current_row.push(core_widget);
        count += 1;
        if count % cores_per_row == 0 {
            core_rows.push(current_row);
            current_row = Row::new().spacing(6);
        }
    }
    if count % cores_per_row != 0 {
        core_rows.push(current_row);
    }

    let core_grid = core_rows
        .into_iter()
        .fold(Column::new().spacing(6), |col, row| col.push(row));

    // Group footer
    let footer = column![
        text(group.label.as_str()).size(12).color(stroke_col),
        group
            .shared_caches
            .iter()
            .fold(Column::new().spacing(2), |col, cache| col.push(
                text(cache.label())
                    .size(10)
                    .color(Color::from_rgb(0.6, 0.6, 0.6))
            ))
    ]
    .align_x(Alignment::Center);

    container(
        column![core_grid, footer]
            .spacing(10)
            .align_x(Alignment::Center),
    )
    .padding(10)
    .into()
}
