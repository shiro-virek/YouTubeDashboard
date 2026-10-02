//! The preferences dialog: which browser opens channel links.

use std::rc::Rc;

use adw::prelude::*;

use crate::config::{self, Browser};

use super::Ui;

const CUSTOM: &str = "Otro comando…";

pub fn open(ui: &Rc<Ui>) {
    // Cloned so the signal handlers can own it: they outlive this function.
    let ui = ui.clone();
    let detected = config::detect_browsers();

    let window = adw::PreferencesDialog::new();
    window.set_title("Preferencias");
    window.set_search_enabled(false);

    let page = adw::PreferencesPage::new();
    page.set_title("General");
    page.set_icon_name(Some("preferences-system-symbolic"));

    let group = adw::PreferencesGroup::new();
    group.set_title("Navegador");
    group.set_description(Some(
        "Aplicación con la que se abren los enlaces de los canales.",
    ));

    let mut labels: Vec<&str> = vec!["Predeterminado del sistema"];
    labels.extend(detected.iter().map(|(label, _)| *label));
    labels.push(CUSTOM);
    let custom_index = (labels.len() - 1) as u32;

    let current = ui.app.borrow().config.browser.clone();
    let selected = match &current {
        Browser::System => 0,
        Browser::Command(command) => detected
            .iter()
            .position(|(_, program)| *program == command)
            .map(|index| index as u32 + 1)
            .unwrap_or(custom_index),
    };

    // ---- browser dropdown ----
    let dropdown = gtk::DropDown::from_strings(&labels);
    dropdown.set_valign(gtk::Align::Center);
    dropdown.set_selected(selected);
    dropdown.set_tooltip_text(Some("Aplicación con la que se abren los enlaces"));

    let row = adw::ActionRow::new();
    row.set_title("Abrir con");
    row.set_subtitle(&current.label());
    row.add_suffix(&dropdown);
    row.set_activatable_widget(Some(&dropdown));
    group.add(&row);

    // ---- free-form command, only shown for the "Otro comando…" option ----
    let entry = gtk::Entry::new();
    entry.set_hexpand(true);
    entry.set_valign(gtk::Align::Center);
    entry.set_width_chars(26);
    entry.set_placeholder_text(Some("firefox"));
    entry.set_tooltip_text(Some(
        "Comando al que se añade la URL, por ejemplo: firefox -P",
    ));
    if let Browser::Command(command) = &current {
        entry.set_text(command);
    }

    let command_row = adw::ActionRow::new();
    command_row.set_title("Comando");
    command_row.add_suffix(&entry);
    command_row.set_activatable_widget(Some(&entry));
    command_row.set_visible(selected == custom_index);
    group.add(&command_row);

    // ---- wiring ----
    let row_weak = row.downgrade();
    let command_weak = command_row.downgrade();
    let ui_for_dropdown = ui.clone();
    dropdown.connect_selected_notify(move |dropdown| {
        let index = dropdown.selected();
        if let Some(command_row) = command_weak.upgrade() {
            command_row.set_visible(index == custom_index);
        }

        let browser = if index == 0 {
            Browser::System
        } else if index != custom_index {
            Browser::Command(detected[(index - 1) as usize].1.to_string())
        } else {
            // "Otro comando…": keep whatever is in the box, so merely opening
            // the dialog can never wipe an existing preference.
            return;
        };
        apply(&ui_for_dropdown, browser, &row_weak);
    });

    let row_weak = row.downgrade();
    let ui_for_entry = ui.clone();
    entry.connect_changed(move |entry| {
        let text = entry.text().trim().to_string();
        if !text.is_empty() {
            apply(&ui_for_entry, Browser::Command(text), &row_weak);
        }
    });

    page.add(&group);
    window.add(&page);
    window.present(Some(ui.window()));
}

/// Stores the preference and mirrors the resulting value in the dialog row.
fn apply(ui: &Rc<Ui>, browser: Browser, row_weak: &gtk::glib::WeakRef<adw::ActionRow>) {
    let label = browser.label();
    let saved = ui.app.borrow_mut().set_browser(browser).is_ok();

    if let Some(row) = row_weak.upgrade() {
        row.set_subtitle(&label);
    }
    if !saved {
        ui.notify("No se pudo guardar la preferencia; se usará solo en esta sesión");
    }
}
