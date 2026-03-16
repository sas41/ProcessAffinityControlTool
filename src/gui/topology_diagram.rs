use iced::widget::{button, column, container, text, Column, Row, Space};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::core::process_config::ProcessGroup;
use crate::core::topology::format_freq_ghz;
use crate::gui::Message;

fn soft_group_background(accent: Color) -> Color {
    Color::from_rgb(
        (accent.r * 0.14 + 0.07).min(1.0),
        (accent.g * 0.14 + 0.07).min(1.0),
        (accent.b * 0.14 + 0.07).min(1.0),
    )
}

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
    let cores_per_row = 4usize;

    let core_widgets: Vec<Element<'a, Message>> = group
        .physical_cores
        .iter()
        // C# note: `|x| { ... }` is a closure/lambda.
        .map(|phys_core| {
            let thread_widgets: Vec<Element<'a, Message>> = phys_core
                .threads
                .iter()
                .take(2)
                .map(|thread| {
                    let usage = stats
                        .per_core
                        .get(thread.logical_index)
                        .copied()
                        .unwrap_or(0.0);
                    let fill_col = core_group_map
                        .get(thread.logical_index)
                        // C# note: `|&g| g` destructures `&Option<usize>` to `Option<usize>`.
                        .and_then(|&g| g)
                        .map_or(Color::from_rgb(0.36, 0.36, 0.36), group_color);

                    let thread_cell = column![
                        container(
                            column![
                                text(format!("T{}", thread.logical_index))
                                    .size(10)
                                    .color(Color::from_rgb(0.85, 0.85, 0.85)),
                                text(format!("{usage:.0}%"))
                                    .size(9)
                                    .color(Color::from_rgb(0.67, 0.67, 0.67)),
                            ]
                            .height(Length::Fill)
                            .align_x(Alignment::Center)
                            .spacing(3),
                        )
                        .height(Length::Fill)
                        .center_x(Length::Fill)
                        .center_y(Length::Fill),
                        container("").height(Length::Fixed(2.0)).style(move |_| {
                            iced::widget::container::Style {
                                background: Some(Background::Color(fill_col)),
                                ..Default::default()
                            }
                        })
                    ]
                    .spacing(0);

                    container(thread_cell)
                        .width(Length::Fill)
                        .height(Length::Fixed(45.0))
                        .style(|_| iced::widget::container::Style {
                            border: Border {
                                color: Color::from_rgb(0.30, 0.30, 0.30),
                                width: 1.0,
                                radius: 3.0.into(),
                            },
                            background: Some(Background::Color(Color::from_rgb(0.12, 0.12, 0.12))),
                            ..Default::default()
                        })
                        .into()
                })
                .collect();

            let thread_cols = thread_widgets.len().max(1);
            let thread_cells =
                thread_widgets
                    .into_iter()
                    .fold(Row::new().spacing(4), |row, thread_widget| {
                        row.push(container(thread_widget).width(Length::FillPortion(1)))
                    });

            let mut core_meta = Column::new().spacing(2).align_x(Alignment::Center).push(
                text(format!(
                    "C{} {}",
                    phys_core.physical_index,
                    format_freq_ghz(phys_core.max_freq_khz)
                ))
                .size(10)
                .color(Color::from_rgb(0.68, 0.68, 0.68)),
            );

            for cache in &phys_core.private_caches {
                core_meta = core_meta.push(
                    text(cache.label())
                        .size(9)
                        .color(Color::from_rgb(0.55, 0.55, 0.55)),
                );
            }

            container(
                column![thread_cells, core_meta]
                    .spacing(3)
                    .align_x(Alignment::Center),
            )
            .width(Length::FillPortion(thread_cols as u16))
            .padding(4)
            .style(|_| iced::widget::container::Style {
                border: Border {
                    color: Color::from_rgb(0.30, 0.30, 0.30),
                    width: 1.0,
                    radius: 5.0.into(),
                },
                background: Some(Background::Color(Color::from_rgb(0.11, 0.11, 0.11))),
                ..Default::default()
            })
            .into()
        })
        .collect();

    let core_grid = {
        let mut elements = core_widgets.into_iter();
        let mut rows = Column::new().spacing(4);

        while let Some(first) = elements.next() {
            let mut row = Row::new().spacing(4);
            row = row.push(container(first).width(Length::FillPortion(1)));

            for _ in 1..cores_per_row {
                if let Some(next_core) = elements.next() {
                    row = row.push(container(next_core).width(Length::FillPortion(1)));
                } else {
                    row = row.push(
                        Space::new()
                            .width(Length::FillPortion(1))
                            .height(Length::Shrink),
                    );
                }
            }

            rows = rows.push(row);
        }

        rows
    };

    let footer = column![
        text(group.label.as_str()).size(12).color(stroke_col),
        text(format!(
            "Max {}",
            format_freq_ghz(
                group
                    .physical_cores
                    .iter()
                    .map(|c| c.max_freq_khz)
                    .max()
                    .unwrap_or(0)
            )
        ))
        .size(10)
        .color(Color::from_rgb(0.72, 0.72, 0.72)),
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
            .spacing(6)
            .align_x(Alignment::Center),
    )
    .padding(6)
    .style(move |_| iced::widget::container::Style {
        border: Border {
            color: stroke_col,
            width: 1.0,
            radius: 8.0.into(),
        },
        background: Some(Background::Color(soft_group_background(stroke_col))),
        ..Default::default()
    })
    .into()
}

