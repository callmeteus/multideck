//! Screens, panels and widgets of the desktop app.

use iced::widget::{
    Column, Row, button, column, container, image, mouse_area, responsive, row, scrollable, slider,
    svg, text, text_input,
};
use iced::{Element, Length};
use multideck_actions as actions;
use multideck_i18n::{Locale, tr, tr_args};
use multideck_ui::Colors;

use crate::app::{App, FREE_SLOTS, Message, Screen};

fn logo() -> svg::Handle {
    static HANDLE: std::sync::OnceLock<svg::Handle> = std::sync::OnceLock::new();
    HANDLE
        .get_or_init(|| {
            svg::Handle::from_memory(
                include_bytes!("../../../../assets/brand/logo-mark.svg").to_vec(),
            )
        })
        .clone()
}

/// Root view.
pub fn view(app: &App) -> Element<'_, Message> {
    let content = match app.screen {
        Screen::Editor => editor(app),
        Screen::Settings => settings(app),
    };
    let base: Element<'_, Message> = container(row![sidebar(app), content].spacing(0))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(multideck_ui::page())
        .into();

    if app.show_pairing {
        iced::widget::stack![base, pairing_modal(app)].into()
    } else if app.limit_notice {
        iced::widget::stack![base, limit_modal()].into()
    } else if app.profile_rename.is_some() || app.profile_delete.is_some() {
        iced::widget::stack![base, profile_dialog(app)].into()
    } else if let Some(id) = &app.profile_menu {
        iced::widget::stack![base, profile_menu(app, id)].into()
    } else {
        base
    }
}

fn profile_menu<'a>(app: &'a App, id: &str) -> Element<'a, Message> {
    let index = app
        .profiles
        .iter()
        .position(|profile| profile.id == id)
        .unwrap_or(0);
    let (left, top) = app
        .menu_pos
        .map(|(x, y)| (x + 6.0, y + 6.0))
        .unwrap_or((40.0, 274.0 + index as f32 * 38.0));

    let menu = container(
        column![
            button(text(tr!("profile_rename")).size(12))
                .on_press(Message::StartProfileRename)
                .width(Length::Fill)
                .padding(8)
                .style(multideck_ui::ghost_button()),
            button(text(tr!("profile_delete")).size(12))
                .on_press(Message::StartProfileDelete)
                .width(Length::Fill)
                .padding(8)
                .style(multideck_ui::ghost_button()),
        ]
        .spacing(2),
    )
    .width(Length::Fixed(170.0))
    .padding(6)
    .style(multideck_ui::dropdown());

    let layer = container(menu)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(iced::Padding {
            top: top.min(650.0),
            left: left.min(1000.0),
            right: 0.0,
            bottom: 0.0,
        })
        .align_x(iced::alignment::Horizontal::Left)
        .align_y(iced::alignment::Vertical::Top);

    let click_away = mouse_area(container(text("")).width(Length::Fill).height(Length::Fill))
        .on_press(Message::CloseProfileMenu);

    iced::widget::stack![click_away, layer].into()
}

