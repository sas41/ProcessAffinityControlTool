use iced::widget::{
    Column, Row, Space, button, column, container, progress_bar, row, scrollable, text,
};
use iced::{Alignment, Color, Element, Length};

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
}

pub fn view<'a>(
    cache: &'a AppCache,
    topo_view: &'a TopologyView,
    num_cores: usize,
) -> container::Container<'a, AppMessage> {
    // ── Top control bar ───────────────────────────────────────────────────────
    // Pleasant green / red for scanner; green / neutral for auto mode.
    const GREEN: Color = Color::from_rgb(0.13, 0.56, 0.30);
    const RED: Color = Color::from_rgb(0.72, 0.18, 0.18);
    const GREY: Color = Color::from_rgb(0.22, 0.22, 0.28);

    let scanner_col = if cache.is_scanner_active { GREEN } else { RED };
    let auto_col = if cache.is_auto_mode { GREEN } else { GREY };

    let scanner_btn = button(
        container(
            row![
                text(if cache.is_scanner_active {
                    "Pause"
                } else {
                    "Resume"
                })
                .size(13),
                icon(if cache.is_scanner_active {
                    iced_fonts::Bootstrap::PauseFill
                } else {
                    iced_fonts::Bootstrap::PlayFill
                }),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .center_x(Length::Fill),
    )
    .on_press(AppMessage::StatusMessage(Message::ToggleScanner))
    .width(Length::Fixed(120.0))
    .style(colored_button_style(scanner_col));

    let auto_btn = button(
        container(
            row![
                text("Auto Mode").size(13),
                icon(if cache.is_auto_mode {
                    iced_fonts::Bootstrap::ToggleOn
                } else {
                    iced_fonts::Bootstrap::ToggleOff
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
                icon(iced_fonts::Bootstrap::ArrowClockwise),
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
        Space::with_width(8.0),
        auto_btn,
        Space::with_width(8.0),
        scan_btn
    ]
    .align_y(Alignment::Center);

    // ── Stats ─────────────────────────────────────────────────────────────────
    let assigned_count = {
        let managed_names: std::collections::HashSet<&str> = cache
            .assigned
            .iter()
            .map(|(n, _)| n.as_str())
            .chain(cache.custom_processes.iter().map(|cp| cp.name.as_str()))
            .collect();
        cache
            .running
            .iter()
            .filter(|n| managed_names.contains(n.as_str()))
            .count()
    };

    let stats_row = row![
        stat_badge("Total", cache.running.len(), Color::from_rgb(0.5, 0.8, 1.0)),
        Space::with_width(12.0),
        stat_badge("Assigned", assigned_count, Color::from_rgb(0.5, 1.0, 0.5)),
        Space::with_width(12.0),
        stat_badge(
            "Inaccessible",
            cache.protected_count,
            Color::from_rgb(1.0, 0.5, 0.5)
        ),
        Space::with_width(12.0),
        stat_badge(
            "Groups",
            cache.groups.len(),
            Color::from_rgb(1.0, 0.84, 0.0)
        ),
    ]
    .align_y(Alignment::Center);

    // ── CPU bar ───────────────────────────────────────────────────────────────
    let cpu_bar = row![
        text("CPU Total:").size(14).font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }),
        progress_bar(0.0..=100.0, cache.cpu_stats.global).width(Length::Fixed(300.0)),
        text(format!("{:.0}%", cache.cpu_stats.global)).size(13),
    ]
    .align_y(Alignment::Center)
    .spacing(10);

    // ── Topology diagram ──────────────────────────────────────────────────────
    let core_group_map = build_core_group_map(&cache.groups, num_cores);

    let topology_elements: Vec<Element<AppMessage>> = topo_view
        .groups
        .iter()
        .enumerate()
        .map(|(gi, topo_group)| {
            draw_topology_group(
                topo_group,
                &cache.cpu_stats,
                &core_group_map,
                group_section_color(gi),
            )
        })
        .collect();

    let topology_widget = container({
        let mut elements = topology_elements.into_iter();
        let mut rows = Column::new().spacing(10);
        while let Some(first) = elements.next() {
            let mut row = Row::new().spacing(10);
            row = row.push(first);
            if let Some(second) = elements.next() {
                row = row.push(second);
            }
            rows = rows.push(row);
        }
        rows
    })
    .padding(10)
    .style(|_| iced::widget::container::Style {
        background: Some(iced::Background::Color(Color::from_rgb(0.05, 0.05, 0.05))),
        ..Default::default()
    });

    // ── Legend ────────────────────────────────────────────────────────────────
    let mut legend_row = Row::new().spacing(10).align_y(Alignment::Center);
    legend_row = legend_row.push(color_swatch(
        Color::from_rgb(0.31, 0.31, 0.31),
        "No group".to_string(),
    ));
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

    container(content)
}
