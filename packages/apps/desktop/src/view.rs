//! Screens, panels and widgets of the desktop app_

use iced::widget::{
    Column, Row, button, column, container, image, row, scrollable, slider, text, text_input,
};
use iced::{Element, Length};
use multideck_actions as actions;
use multideck_i18n::{Locale, tr, tr_args};
use multideck_ui::Colors;

use crate::app::{App, Message, Screen};

const KEY_PIXELS: f32 = 88.0;

/// Root view.
pub fn view(app: &App) -> Element<'_, Message> {
    let content = match app.screen {
        Screen::Editor => editor(app),
        Screen::Settings => settings(app),
    };
    container(row![sidebar(app), content].spacing(0))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(multideck_ui::page())
        .into()
}

fn spacer() -> Element<'static, Message> {
    container(text("")).width(Length::Fill).into()
}

fn vertical_spacer() -> Element<'static, Message> {
    container(text(""))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn section(title: impl ToString) -> Element<'static, Message> {
    text(title.to_string())
        .size(11)
        .color(Colors::MUTED)
        .width(Length::Fill)
        .into()
}

fn sidebar(app: &App) -> Element<'_, Message> {
    let is_virtual = app.device_descriptor.transport == multideck_core::Transport::Virtual;
    let device = if is_virtual {
        tr!("devices_virtual")
    } else {
        app.device_descriptor.name.clone()
    };
    let model = if is_virtual {
        tr!("devices_virtual_model")
    } else {
        app.device_descriptor.model.clone()
    };
    let connected = tr!("devices_connected");

    let mut profile_buttons: Vec<Element<'_, Message>> = Vec::new();
    for profile in &app.profiles {
        let active = profile.id == app.profile.id;
        profile_buttons.push(
            button(text(profile.name.clone()).size(13))
                .on_press(Message::SelectProfile(profile.id.clone()))
                .width(Length::Fill)
                .padding(8)
                .style(multideck_ui::toggle_button(active))
                .into(),
        );
    }

    let device_card = container(
        column![
            text(device).size(14),
            text(model).size(10).color(Colors::MUTED),
            text(connected).size(10).color(Colors::SUCCESS),
            slider(0..=100u8, app.profile.brightness, Message::SetBrightness)
                .step(1u8)
                .width(Length::Fill),
        ]
        .spacing(4),
    )
    .padding(10)
    .width(Length::Fill)
    .style(multideck_ui::card());

    let body = column![
        column![
            text(tr!("app_name")).size(20),
            text(tr!("app_tagline")).size(10).color(Colors::MUTED),
        ]
        .spacing(2),
        section(tr!("nav_devices")),
        device_card,
        section(tr!("profiles_title")),
        Column::with_children(profile_buttons).spacing(4),
        button(text(tr!("profiles_new")).size(12))
            .on_press(Message::NewProfile)
            .width(Length::Fill)
            .padding(6)
            .style(multideck_ui::ghost_button()),
        spacer(),
        button(text(tr!("nav_editor")).size(13))
            .on_press(Message::ShowEditor)
            .width(Length::Fill)
            .padding(8)
            .style(multideck_ui::toggle_button(app.screen == Screen::Editor)),
        button(text(tr!("nav_settings")).size(13))
            .on_press(Message::ShowSettings)
            .width(Length::Fill)
            .padding(8)
            .style(multideck_ui::toggle_button(app.screen == Screen::Settings)),
        text(format!("v{}", env!("CARGO_PKG_VERSION")))
            .size(10)
            .color(Colors::MUTED),
    ]
    .spacing(12);

    container(body)
        .width(Length::Fixed(240.0))
        .height(Length::Fill)
        .padding(16)
        .style(multideck_ui::panel())
        .into()
}

fn editor(app: &App) -> Element<'_, Message> {
    container(
        column![
            header(app),
            row![grid(app), right_panel(app)]
                .spacing(16)
                .height(Length::Fill),
            status_bar(app),
        ]
        .spacing(14),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(18)
    .into()
}