fn profile_dialog(app: &App) -> Element<'_, Message> {
    let name_of = |id: &str| {
        app.profiles
            .iter()
            .find(|profile| profile.id == id)
            .map(|profile| profile.name.clone())
            .unwrap_or_default()
    };

    let body: Element<'_, Message> = if let Some((_, draft)) = &app.profile_rename {
        column![
            text(tr!("profile_rename_title")).size(18),
            text_input("", draft)
                .on_input(Message::ProfileRenameDraft)
                .on_submit(Message::ConfirmProfileRename)
                .padding(8)
                .width(Length::Fill)
                .style(multideck_ui::input()),
            row![
                button(text(tr!("profile_save")).size(12))
                    .on_press(Message::ConfirmProfileRename)
                    .padding(8)
                    .style(multideck_ui::primary_button()),
                button(text(tr!("profile_cancel")).size(12))
                    .on_press(Message::CloseProfileDialog)
                    .padding(8)
                    .style(multideck_ui::ghost_button()),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .into()
    } else if let Some(id) = &app.profile_delete {
        column![
            text(tr!("profile_delete_title")).size(18),
            text(tr_args!("profile_delete_confirm", "name" => name_of(id)))
                .size(11)
                .width(Length::Fill)
                .color(Colors::MUTED),
            row![
                button(text(tr!("profile_delete")).size(12))
                    .on_press(Message::ConfirmProfileDelete)
                    .padding(8)
                    .style(multideck_ui::primary_button()),
                button(text(tr!("profile_cancel")).size(12))
                    .on_press(Message::CloseProfileDialog)
                    .padding(8)
                    .style(multideck_ui::ghost_button()),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .into()
    } else if let Some(id) = &app.profile_menu {
        column![
            text(name_of(id)).size(18),
            button(text(tr!("profile_rename")).size(12))
                .on_press(Message::StartProfileRename)
                .width(Length::Fill)
                .padding(8)
                .style(multideck_ui::ghost_button()),
            button(text(tr!("profile_delete")).size(12))
                .on_press(Message::StartProfileDelete)
                .width(Length::Fill)
                .padding(8)
                .style(multideck_ui::ghost_button()),
            button(text(tr!("profile_cancel")).size(12))
                .on_press(Message::CloseProfileDialog)
                .width(Length::Fill)
                .padding(8)
                .style(multideck_ui::ghost_button()),
        ]
        .spacing(8)
        .into()
    } else {
        column![].into()
    };

    container(
        container(body)
            .width(Length::Fixed(420.0))
            .padding(22)
            .style(multideck_ui::modal()),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(multideck_ui::scrim())
    .into()
}

fn limit_modal() -> Element<'static, Message> {
    let body = column![
        text(tr!("mobile_limit_title")).size(18),
        text(tr!("mobile_limit_body"))
            .size(11)
            .width(Length::Fill)
            .color(Colors::MUTED),
        row![
            button(text(tr!("mobile_limit_apply")).size(12))
                .on_press(Message::ApplyFreeLayout)
                .padding(8)
                .style(multideck_ui::primary_button()),
            button(text(tr!("mobile_limit_later")).size(12))
                .on_press(Message::CloseLimitNotice)
                .padding(8)
                .style(multideck_ui::ghost_button()),
        ]
        .spacing(8),
    ]
    .spacing(12);

    container(
        container(body)
            .width(Length::Fixed(480.0))
            .padding(22)
            .style(multideck_ui::modal()),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(multideck_ui::scrim())
    .into()
}

fn spacer() -> Element<'static, Message> {
    container(text("")).width(Length::Fill).into()
}

fn section(title: impl ToString) -> Element<'static, Message> {
    text(title.to_string().to_uppercase())
        .size(10)
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
    let connected = if app.device_descriptor.transport == multideck_core::Transport::Network {
        app.mobile_connected
    } else {
        true
    };

    let mut profile_buttons: Vec<Element<'_, Message>> = Vec::new();
    for profile in &app.profiles {
        let active = profile.id == app.profile.id;
        let hover_id = format!("profile:{}", profile.id);
        let hovered = app.hover.as_deref() == Some(hover_id.as_str());
        profile_buttons.push(
            mouse_area(
                container(text(profile.name.clone()).size(13))
                    .width(Length::Fill)
                    .padding(8)
                    .style(multideck_ui::nav_item(active, hovered)),
            )
            .on_press(Message::SelectProfile(profile.id.clone()))
            .on_right_press(Message::OpenProfileMenu(profile.id.clone()))
            .on_enter(Message::Hover(hover_id))
            .on_exit(Message::Unhover)
            .into(),
        );
    }

    let mut device_items: Vec<Element<'_, Message>> = vec![
        text(device).size(14).into(),
        text(model).size(10).color(Colors::MUTED).into(),
    ];
    if connected {
        device_items.push(
            text(tr!("devices_connected"))
                .size(10)
                .color(Colors::SUCCESS)
                .into(),
        );
        if app.mobile_pro == Some(false) {
            device_items.push(
                text(tr!("devices_free_plan"))
                    .size(10)
                    .color(Colors::MUTED)
                    .width(Length::Fill)
                    .into(),
            );
        }
    } else if app.mobile_enabled() {
        device_items.push(
            button(text(tr!("devices_pair")).size(11))
                .on_press(Message::OpenPairing)
                .width(Length::Fill)
                .padding(6)
                .style(multideck_ui::primary_button())
                .into(),
        );
    }
    device_items.push(
        slider(0..=100u8, app.profile.brightness, Message::SetBrightness)
            .step(1u8)
            .width(Length::Fill)
            .into(),
    );

    let device_card = container(Column::with_children(device_items).spacing(4))
        .padding(10)
        .width(Length::Fill)
        .style(multideck_ui::card());

    let body = column![
        row![
            svg(logo()).width(30).height(30),
            column![
                text(tr!("app_name")).size(20),
                text(tr!("app_tagline")).size(10).color(Colors::MUTED),
            ]
            .spacing(2),
        ]
        .spacing(10),
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
        .width(Length::Fixed(252.0))
        .height(Length::Fill)
        .padding(16)
        .style(multideck_ui::panel())
        .into()
}

fn editor(app: &App) -> Element<'_, Message> {
    let mut sections: Vec<Element<'_, Message>> = vec![header(app)];
    sections.push(
        row![grid(app), right_panel(app)]
            .spacing(16)
            .height(Length::Fill)
            .into(),
    );
    sections.push(status_bar(app));

    container(Column::with_children(sections).spacing(14))
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
        text(app.profile.name.clone()).size(22),
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
    // A free phone only accepts the first three keys of the first two rows.
    // The shared constant keeps the visible grid and the profile rewrite aligned.
    let keys: Vec<u8> = if app.free_layout {
        FREE_SLOTS.iter().map(|key| *key as u8).collect()
    } else {
        (0..app.caps.key_count()).collect()
    };
    let cols = if app.free_layout {
        3usize
    } else {
        app.caps.cols as usize
    };
    let rows = keys.len().div_ceil(cols);
    let key_count = keys.len();

    let stats = row![
        text(format!(
            "{}: {}",
            tr!("devices_transport"),
            app.device_descriptor.name
        ))
        .size(10)
        .color(Colors::MUTED),
        text(format!("{}: {}", tr!("devices_keys"), key_count))
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

    let keypad = responsive(move |size| {
        let gap = 10.0;
        let available_width = (size.width - gap * (cols.saturating_sub(1)) as f32).max(0.0);
        let available_height = (size.height - gap * (rows.saturating_sub(1)) as f32).max(0.0);
        let pixels = (available_width / cols as f32)
            .min(available_height / rows as f32)
            .clamp(56.0, 150.0);

        let mut lines: Vec<Element<'_, Message>> = Vec::new();
        for row in keys.chunks(cols) {
            let mut cells: Vec<Element<'_, Message>> = Vec::new();
            for key in row {
                cells.push(key_cell(app, *key, pixels));
            }
            lines.push(Row::with_children(cells).spacing(gap).into());
        }

        container(Column::with_children(lines).spacing(gap))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    })
    .width(Length::Fill)
    .height(Length::Fill);

    container(
        column![
            keypad,
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

fn key_cell(app: &App, key: u8, pixels: f32) -> Element<'_, Message> {
    let selected = app.selected_key == Some(key);
    let hover_id = format!("key:{key}");
    let hovered = app.hover.as_deref() == Some(hover_id.as_str());
    let inner: Element<'_, Message> = match app.key_images.get(key as usize) {
        Some(Some(handle)) => image(handle.clone())
            .width(Length::Fixed(pixels))
            .height(Length::Fixed(pixels))
            .into(),
        _ => container(text(""))
            .width(Length::Fixed(pixels))
            .height(Length::Fixed(pixels))
            .into(),
    };

    container(
        mouse_area(inner)
            .on_press(Message::SelectKey(key))
            .on_release(Message::DropOnKey(key))
            .on_enter(Message::Hover(hover_id))
            .on_exit(Message::Unhover),
    )
    .padding(3)
    .style(multideck_ui::key_frame(selected, hovered))
    .into()
}

fn right_panel(app: &App) -> Element<'_, Message> {
    if let Some(code) = &app.pairing_code {
        return panel_container(pairing_request(code));
    }
    if let Some(key) = app.pending_delete {
        return panel_container(confirmation(app, key));
    }

    let mut items: Vec<Element<'_, Message>> = Vec::new();
    items.push(inspector(app));
    items.push(section(tr!("actions_palette")));
    items.push(
        text_input(&tr!("palette_search"), &app.palette_query)
            .on_input(Message::PaletteQuery)
            .padding(8)
            .style(multideck_ui::input())
            .into(),
    );
    let query = app.palette_query.trim().to_lowercase();
    for definition in actions::definitions() {
        if !query.is_empty() {
            let label = tr(definition.label_key).to_lowercase();
            let description = tr(definition.description_key).to_lowercase();
            let tag_match = definition
                .tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(&query));
            if !label.contains(&query)
                && !description.contains(&query)
                && !tag_match
                && !definition.kind.contains(&query)
            {
                continue;
            }
        }
        let hovered = app.hover.as_deref() == Some(format!("palette:{}", definition.kind).as_str());
        items.push(palette_item(definition, hovered));
    }

    panel_container(Column::with_children(items).spacing(8))
}