/// Compact topology-based core selector used by editors.
pub fn draw_core_selector<Message: Clone + 'static>(
    topo_view: &crate::core::topology::TopologyView,
    core_checks: &[bool],
    topology_group_repeat: usize,
    on_toggle: fn(usize, bool) -> Message,
) -> Column<'static, Message> {
    let repeat = topology_group_repeat.max(1);
    let total_groups = topo_view.groups.len() * repeat;
    let groups_per_row = if total_groups <= 2 {
        total_groups.max(1)
    } else {
        2
    };
    let mut group_cards: Vec<Element<'static, Message>> = Vec::new();

    for rep in 0..repeat {
        for (gi, group) in topo_view.groups.iter().enumerate() {
            let color_idx = rep * topo_view.groups.len() + gi;
            let mut core_rows = Column::new().spacing(4);
            let mut core_iter = group.physical_cores.iter();

            while let Some(first_core) = core_iter.next() {
                let mut row = Row::new().spacing(4);
                let mut cells_in_row = 0usize;

                for core in std::iter::once(first_core).chain(core_iter.by_ref().take(3)) {
                    let mut thread_row = Row::new().spacing(3);
                    let thread_columns = if core.threads.len() >= 2 { 2 } else { 1 };

                    for thread in core.threads.iter().take(thread_columns) {
                        let selected = core_checks
                            .get(thread.logical_index)
                            .copied()
                            .unwrap_or(false);

                        let chip_width = if thread_columns == 2 {
                            Length::FillPortion(1)
                        } else {
                            Length::Fill
                        };

                        thread_row = thread_row.push(
                            button(
                                container(text(format!("T{}", thread.logical_index)).size(10))
                                    .width(Length::Fill)
                                    .center_x(Length::Fill),
                            )
                            .on_press(on_toggle(thread.logical_index, !selected))
                            .width(chip_width)
                            .padding([2, 4])
                            .style(move |_, _| button::Style {
                                background: Some(Background::Color(if selected {
                                    group_color(color_idx)
                                } else {
                                    Color::from_rgb(0.20, 0.20, 0.20)
                                })),
                                text_color: if selected {
                                    Color::BLACK
                                } else {
                                    Color::from_rgb(0.82, 0.82, 0.82)
                                },
                                border: Border {
                                    color: Color::from_rgb(0.34, 0.34, 0.34),
                                    width: 1.0,
                                    radius: 3.0.into(),
                                },
                                ..Default::default()
                            }),
                        );
                    }

                    row = row.push(
                        container(
                            column![
                                text(format!("C{}", core.physical_index)).size(10),
                                thread_row
                            ]
                            .spacing(2)
                            .align_x(Alignment::Center),
                        )
                        .padding(4)
                        .width(Length::FillPortion(1))
                        .style(|_| iced::widget::container::Style {
                            border: Border {
                                color: Color::from_rgb(0.32, 0.32, 0.32),
                                width: 1.0,
                                radius: 4.0.into(),
                            },
                            background: Some(Background::Color(Color::from_rgb(0.11, 0.11, 0.11))),
                            ..Default::default()
                        }),
                    );
                    cells_in_row += 1;
                }

                while cells_in_row < 4 {
                    row = row.push(Space::new().width(Length::FillPortion(1)));
                    cells_in_row += 1;
                }

                core_rows = core_rows.push(row);
            }

            let label = if repeat == 1 {
                group.label.clone()
            } else {
                format!("{} #{}", group.label, rep + 1)
            };

            group_cards.push(
                container(
                    column![
                        text(label).size(11).color(group_section_color(color_idx)),
                        core_rows
                    ]
                    .spacing(4),
                )
                .padding(6)
                .style(move |_| iced::widget::container::Style {
                    border: Border {
                        color: group_section_color(color_idx),
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    background: Some(Background::Color(soft_group_background(
                        group_section_color(color_idx),
                    ))),
                    ..Default::default()
                })
                .into(),
            );
        }
    }

    let mut grid = Column::new().spacing(8);
    let mut cards = group_cards.into_iter();

    while let Some(first) = cards.next() {
        let mut row = Row::new().spacing(8);
        row = row.push(container(first).width(Length::FillPortion(1)));

        for _ in 1..groups_per_row {
            if let Some(next) = cards.next() {
                row = row.push(container(next).width(Length::FillPortion(1)));
            } else {
                row = row.push(Space::new().width(Length::FillPortion(1)));
            }
        }

        grid = grid.push(row);
    }

    grid
}