fn header(app: &App) -> Element<'_, Message> {
    let mut chips: Vec<Element<'_, Message>> = Vec::new();
    for (index, page) in app.profile.pages.iter().enumerate() {
        chips.push(
            button(text(page.name.clone()).size(11))
                .on_press(Message::SelectPage(index))
                .padding(6)
                .style(multideck_ui::toggle_button(
                    index == app.profile.active_page,
                ))
                .into(),
        );
    }
    chips.push(
        button(text("+").size(11))
            .on_press(Message::NewPage)
            .padding(6)
            .style(multideck_ui::ghost_button())
            .into(),
    );

    row![
        text(app.profile.name.clone()).size(18),
        Row::with_children(chips).spacing(6),
        spacer(),
        text(tr_args!(
            "status_page_of",
            "current" => app.profile.active_page + 1,
            "total" => app.profile.pages.len()
        ))
        .size(11)
        .color(Colors::MUTED),
    ]
    .spacing(12)
    .into()
}

fn grid(app: &App) -> Element<'_, Message> {
    let cols = app.caps.cols;
    let rows = app.caps.rows;
    let mut lines: Vec<Element<'_, Message>> = Vec::new();

    for row_index in 0..rows {
        let mut cells: Vec<Element<'_, Message>> = Vec::new();
        for col_index in 0..cols {
            let key = row_index * cols + col_index;
            cells.push(key_cell(app, key));
        }
        lines.push(Row::with_children(cells).spacing(8).into());
    }

    let stats = row![
        text(format!(
            "{}: {}",
            tr!("devices_transport"),
            app.device_descriptor.name
        ))
        .size(10)
        .color(Colors::MUTED),
        text(format!("{}: {}", tr!("devices_keys"), app.caps.key_count()))
            .size(10)
            .color(Colors::MUTED),
        text(format!(
            "{}: {}%",
            tr!("devices_brightness"),
            app.profile.brightness
        ))
        .size(10)
        .color(Colors::MUTED),
    ]
    .spacing(16);

    container(
        column![
            vertical_spacer(),
            Column::with_children(lines).spacing(8),
            vertical_spacer(),
            text(tr!("editor_select_key")).size(11).color(Colors::MUTED),
            stats,
        ]
        .spacing(12)
        .height(Length::Fill),
    )
    .padding(16)
    .height(Length::Fill)
    .style(multideck_ui::card())
    .into()
}

fn key_cell(app: &App, key: u8) -> Element<'_, Message> {
    let selected = app.selected_key == Some(key);
    let inner: Element<'_, Message> = match app.key_images.get(key as usize) {
        Some(Some(handle)) => image(handle.clone())
            .width(Length::Fixed(KEY_PIXELS))
            .height(Length::Fixed(KEY_PIXELS))
            .into(),
        _ => container(text(""))
            .width(Length::Fixed(KEY_PIXELS))
            .height(Length::Fixed(KEY_PIXELS))
            .into(),
    };

    container(
        button(inner)
            .on_press(Message::SelectKey(key))
            .padding(2)
            .style(multideck_ui::bare_button()),
    )
    .padding(3)
    .style(multideck_ui::key_frame(selected))
    .into()
}

fn right_panel(app: &App) -> Element<'_, Message> {
    let mut items: Vec<Element<'_, Message>> =
        vec![inspector(app), section(tr!("actions_palette"))];
    for definition in actions::definitions() {
        items.push(palette_item(definition));
    }

    container(scrollable(Column::with_children(items).spacing(8)).height(Length::Fill))
        .width(Length::Fixed(320.0))
        .height(Length::Fill)
        .padding(12)
        .style(multideck_ui::card())
        .into()
}

