//! Channel cards (grid and list flavours) plus the drag & drop reordering.

use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, glib};

use crate::model::{describe_url, Channel};
use crate::state::ViewMode;

use super::avatar;
use super::Ui;

enum Action {
    Open,
    Edit,
    Delete,
}

/// Builds the inner card widget (without the FlowBoxChild / ListBoxRow wrapper).
pub fn build(ui: &Rc<Ui>, channel: &Channel, view: ViewMode) -> gtk::Widget {
    let compact = view == ViewMode::List;

    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    card.add_css_class("ytd-card");
    if !compact {
        card.set_width_request(248);
    }

    let main = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    main.add_css_class("ytd-main");
    main.set_hexpand(true);
    card.append(&main);

    main.append(&avatar::build(
        &channel.label(),
        &channel.initial(),
        if compact { 34 } else { 46 },
    ));

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.set_valign(gtk::Align::Center);
    main.append(&text);

    let name = gtk::Label::new(Some(&channel.label()));
    name.add_css_class("ytd-name");
    name.set_xalign(0.0);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name.set_max_width_chars(24);
    name.set_tooltip_text(Some(&channel.label()));
    text.append(&name);

    let handle = gtk::Label::new(Some(&describe_url(&channel.url)));
    handle.add_css_class("ytd-handle");
    handle.set_xalign(0.0);
    handle.set_ellipsize(gtk::pango::EllipsizeMode::End);
    handle.set_tooltip_text(Some(&channel.url));
    text.append(&handle);

    if !channel.tags.is_empty() {
        let chips = gtk::FlowBox::new();
        chips.add_css_class("ytd-tags");
        chips.set_selection_mode(gtk::SelectionMode::None);
        chips.set_homogeneous(true);
        chips.set_max_children_per_line(if compact { 6 } else { 3 });
        chips.set_column_spacing(5);
        chips.set_row_spacing(4);

        for tag in &channel.tags {
            let chip = gtk::Label::new(Some(tag));
            chip.add_css_class("ytd-tag");
            let cell = gtk::FlowBoxChild::new();
            cell.set_child(Some(&chip));
            chips.append(&cell);
        }
        text.append(&chips);
    }

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    actions.add_css_class("ytd-actions");
    actions.set_halign(gtk::Align::End);
    actions.append(&action_button(
        ui,
        channel.id,
        Action::Open,
        "document-open-symbolic",
        "Abrir en el navegador",
    ));
    actions.append(&action_button(
        ui,
        channel.id,
        Action::Edit,
        "document-edit-symbolic",
        "Editar canal",
    ));
    actions.append(&action_button(
        ui,
        channel.id,
        Action::Delete,
        "user-trash-symbolic",
        "Eliminar canal",
    ));
    card.append(&actions);

    // A double click on the card body opens the channel; a single click only
    // focuses. `released` (not `pressed`) so a drag gesture never fires the
    // browser, and `n_press == 2` so pressing and dragging away does not count
    // as the second click of a double click.
    let click = gtk::GestureClick::builder()
        .button(gdk::BUTTON_PRIMARY)
        .build();
    let ui_weak = Rc::downgrade(ui);
    let id = channel.id;
    click.connect_released(move |_, presses, _x, _y| {
        if presses == 2 {
            if let Some(ui) = ui_weak.upgrade() {
                ui.open_channel(id);
            }
        }
    });
    main.add_controller(click);

    install_context_menu(ui, &card, channel.id);
    install_dnd(ui, &card, &main, channel, view);

    card.upcast()
}

/// Wraps a card for the grid container.
pub fn grid_child(card: gtk::Widget) -> gtk::FlowBoxChild {
    let child = gtk::FlowBoxChild::new();
    child.set_child(Some(&card));
    child
}

/// Wraps a card for the list container.
pub fn list_row(card: gtk::Widget) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&card));
    row.set_activatable(false);
    row
}

fn action_button(ui: &Rc<Ui>, id: i64, action: Action, icon: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon);
    button.set_tooltip_text(Some(tooltip));
    if matches!(action, Action::Delete) {
        button.add_css_class("error");
    }

    let ui_weak = Rc::downgrade(ui);
    button.connect_clicked(move |_| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        match action {
            Action::Open => ui.open_channel(id),
            Action::Edit => super::editor::open(&ui, Some(id)),
            Action::Delete => confirm_delete(&ui, id),
        }
    });

    button
}

