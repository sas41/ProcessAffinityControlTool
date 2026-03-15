use iced::widget::{Column, Row, column, container, row, text};
use iced::{Alignment, Background, Color, Element, Length};

use crate::core::process_config::ProcessGroup;
use crate::gui::Message;

/// Returns a stable accent color for a process group index.
///
/// The palette intentionally wraps with modulo so group `n` always maps to
/// the same color across frames, even when there are more groups than colors.
pub fn group_color(group_index: usize) -> Color {
    const COLORS: &[Color] = &[
        Color::from_rgb(0.0, 0.58, 1.0),
        Color::from_rgb(1.0, 0.84, 0.0),
        Color::from_rgb(0.39, 0.86, 0.39),
        Color::from_rgb(1.0, 0.54, 0.0),
        Color::from_rgb(0.70, 0.39, 1.0),
        Color::from_rgb(0.0, 0.82, 0.82),
        Color::from_rgb(1.0, 0.3, 0.3),
        Color::from_rgb(1.0, 0.70, 0.70),
    ];
    COLORS[group_index % COLORS.len()]
}

/// Returns a muted color for top-level hardware sections.
///
/// These colors are separate from process-group accents so hardware grouping
/// remains visually distinct from process assignment overlays.
pub fn group_section_color(index: usize) -> Color {
    const COLS: &[Color] = &[
        Color::from_rgb(0.31, 0.47, 0.70),
        Color::from_rgb(0.62, 0.39, 0.23),
        Color::from_rgb(0.23, 0.54, 0.31),
        Color::from_rgb(0.54, 0.27, 0.54),
    ];
    COLS[index % COLS.len()]
}

/// Legacy layout constants retained for compatibility.
#[allow(dead_code)]
pub const THREAD_W: f32 = 52.0;

#[allow(dead_code)]
pub const THREAD_H: f32 = 68.0;

#[allow(dead_code)]
pub const THREAD_GAP: f32 = 3.0;

#[allow(dead_code)]
pub const CORE_PAD: f32 = 5.0;

#[allow(dead_code)]
pub const CORE_LABEL_H: f32 = 13.0;

#[allow(dead_code)]
pub const CACHE_LINE_H: f32 = 11.0;

#[allow(dead_code)]
pub const CORE_GAP: f32 = 6.0;

#[allow(dead_code)]
pub const GROUP_PAD: f32 = 10.0;

#[allow(dead_code)]
pub const GROUP_FOOTER_H: f32 = 14.0;

#[allow(dead_code)]
pub const GROUP_FOOTER_CACHE_H: f32 = 12.0;

/// Maps each logical core to the last process group that includes it.
///
/// Later groups overwrite earlier ones; this gives a single, deterministic
/// color per thread when affinity sets overlap.
/// C# note: `&[T]` is a borrowed read-only slice (like `IReadOnlyList<T>` view),
/// and `Vec<Option<usize>>` is a growable list of nullable-ish indices.
pub fn build_core_group_map(groups: &[ProcessGroup], num_cores: usize) -> Vec<Option<usize>> {
    let mut map = vec![None; num_cores];

    // C# note: `iter().enumerate()` is like `Select((item, index) => ...)`.
    for (gi, g) in groups.iter().enumerate() {
        // C# note: `if let Some(...) = ...` is pattern-matching for "has value".
        if let Some(ref aff) = g.affinity {
            // C# note: `for &c in &list` iterates borrowed items and copies each `usize` value.
            for &c in &aff.core_list {
                if c < num_cores {
                    map[c] = Some(gi);
                }
            }
        }
    }

    map
}
/// Draws one topology section: core grid first, then label/cache footer.
///
/// The explicit `'a` lifetime ties returned widget elements to borrowed data
/// from `group`. The `move` style closure copies `fill_col` by value so each
/// thread bar keeps its own color without borrowing temporary locals.
/// C# note: `Element<'a, Message>` means "UI node borrowing data valid for `'a`"
/// and carrying `Message` as its event type.
pub fn draw_topology_group<'a>(
    group: &'a crate::core::topology::TopLevelGroup,
    stats: &crate::core::process_overwatch::CpuStats,
    core_group_map: &[Option<usize>],
    stroke_col: Color,
) -> Element<'a, Message> {
    // C# note: `4usize` uses an explicit unsigned type suffix.
    let cores_per_row = 4usize.min(group.physical_cores.len()).max(1);

    let core_widgets: Vec<Element<'a, Message>> = group
        .physical_cores
        .iter()
        // C# note: `|x| { ... }` is a closure/lambda.
        .map(|phys_core| {
            let thread_bars: Row<Message> =
                phys_core
                    .threads
                    .iter()
                    .fold(Row::new().spacing(3), |row, thread| {
                        let usage = stats
                            .per_core
                            .get(thread.logical_index)
                            .copied()
                            .unwrap_or(0.0);
                        let fill_col = core_group_map
                            .get(thread.logical_index)
                            // C# note: `|&g| g` destructures `&Option<usize>` to `Option<usize>`.
                            .and_then(|&g| g)
                            .map_or(Color::from_rgb(0.21, 0.21, 0.21), group_color);
                        let bar_height = ((usage / 100.0).clamp(0.0, 1.0) * 60.0).max(2.0);

                        row.push(
                            container(
                                text(format!("T{}", thread.logical_index))
                                    .size(10)
                                    .color(Color::WHITE),
                            )
                            .width(Length::Fixed(40.0))
                            .height(Length::Fixed(bar_height))
                            .style(move |_| {
                                iced::widget::container::Style {
                                    background: Some(Background::Color(fill_col)),
                                    ..Default::default()
                                }
                            }),
                        )
                    });

            container(
                column![
                    row![
                        text(format!("C{}", phys_core.physical_index))
                            .size(10)
                            .color(Color::from_rgb(0.7, 0.7, 0.7))
                    ]
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

    let mut core_rows: Vec<Row<Message>> = Vec::new();
    let mut current_row = Row::new().spacing(6);
    let mut count = 0;

    for core_widget in core_widgets {
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

    let footer = column![
        text(group.label.as_str()).size(12).color(stroke_col),
        group
            .shared_caches
            .iter()
            .fold(Column::new().spacing(2), |col, cache| {
                col.push(
                    text(cache.label())
                        .size(10)
                        .color(Color::from_rgb(0.6, 0.6, 0.6)),
                )
            })
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