fn mobile_pair_body<'a>(
    app: &'a App,
    handle: &'a iced::widget::image::Handle,
) -> Element<'a, Message> {
    let mut chips: Vec<Element<'_, Message>> = Vec::new();
    for interface in &app.mobile_interfaces {
        let selected = app.mobile_address.as_deref() == Some(interface.address.as_str());
        chips.push(
            button(text(format!("{} · {}", interface.name, interface.address)).size(10))
                .on_press(Message::SelectMobileAddress(interface.address.clone()))
                .width(Length::Fill)
                .padding(6)
                .style(multideck_ui::toggle_button(selected))
                .into(),
        );
    }

    row![
        container(image(handle.clone()).width(190).height(190))
            .padding(10)
            .style(multideck_ui::qr_tile()),
        column![
            text(tr!("mobile_pair_title")).size(18),
            text(tr!("mobile_pair_body"))
                .size(11)
                .width(Length::Fill)
                .color(Colors::MUTED),
            section(tr!("mobile_pair_network")),
            Column::with_children(chips).spacing(4),
            tailscale_hint(app),
        ]
        .spacing(6)
        .width(Length::Fill),
    ]
    .spacing(18)
    .into()
}

fn tailscale_hint(app: &App) -> Element<'_, Message> {
    let selected = app.mobile_address.as_deref().and_then(|address| {
        app.mobile_interfaces
            .iter()
            .find(|interface| interface.address == address)
    });
    let is_tailscale = selected
        .map(|interface| {
            interface.name.to_ascii_lowercase().contains("tailscale")
                || interface.address.starts_with("100.")
        })
        .unwrap_or(false);
    if is_tailscale {
        text(tr!("mobile_pair_tailscale_hint"))
            .size(10)
            .width(Length::Fill)
            .color(Colors::MUTED)
            .into()
    } else {
        container(text("")).height(Length::Fixed(0.0)).into()
    }
}