fn palette_item(definition: &'static multideck_core::ActionDef) -> Element<'static, Message> {
    let label = tr(definition.label_key);
    let description = tr(definition.description_key);
    button(
        column![
            text(label).size(13),
            text(description).size(10).color(Colors::MUTED),
        ]
        .spacing(2),
    )
    .on_press(Message::PickAction(definition.kind))
    .width(Length::Fill)
    .padding(8)
    .style(multideck_ui::ghost_button())
    .into()
}

fn inspector(app: &App) -> Element<'_, Message> {
    let key = app.selected_key.unwrap_or(0);
    let heading = || match app.selected_key {
        Some(selected) => section(tr_args!(
            "inspector_selected",
            "number" => selected + 1
        )),
        None => section(tr!("inspector_title")),
    };

    let Some(action) = app.selected_action() else {
        return column![
            heading(),
            text(tr!("inspector_empty")).size(12).color(Colors::MUTED)
        ]
        .spacing(6)
        .into();
    };

    let Some(definition) = actions::definition(&action.kind) else {
        return text(tr!("inspector_empty"))
            .size(12)
            .color(Colors::MUTED)
            .into();
    };

    let mut fields: Vec<Element<'_, Message>> =
        vec![heading(), text(tr(definition.label_key)).size(15).into()];

    for (index, field) in definition.fields.iter().enumerate() {
        let value = app
            .inspector
            .get(index)
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| field.default.to_string());

        if definition.kind == "profile" && field.key == "profile" {
            let mut chips: Vec<Element<'_, Message>> = Vec::new();
            for profile in &app.profiles {
                chips.push(
                    button(text(profile.name.clone()).size(11))
                        .on_press(Message::FieldChanged(index, profile.id.clone()))
                        .padding(6)
                        .style(multideck_ui::toggle_button(profile.id == value))
                        .into(),
                );
            }
            fields.push(
                column![
                    text(tr(field.label_key)).size(10).color(Colors::MUTED),
                    Row::with_children(chips).spacing(6),
                ]
                .spacing(3)
                .into(),
            );
            continue;
        }

        fields.push(
            column![
                text(tr(field.label_key)).size(10).color(Colors::MUTED),
                text_input("", &value)
                    .on_input(move |input| Message::FieldChanged(index, input))
                    .padding(8)
                    .size(13),
            ]
            .spacing(3)
            .into(),
        );
    }

    if let Some(choices) = definition.fields.iter().find_map(|field| match field.kind {
        multideck_core::FieldKind::Select(values) => Some(values),
        _ => None,
    }) {
        fields.push(
            text(choices.join(" · "))
                .size(10)
                .color(Colors::MUTED)
                .into(),
        );
    }

    fields.push(
        row![
            button(text(tr!("inspector_apply")).size(12))
                .on_press(Message::ApplyInspector)
                .padding(8)
                .style(multideck_ui::primary_button()),
            button(text(tr!("status_press_test")).size(12))
                .on_press(Message::PressKey(key))
                .padding(8)
                .style(multideck_ui::ghost_button()),
            button(text(tr!("inspector_delete")).size(12))
                .on_press(Message::DeleteAction)
                .padding(8)
                .style(multideck_ui::ghost_button()),
        ]
        .spacing(6)
        .into(),
    );

    Column::with_children(fields).spacing(8).into()
}

fn status_bar(app: &App) -> Element<'_, Message> {
    row![
        text(app.status.clone()).size(11).color(Colors::MUTED),
        spacer(),
        text(tr!("devices_press_hint"))
            .size(11)
            .color(Colors::MUTED),
    ]
    .spacing(12)
    .into()
}

fn settings(app: &App) -> Element<'_, Message> {
    let mut languages: Vec<Element<'_, Message>> = Vec::new();
    for locale in Locale::ALL {
        languages.push(
            button(text(locale.native_name()).size(12))
                .on_press(Message::SelectLocale(locale))
                .padding(8)
                .style(multideck_ui::toggle_button(locale == app.locale))
                .into(),
        );
    }

    let content = column![
        text(tr!("settings_title")).size(20),
        container(
            column![
                text(tr!("settings_language")).size(14),
                text(tr!("settings_language_desc"))
                    .size(10)
                    .color(Colors::MUTED),
                Row::with_children(languages).spacing(8),
            ]
            .spacing(8),
        )
        .padding(14)
        .style(multideck_ui::card()),
        container(
            column![
                text(tr!("settings_brightness")).size(14),
                slider(0..=100u8, app.profile.brightness, Message::SetBrightness)
                    .step(1u8)
                    .width(Length::Fill),
                text(format!("{}%", app.profile.brightness))
                    .size(11)
                    .color(Colors::MUTED),
            ]
            .spacing(8),
        )
        .padding(14)
        .style(multideck_ui::card()),
        container(
            column![
                text(tr!("settings_storage")).size(14),
                text(app.data_dir.display().to_string())
                    .size(11)
                    .color(Colors::MUTED),
                button(text(tr!("settings_open_folder")).size(12))
                    .on_press(Message::OpenDataFolder)
                    .padding(8)
                    .style(multideck_ui::ghost_button()),
            ]
            .spacing(8),
        )
        .padding(14)
        .style(multideck_ui::card()),
        container(
            column![
                text(tr!("settings_about")).size(14),
                text(format!("MultiDeck v{}", env!("CARGO_PKG_VERSION"))).size(11),
                text("MIT").size(11).color(Colors::MUTED),
                text(tr!("settings_devices_hint"))
                    .size(11)
                    .color(Colors::MUTED),
            ]
            .spacing(6),
        )
        .padding(14)
        .style(multideck_ui::card()),
    ]
    .spacing(14);

    container(scrollable(content).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(18)
        .into()
}