fn install_context_menu(ui: &Rc<Ui>, card: &gtk::Box, id: i64) {
    let popover = gtk::Popover::new();
    popover.set_has_arrow(false);
    popover.set_parent(card);

    let menu = gtk::Box::new(gtk::Orientation::Vertical, 4);
    menu.set_margin_top(6);
    menu.set_margin_bottom(6);
    menu.set_margin_start(6);
    menu.set_margin_end(6);
    popover.set_child(Some(&menu));

    for (label, icon, kind) in [
        (
            "Abrir en el navegador",
            "document-open-symbolic",
            MenuItem::Open,
        ),
        ("Copiar enlace", "edit-copy-symbolic", MenuItem::Copy),
        ("Editar", "document-edit-symbolic", MenuItem::Edit),
        ("Eliminar", "user-trash-symbolic", MenuItem::Delete),
    ] {
        let button = gtk::Button::new();
        button.set_child(Some(&menu_row(icon, label)));
        button.add_css_class("flat");
        button.set_hexpand(true);
        if matches!(kind, MenuItem::Delete) {
            button.add_css_class("error");
        }

        let ui_weak = Rc::downgrade(ui);
        let popover_weak = popover.downgrade();
        button.connect_clicked(move |_| {
            if let Some(popover) = popover_weak.upgrade() {
                popover.popdown();
            }
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            match kind {
                MenuItem::Open => ui.open_channel(id),
                MenuItem::Copy => ui.copy_channel_url(id),
                MenuItem::Edit => super::editor::open(&ui, Some(id)),
                MenuItem::Delete => confirm_delete(&ui, id),
            }
        });

        menu.append(&button);
    }

    let gesture = gtk::GestureClick::builder()
        .button(gdk::BUTTON_SECONDARY)
        .build();
    let popover_weak = popover.downgrade();
    gesture.connect_released(move |_, _, x, y| {
        if let Some(popover) = popover_weak.upgrade() {
            popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            popover.popup();
        }
    });
    card.add_controller(gesture);
}

enum MenuItem {
    Open,
    Copy,
    Edit,
    Delete,
}

fn menu_row(icon: &str, label: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);

    let image = gtk::Image::from_icon_name(icon);
    row.append(&image);

    let text = gtk::Label::new(Some(label));
    text.set_xalign(0.0);
    text.set_hexpand(true);
    row.append(&text);

    row
}

/// Drag the card body, drop it onto another card to reposition it.
fn install_dnd(
    ui: &Rc<Ui>,
    card: &gtk::Box,
    drag_area: &gtk::Box,
    channel: &Channel,
    view: ViewMode,
) {
    let source = gtk::DragSource::new();
    source.set_actions(gdk::DragAction::MOVE);

    let payload = channel.id.to_string();
    source.set_content(Some(&gdk::ContentProvider::for_value(&payload.to_value())));
    // Reuse the avatar widget itself as the drag icon.
    let icon = avatar::build(&channel.label(), &channel.initial(), 48);
    let paintable = gtk::WidgetPaintable::new(Some(&icon));
    source.set_icon(Some(&paintable), 24, 24);
    drag_area.add_controller(source);

    let target = gtk::DropTarget::new(glib::Type::STRING, gdk::DragAction::MOVE);

    let weak = card.downgrade();
    target.connect_enter(move |_, _, _| {
        if let Some(card) = weak.upgrade() {
            card.add_css_class("ytd-drop");
        }
        gdk::DragAction::MOVE
    });

    let weak = card.downgrade();
    target.connect_leave(move |_| {
        if let Some(card) = weak.upgrade() {
            card.remove_css_class("ytd-drop");
        }
    });

    target.connect_motion(|_, _, _| gdk::DragAction::MOVE);

    let target_id = channel.id;
    let card_weak = card.downgrade();
    let ui_weak = Rc::downgrade(ui);
    target.connect_drop(move |_, value, x, y| {
        let (Some(ui), Some(card)) = (ui_weak.upgrade(), card_weak.upgrade()) else {
            return false;
        };
        let Ok(text) = value.get::<String>() else {
            return false;
        };
        let Ok(source_id) = text.parse::<i64>() else {
            return false;
        };
        let after = drop_after(&card, x, y, view);
        ui.reorder(source_id, target_id, after);
        true
    });

    card.add_controller(target);
}

/// Grid drops look at the horizontal half, list drops at the vertical half.
fn drop_after(card: &gtk::Box, x: f64, y: f64, view: ViewMode) -> bool {
    match view {
        ViewMode::Grid => x > card.width() as f64 / 2.0,
        ViewMode::List => y > card.height() as f64 / 2.0,
    }
}

pub fn confirm_delete(ui: &Rc<Ui>, id: i64) {
    let Some(label) = ui.app.borrow().channel(id).map(Channel::label) else {
        return;
    };

    let dialog = adw::AlertDialog::new(
        Some("¿Eliminar el canal?"),
        Some(&format!("«{label}» se quitará de tu panel.")),
    );
    dialog.add_response("cancel", "Cancelar");
    dialog.add_response("delete", "Eliminar");
    dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    let ui_weak = Rc::downgrade(ui);
    dialog.connect_response(None, move |_, response| {
        if response == "delete" {
            if let Some(ui) = ui_weak.upgrade() {
                ui.delete_channel(id);
            }
        }
    });

    dialog.present(Some(&ui.window));
}