fn pairing_modal(app: &App) -> Element<'_, Message> {
    let body: Element<'_, Message> = if let Some(code) = &app.pairing_code {
        pairing_request(code)
    } else if let Some((_, handle)) = &app.qr {
        mobile_pair_body(app, handle)
    } else {
        column![
            text(tr!("mobile_pair_title")).size(18),
            text(tr!("mobile_pair_unavailable"))
                .size(11)
                .color(Colors::MUTED),
        ]
        .spacing(6)
        .into()
    };

    let close = button(text(tr!("pairing_close")).size(11))
        .on_press(Message::ClosePairing)
        .padding(6)
        .style(multideck_ui::ghost_button());

    container(
        container(column![body, row![spacer(), close]].spacing(14))
            .width(Length::Fixed(560.0))
            .padding(22)
            .style(multideck_ui::modal()),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(multideck_ui::scrim())
    .into()
}

fn panel_container<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    let body = container(content).padding(iced::Padding {
        top: 0.0,
        right: 10.0,
        bottom: 0.0,
        left: 0.0,
    });
    container(
        scrollable(body)
            .direction(iced::widget::scrollable::Direction::Vertical(
                iced::widget::scrollable::Scrollbar::new()
                    .width(6.0)
                    .margin(4.0)
                    .scroller_width(4.0),
            ))
            .style(multideck_ui::scrollable_style())
            .height(Length::Fill),
    )
    .width(Length::Fixed(320.0))
    .height(Length::Fill)
    .padding(12)
    .style(multideck_ui::card())
    .into()
}

fn pairing_request(code: &str) -> Element<'_, Message> {
    column![
        section(tr!("pairing_title")),
        text(code.to_string()).size(26),
        text(tr_args!("pairing_body", "code" => code))
            .size(11)
            .color(Colors::MUTED),
        row![
            button(text(tr!("pairing_accept")).size(12))
                .on_press(Message::PairAccept)
                .padding(8)
                .style(multideck_ui::primary_button()),
            button(text(tr!("pairing_reject")).size(12))
                .on_press(Message::PairReject)
                .padding(8)
                .style(multideck_ui::ghost_button()),
        ]
        .spacing(6),
    ]
    .spacing(10)
    .into()
}

