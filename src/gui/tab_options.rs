//! Options-tab UI: user-facing settings and quick status info.

use iced::font;

use iced::widget::{
    button, checkbox, column, container, row, scrollable, slider, stack, text, Column, Container,
    Space,
};

use iced::{Alignment, Background, Color, Font, Length};

#[cfg(target_os = "linux")]
use crate::gui::priority::priority_to_niceness;
use crate::gui::priority::{priority_to_index, PRIORITY_LABELS};
use crate::gui::topology_diagram::group_color;
use crate::gui::{AppCache, Message as AppMessage};

/// User actions emitted from the Options tab.
#[derive(Debug, Clone)]
pub enum Message {
    /// Slider changed: update scan interval (milliseconds).
    SetScanInterval(f32),

    /// Button pressed: restore built-in default config values.
    ResetConfig,

    /// Button pressed: open the project GitHub page.
    OpenGitHub,

    /// Toggle whether app should start hidden to tray.
    SetLaunchMinimized(bool),
}

/// Builds the full Options tab view from top to bottom sections.
/// `<'a>` is a lifetime parameter (borrow scope), not a generic type.
pub fn view<'a>(cache: &'a AppCache, num_cores: usize) -> Container<'a, AppMessage> {
    let scan_ms = cache.scan_interval as f32;

    // Section 1: scan interval controls.
    // `column![...]` uses a macro (`!`) to build a widget tree.
    let interval_section = column![
        text("Scan Interval").size(16).font(Font {
            weight: font::Weight::Bold,
            // Struct update syntax: keep all other fields at default values.
            ..Default::default()
        }),
        row![
            // `a..=b` is an inclusive range; `|v| { ... }` is a closure (lambda).
            slider(500.0f32..=10000.0, scan_ms, |v| {
                AppMessage::OptionsMessage(Message::SetScanInterval(v))
            })
            .step(100.0f32)
            .width(Length::Fixed(220.0)),
            Space::new().width(8.0),
            text(format!("{} ms", scan_ms as u64)).size(13),
        ]
        .align_y(Alignment::Center)
        .spacing(4),
    ]
    .spacing(8);

    // Section 2: import/export/reset config actions.
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
        checkbox(cache.launch_minimized)
            .label("Launch minimized")
            .on_toggle(|v| AppMessage::OptionsMessage(Message::SetLaunchMinimized(v))),
    ]
    .spacing(8);

    // Section 3: read-only CPU topology details.
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

    // Section 4: groups overview table.
    let italic = Font {
        style: iced::font::Style::Italic,
        ..Default::default()
    };

    let mut header_row = row![
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
    ]
    .spacing(0);

    #[cfg(target_os = "linux")]
    {
        header_row = header_row.push(
            text("Nice")
                .size(13)
                .font(Font {
                    weight: font::Weight::Bold,
                    ..Default::default()
                })
                .width(Length::Fixed(60.0)),
        );
    }

    header_row = header_row
        .push(
            text("Flags")
                .size(13)
                .font(Font {
                    weight: font::Weight::Bold,
                    ..Default::default()
                })
                .width(Length::Fixed(80.0)),
        )
        .push(text("Processes").size(13).font(Font {
            weight: font::Weight::Bold,
            ..Default::default()
        }));

    // Pre-count assigned processes per group for the table.
    let mut proc_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    // `_` ignores an unused tuple item; `&` borrows instead of moving.
    for assigned in &cache.assigned {
        // `entry(...).or_default()` inserts missing key with `usize::default()` (0).
        *proc_counts
            .entry(assigned.group.to_lowercase())
            .or_default() += 1;
    }

    let group_rows = cache
        .groups
        .iter()
        .enumerate()
        // `fold(init, |acc, item| ...)` reduces iterator items into one value.
        .fold(Column::new().spacing(4), |col, (gi, g)| {
            let affinity_str = match &g.affinity {
                Some(aff) => format!("{} cores", aff.core_list.len()),
                None => "ignore".to_string(),
            };

            let priority_str = match &g.priority {
                Some(p) => PRIORITY_LABELS[priority_to_index(p)].to_string(),
                None => "ignore".to_string(),
            };

            #[cfg(target_os = "linux")]
            let niceness_str = match g.niceness {
                Some(v) => v.to_string(),
                None => g
                    .priority
                    .as_ref()
                    .map(priority_to_niceness)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "-".to_string()),
            };

            let has_affinity = g.affinity.is_some();
            let has_priority = g.priority.is_some() || g.niceness.is_some();
            let flags_str = if g.is_default {
                "default".to_string()
            } else if !has_affinity && !has_priority {
                "blacklist".to_string()
            } else {
                "-".to_string()
            };

            let proc_count = proc_counts
                .get(&g.name.to_lowercase())
                .copied()
                .unwrap_or(0);

            let gc = group_color(gi);

            let mut group_row = row![
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
            ]
            .spacing(0);

            #[cfg(target_os = "linux")]
            {
                group_row = group_row.push(
                    text(niceness_str)
                        .size(13)
                        .font(italic)
                        .color(Color::from_rgb(0.65, 0.65, 0.65))
                        .width(Length::Fixed(60.0)),
                );
            }

            group_row = group_row
                .push(
                    text(flags_str)
                        .size(13)
                        .color(Color::from_rgb(0.65, 0.65, 0.65))
                        .width(Length::Fixed(80.0)),
                )
                .push(
                    text(proc_count.to_string())
                        .size(13)
                        .color(Color::from_rgb(0.65, 0.65, 0.65)),
                );

            col.push(group_row)
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

    // Shared horizontal divider used between sections.
    let divider = || {
        container(Space::new().height(1.0))
            .width(Length::Fill)
            .style(|_| iced::widget::container::Style {
                background: Some(Background::Color(Color::from_rgb(0.25, 0.25, 0.25))),
                ..Default::default()
            })
    };

    let github_btn =
        button(text("GitHub").size(13)).on_press(AppMessage::OptionsMessage(Message::OpenGitHub));

    let content = column![
        interval_section,
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

    container(stack![
        container(scrollable(content).height(Length::Fill)),
        version_label
    ])
}
