// Status tab UI.
use iced::widget::tooltip::Position as TooltipPosition;
use iced::widget::{
    button, column, container, progress_bar, row, scrollable, stack, text, tooltip, Column, Row,
    Space,
};
use iced::{Alignment, Background, Color, Element, Length};

use crate::core::topology::TopologyView;
use crate::gui::topology_diagram::{
    build_core_group_map, draw_topology_group, group_color, group_section_color,
};
use crate::gui::widgets::{color_swatch, colored_button_style, icon, stat_badge};
use crate::gui::{AppCache, Message as AppMessage};

#[derive(Debug, Clone)]
pub enum Message {
    ToggleScanner,
    ToggleAutoMode,
    RequestFreshScan,
    OpenInaccessibleList,
}

// `'a` is a lifetime parameter (roughly: how long borrowed data stays valid),
// similar in intent to C# reference-safety constraints but checked at compile time.
pub fn view<'a>(
    cache: &'a AppCache,
    topo_view: &'a TopologyView,
    num_cores: usize,
    topology_group_repeat: usize,
) -> container::Container<'a, AppMessage> {
    // Button colors encode runtime state at a glance.
    const GREEN: Color = Color::from_rgb(0.13, 0.56, 0.30);
    const RED: Color = Color::from_rgb(0.72, 0.18, 0.18);
    const GREY: Color = Color::from_rgb(0.22, 0.22, 0.28);

    // `if` is an expression in Rust, so it directly returns a value for `let`.
    let scanner_col = if cache.is_scanner_active { GREEN } else { RED };
    let auto_col = if cache.is_auto_mode { GREEN } else { GREY };

    let scanner_btn = button(
        container(
            // `row![...]` is a macro (`!`) that expands into widget-building code.
            row![
                text(if cache.is_scanner_active {
                    "Pause"
                } else {
                    "Resume"
                })
                .size(13),
                icon(if cache.is_scanner_active {
                    iced_fonts::bootstrap::pause_fill()
                } else {
                    iced_fonts::bootstrap::play_fill()
                }),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .center_x(Length::Fill),
    )
    // `Enum::Variant(...)` is namespaced variant construction (like C# `Type.Member`).
    .on_press(AppMessage::StatusMessage(Message::ToggleScanner))
    .width(Length::Fixed(120.0))
    .style(colored_button_style(scanner_col));

    let auto_btn = button(
        container(
            row![
                text("Auto Mode").size(13),
                icon(if cache.is_auto_mode {
                    iced_fonts::bootstrap::toggle_on()
                } else {
                    iced_fonts::bootstrap::toggle_off()
                }),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .center_x(Length::Fill),
    )
    .on_press(AppMessage::StatusMessage(Message::ToggleAutoMode))
    .width(Length::Fixed(120.0))
    .style(colored_button_style(auto_col));

    let scan_btn = button(
        container(
            row![
                text("Fresh Scan").size(13),
                icon(iced_fonts::bootstrap::arrow_clockwise()),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .center_x(Length::Fill),
    )
    .on_press(AppMessage::StatusMessage(Message::RequestFreshScan))
    .width(Length::Fixed(120.0))
    .style(colored_button_style(GREY));

    let top_bar = row![
        scanner_btn,
        Space::new().width(8.0),
        auto_btn,
        Space::new().width(8.0),
        scan_btn
    ]
    .align_y(Alignment::Center);

    // "Assigned" in the status badges means successfully managed process count
    // (i.e., changes actually applied), not just configured/effective matches.
    let assigned_count = cache.managed_count;

    // Top counters summarize what the scanner sees right now:
    // - Total: all visible running process names.
    // - Assigned: running names that map to configured management rules.
    // - Inaccessible: processes skipped due to permission limits.
    // - Groups: configured process groups available for assignment.
    let inaccessible_badge = {
        let badge = stat_badge(
            "Inaccessible",
            cache.protected_count,
            Color::from_rgb(1.0, 0.5, 0.5),
        );

        let btn = button(container(badge).padding([0, 2]))
            .on_press(AppMessage::StatusMessage(Message::OpenInaccessibleList))
            .style(|_, _| button::Style {
                background: None,
                text_color: Color::WHITE,
                border: iced::Border::default(),
                shadow: iced::Shadow::default(),
                snap: false,
            });

        tooltip(
            btn,
            container(
                text("Click to see a list")
                    .size(12)
                    .color(Color::from_rgb(0.88, 0.88, 0.88))
                    .width(Length::Shrink),
            )
            .width(Length::Shrink),
            TooltipPosition::Top,
        )
        .gap(10)
        .padding(10)
        .snap_within_viewport(true)
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(Color::from_rgb(0.11, 0.11, 0.11))),
            border: iced::Border {
                color: Color::from_rgb(0.40, 0.40, 0.40),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
    };

    let stats_row = row![
        stat_badge("Total", cache.running.len(), Color::from_rgb(0.5, 0.8, 1.0)),
        Space::new().width(12.0),
        stat_badge("Assigned", assigned_count, Color::from_rgb(0.5, 1.0, 0.5)),
        Space::new().width(12.0),
        inaccessible_badge,
        Space::new().width(12.0),
        stat_badge(
            "Groups",
            cache.groups.len(),
            Color::from_rgb(1.0, 0.84, 0.0)
        ),
    ]
    .align_y(Alignment::Center);

    let cpu_bar = row![
        text("CPU Total:").size(14).font(iced::Font {
            weight: iced::font::Weight::Bold,
            // `..Default::default()` fills remaining struct fields from defaults.
            ..Default::default()
        }),
        // `a..=b` is an inclusive range (`b` is included).
        progress_bar(0.0..=100.0, cache.cpu_stats.global).length(Length::Fixed(300.0)),
        text(format!("{:.0}%", cache.cpu_stats.global)).size(13),
    ]
    .align_y(Alignment::Center)
    .spacing(10);

    // Precompute core -> group for fast topology coloring and badges.
    let core_group_map = build_core_group_map(&cache.groups, num_cores);

    let repeat = topology_group_repeat.max(1);
    let rendered_group_count = topo_view.groups.len() * repeat;
    let groups_per_row = match rendered_group_count {
        0 => 1,
        1..=3 => rendered_group_count,
        4 => 2,
        _ => 3,
    };

    let mut topology_elements: Vec<Element<AppMessage>> = Vec::new();
    for rep in 0..repeat {
        for (gi, topo_group) in topo_view.groups.iter().enumerate() {
            let color_idx = rep * topo_view.groups.len() + gi;
            topology_elements.push(draw_topology_group(
                topo_group,
                &cache.cpu_stats,
                &core_group_map,
                group_section_color(color_idx),
            ));
        }
    }

    // Layout rules:
    // 1-3 groups: one row
    // 4 groups: 2x2
    // 5-6 groups: 3 columns x 2 rows
    // 7+ groups: 3 columns x N rows
    let topology_widget = container({
        let mut elements = topology_elements.into_iter();
        let mut rows = Column::new().spacing(3);

        while let Some(first) = elements.next() {
            let mut row = Row::new().spacing(3);
            row = row.push(container(first).width(Length::FillPortion(1)));

            for _ in 1..groups_per_row {
                if let Some(next_group) = elements.next() {
                    row = row.push(container(next_group).width(Length::FillPortion(1)));
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
    })
    .padding(10)
    .style(|_| iced::widget::container::Style::default());

    let mut legend_row = Row::new().spacing(10).align_y(Alignment::Center);

    legend_row = legend_row.push(color_swatch(
        Color::from_rgb(0.31, 0.31, 0.31),
        "No group".to_string(),
    ));

    // Legend mirrors group colors used in the topology cards.
    // Prefixes: [BL] blacklist group, [D] default fallback group.
    for (gi, g) in cache.groups.iter().enumerate() {
        let lbl = if g.is_blacklist {
            format!("[BL] {}", g.name)
        } else if g.is_default {
            format!("[D] {}", g.name)
        } else {
            g.name.clone()
        };
        legend_row = legend_row.push(color_swatch(group_color(gi), lbl));
    }

    let content = column![
        top_bar,
        stats_row,
        cpu_bar,
        scrollable(topology_widget).height(Length::Fill),
        container(legend_row).padding(10),
    ]
    .spacing(10)
    .padding(10);

    let version_label = container(
        text(env!("APP_VERSION"))
            .size(11)
            .color(Color::from_rgba(1.0, 1.0, 1.0, 0.25)),
    )
    .padding(iced::Padding {
        top: 0.0,
        right: 8.0,
        bottom: 6.0,
        left: 0.0,
    })
    .align_right(Length::Fill)
    .align_bottom(Length::Fill);

    container(stack![container(content), version_label])
}
