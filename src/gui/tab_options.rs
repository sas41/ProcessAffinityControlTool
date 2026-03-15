use iced::font;
use iced::widget::{
    Column, Container, Space, button, checkbox, column, container, row, scrollable, slider, text,
};
use iced::{Alignment, Background, Color, Font, Length};

use crate::gui::priority::{PRIORITY_LABELS, priority_to_index};
use crate::gui::topology_diagram::group_color;
use crate::gui::{AppCache, Message as AppMessage};

#[derive(Debug, Clone)]
pub enum Message {
    SetScanInterval(f32),
    ResetConfig,
    OpenGitHub,
    SetMinimizeToTray(bool),
}

pub fn view<'a>(cache: &'a AppCache, num_cores: usize) -> Container<'a, AppMessage> {
    let scan_ms = cache.scan_interval as f32;

    // ── Scan Interval ─────────────────────────────────────────────────────────
    let interval_section = column![
        text("Scan Interval").size(16).font(Font {
            weight: font::Weight::Bold,
            ..Default::default()
        }),
        row![
            slider(500.0f32..=10000.0, scan_ms, |v| {
                AppMessage::OptionsMessage(Message::SetScanInterval(v))
            })
            .step(100.0f32)
            .width(Length::Fixed(220.0)),
            Space::with_width(8.0),
            text(format!("{} ms", scan_ms as u64)).size(13),
        ]
        .align_y(Alignment::Center)
        .spacing(4),
    ]
    .spacing(8);

    // ── Configuration ─────────────────────────────────────────────────────────
    let config_section = column![
        text("Configuration").size(16).font(Font {
            weight: font::Weight::Bold,
            ..Default::default()
        }),
        row![
            button(text("Import Config…").size(13)).on_press(AppMessage::ImportConfig),
            button(text("Export Config…").size(13)).on_press(AppMessage::ExportConfig),
            button(text("Reset to Defaults").size(13))
                .on_press(AppMessage::OptionsMessage(Message::ResetConfig)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ]
    .spacing(8);

    // ── CPU Topology ──────────────────────────────────────────────────────────
    let topology_section = column![
        text("CPU Topology").size(16).font(Font {
            weight: font::Weight::Bold,
            ..Default::default()
        }),
        text(format!("Logical processors:  {}", num_cores))
            .size(13)
            .color(Color::from_rgb(0.75, 0.75, 0.75)),
    ]
    .spacing(8);

    // ── Groups overview ───────────────────────────────────────────────────────
    let italic = Font {
        style: iced::font::Style::Italic,
        ..Default::default()
    };

    let header_row = row![
        text("Name")
            .size(13)
            .font(Font {
                weight: font::Weight::Bold,
                ..Default::default()
            })
            .width(Length::Fixed(130.0)),
        text("Affinity")
            .size(13)
            .font(Font {
                weight: font::Weight::Bold,
                ..Default::default()
            })
            .width(Length::Fixed(80.0)),
        text("Priority")
            .size(13)
            .font(Font {
                weight: font::Weight::Bold,
                ..Default::default()
            })
            .width(Length::Fixed(80.0)),
        text("Flags")
            .size(13)
            .font(Font {
                weight: font::Weight::Bold,
                ..Default::default()
            })
            .width(Length::Fixed(80.0)),
        text("Processes").size(13).font(Font {
            weight: font::Weight::Bold,
            ..Default::default()
        }),
    ]
    .spacing(0);

    // Pre-count assigned processes per group — O(M) instead of O(N×M).
    let mut proc_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for (_, grp) in &cache.assigned {
        *proc_counts.entry(grp.to_lowercase()).or_default() += 1;
    }

    let group_rows =
        cache
            .groups
            .iter()
            .enumerate()
            .fold(Column::new().spacing(4), |col, (gi, g)| {
                let affinity_str = match &g.affinity {
                    Some(aff) => format!("{} cores", aff.core_list.len()),
                    None => "ignore".to_string(),
                };
                let priority_str = match &g.priority {
                    Some(p) => PRIORITY_LABELS[priority_to_index(p)].to_string(),
                    None => "ignore".to_string(),
                };
                let flags_str = if g.is_default {
                    "default".to_string()
                } else if g.is_blacklist {
                    "blacklist".to_string()
                } else {
                    "-".to_string()
                };
                let proc_count = proc_counts
                    .get(&g.name.to_lowercase())
                    .copied()
                    .unwrap_or(0);
                let gc = group_color(gi);

                col.push(
                    row![
                        text(g.name.clone())
                            .size(13)
                            .color(gc)
                            .width(Length::Fixed(130.0)),
                        text(affinity_str)
                            .size(13)
                            .font(italic)
                            .color(Color::from_rgb(0.65, 0.65, 0.65))
                            .width(Length::Fixed(80.0)),
                        text(priority_str)
                            .size(13)
                            .font(italic)
                            .color(Color::from_rgb(0.65, 0.65, 0.65))
                            .width(Length::Fixed(80.0)),
                        text(flags_str)
                            .size(13)
                            .color(Color::from_rgb(0.65, 0.65, 0.65))
                            .width(Length::Fixed(80.0)),
                        text(proc_count.to_string())
                            .size(13)
                            .color(Color::from_rgb(0.65, 0.65, 0.65)),
                    ]
                    .spacing(0),
                )
            });

    let groups_section = column![
        text("Groups overview").size(16).font(Font {
            weight: font::Weight::Bold,
            ..Default::default()
        }),
        header_row,
        group_rows,
    ]
    .spacing(8);

    let divider = || {
        container(Space::with_height(1.0))
            .width(Length::Fill)
            .style(|_| iced::widget::container::Style {
                background: Some(Background::Color(Color::from_rgb(0.25, 0.25, 0.25))),
                ..Default::default()
            })
    };

    let github_btn =
        button(text("GitHub").size(13)).on_press(AppMessage::OptionsMessage(Message::OpenGitHub));

    let behavior_section = column![
        text("Behavior").size(16).font(Font {
            weight: iced::font::Weight::Bold,
            ..Default::default()
        }),
        checkbox("Minimize to tray on close", cache.minimize_to_tray)
            .on_toggle(|v| AppMessage::OptionsMessage(Message::SetMinimizeToTray(v))),
    ]
    .spacing(8);

    let content = column![
        interval_section,
        divider(),
        behavior_section,
        divider(),
        config_section,
        divider(),
        topology_section,
        divider(),
        groups_section,
        divider(),
        github_btn,
    ]
    .spacing(16)
    .padding(14);

    container(scrollable(content).height(Length::Fill))
}