fn confirmation(app: &App, key: u8) -> Element<'_, Message> {
    let action = app
        .profile
        .slot(key)
        .and_then(|slot| slot.action.as_ref())
        .map(|action| actions::present(action, key).title)
        .unwrap_or_default();

    column![
        section(tr!("confirm_delete_title")),
        text(action).size(15),
        text(tr!("confirm_delete_body"))
            .size(11)
            .color(Colors::MUTED),
        row![
            button(text(tr!("inspector_delete")).size(12))
                .on_press(Message::ConfirmDelete)
                .padding(8)
                .style(multideck_ui::primary_button()),
            button(text(tr!("common_cancel")).size(12))
                .on_press(Message::CancelDelete)
                .padding(8)
                .style(multideck_ui::ghost_button()),
        ]
        .spacing(6),
    ]
    .spacing(10)
    .into()
}

fn palette_item(
    definition: &'static multideck_core::ActionDef,
    hovered: bool,
) -> Element<'static, Message> {
    let label = tr(definition.label_key);
    let description = tr(definition.description_key);
    mouse_area(
        container(
            row![
                action_icon(definition.icon),
                column![
                    text(label).size(13),
                    text(description).size(10).color(Colors::MUTED),
                ]
                .spacing(2),
            ]
            .spacing(10),
        )
        .width(Length::Fill)
        .padding(10)
        .style(multideck_ui::palette_card(hovered)),
    )
    .on_press(Message::StartDrag(definition.kind))
    .on_release(Message::CancelDrag)
    .on_enter(Message::Hover(format!("palette:{}", definition.kind)))
    .on_exit(Message::Unhover)
    .into()
}

fn action_icon(name: &str) -> Element<'static, Message> {
    svg(icon_handle(name)).width(20).height(20).into()
}

fn icon_handle(name: &str) -> svg::Handle {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    static CACHE: OnceLock<Mutex<HashMap<String, svg::Handle>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(cache) = cache.lock()
        && let Some(handle) = cache.get(name)
    {
        return handle.clone();
    }

    let path = multideck_render::icon_path(name).unwrap_or_default();
    let markup = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\" fill=\"#cfd6e6\"><path d=\"{path}\"/></svg>"
    );
    let handle = svg::Handle::from_memory(markup.into_bytes());
    if let Ok(mut cache) = cache.lock() {
        cache.insert(name.to_string(), handle.clone());
    }
    handle
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
                    .size(13)
                    .style(multideck_ui::input()),
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

    let mut devices: Vec<Element<'_, Message>> = Vec::new();
    for (kind, label) in [
        ("mobile", tr!("devices_mobile")),
        ("virtual", tr!("devices_virtual")),
        ("usb", tr!("devices_usb")),
    ] {
        devices.push(
            button(text(label).size(12))
                .on_press(Message::SelectDevice(kind.to_string()))
                .padding(8)
                .style(multideck_ui::toggle_button(app.device_kind == kind))
                .into(),
        );
    }

    let content = column![
        text(tr!("settings_title")).size(20),
        container(
            column![
                text(tr!("settings_device")).size(14),
                text(tr!("settings_device_desc"))
                    .size(10)
                    .color(Colors::MUTED),
                Row::with_children(devices).spacing(8),
            ]
            .spacing(8),
        )
        .padding(14)
        .style(multideck_ui::card()),
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
                text(tr!("settings_mobile")).size(14),
                text(tr!("settings_mobile_desc"))
                    .size(10)
                    .color(Colors::MUTED),
                button(text(tr!("settings_mobile_pair")).size(12))
                    .on_press_maybe(app.mobile_enabled().then_some(Message::OpenPairing))
                    .padding(8)
                    .style(multideck_ui::primary_button()),
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

    let body = container(content).padding(iced::Padding {
        top: 0.0,
        right: 10.0,
        bottom: 0.0,
        left: 0.0,
    });
    container(
        scrollable(body)
            .direction(iced::widget::scrollable::Direction::Vertical(
                iced::widget::scrollable::Scrollbar::new()
                    .width(6.0)
                    .margin(4.0)
                    .scroller_width(4.0),
            ))
            .style(multideck_ui::scrollable_style())
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(18)
    .into()
}
