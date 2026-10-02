//! Add / edit dialog for a channel.

use std::rc::Rc;

use adw::prelude::*;

use crate::model::{describe_url, normalize_url, parse_tags};

use super::Ui;

/// Opens the editor. `channel_id` selects an existing channel, otherwise a new
/// one is created.
pub fn open(ui: &Rc<Ui>, channel_id: Option<i64>) {
    let existing = channel_id.and_then(|id| ui.app.borrow().channel(id).cloned());
    let known_tags = ui.app.borrow().tags.clone();

    let name_entry = gtk::Entry::new();
    name_entry.set_placeholder_text(Some("Nombre del canal"));
    name_entry.set_activates_default(true);
    if let Some(channel) = &existing {
        name_entry.set_text(&channel.name);
    }

    let url_entry = gtk::Entry::new();
    url_entry.set_placeholder_text(Some("@handle, youtube.com/@canal o https://…"));
    url_entry.set_activates_default(true);
    if let Some(channel) = &existing {
        url_entry.set_text(&channel.url);
    }

    let tags_entry = gtk::Entry::new();
    tags_entry.set_placeholder_text(Some("rust, música, español"));
    tags_entry.set_activates_default(true);
    if let Some(channel) = &existing {
        tags_entry.set_text(&channel.tags.join(", "));
    }

    let preview = gtk::Label::new(None);
    preview.add_css_class("ytd-hint");
    preview.set_xalign(0.0);
    preview.set_wrap(true);

    let error = gtk::Label::new(None);
    error.add_css_class("ytd-error");
    error.set_xalign(0.0);
    error.set_wrap(true);
    error.set_visible(false);

    // Live feedback while typing: show the link that will actually be opened.
    {
        let preview_weak = preview.downgrade();
        let error_weak = error.downgrade();
        url_entry.connect_changed(move |entry| {
            let raw = entry.text().to_string();
            let normalized = normalize_url(&raw);

            if let Some(preview) = preview_weak.upgrade() {
                if normalized.is_empty() {
                    preview.set_text("Se abrirá en el navegador al hacer clic.");
                } else {
                    preview.set_text(&format!("Se abrirá: {normalized}"));
                }
            }
            if let Some(error) = error_weak.upgrade() {
                error.set_visible(false);
            }
        });
    }

    let confirm = gtk::Button::new();
    let form = gtk::Box::new(gtk::Orientation::Vertical, 0);
    form.add_css_class("ytd-form");
    form.append(&field("Nombre", &name_entry));
    form.append(&field("Canal", &url_entry));
    form.append(&preview);
    form.append(&error);
    form.append(&field("Etiquetas (separadas por comas)", &tags_entry));

    if !known_tags.is_empty() {
        form.append(&existing_tag_picker(&tags_entry, &known_tags));
    }

    let scroller = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_height(true)
        .child(&form)
        .build();

    let dialog = adw::Dialog::new();
    dialog.set_title(if existing.is_some() {
        "Editar canal"
    } else {
        "Añadir canal"
    });
    dialog.set_content_width(460);

    let toolbar = adw::ToolbarView::new();
    let fields = Fields {
        confirm: confirm.clone(),
        name: name_entry.clone(),
        url: url_entry.clone(),
        tags: tags_entry.clone(),
        error: error.clone(),
    };
    toolbar.add_top_bar(&header(&fields, dialog.downgrade(), ui, channel_id));
    toolbar.set_content(Some(&scroller));
    dialog.set_child(Some(&toolbar));
    dialog.set_default_widget(Some(&fields.confirm));

    refresh_preview(&url_entry, &preview);
    dialog.present(Some(&ui.window));

    if existing.is_none() {
        name_entry.grab_focus();
    } else {
        url_entry.grab_focus();
    }
}

/// The live editor widgets, owned so the button handler can outlive the builder.
#[derive(Clone)]
struct Fields {
    confirm: gtk::Button,
    name: gtk::Entry,
    url: gtk::Entry,
    tags: gtk::Entry,
    error: gtk::Label,
}

fn header(
    fields: &Fields,
    dialog: gtk::glib::WeakRef<adw::Dialog>,
    ui: &Rc<Ui>,
    channel_id: Option<i64>,
) -> adw::HeaderBar {
    let Fields {
        confirm,
        name: name_entry,
        url: url_entry,
        tags: tags_entry,
        error,
    } = fields.clone();
    let header = adw::HeaderBar::new();

    let cancel = gtk::Button::with_label("Cancelar");
    cancel.add_css_class("flat");
    let weak = dialog.clone();
    cancel.connect_clicked(move |_| {
        if let Some(dialog) = weak.upgrade() {
            dialog.force_close();
        }
    });
    header.pack_start(&cancel);

    let confirm = confirm.clone();
    confirm.set_label(if channel_id.is_some() {
        "Guardar"
    } else {
        "Añadir"
    });
    confirm.add_css_class("suggested-action");

    let ui_weak = Rc::downgrade(ui);
    let weak = dialog.clone();
    confirm.connect_clicked(move |_| {
        let (Some(ui), Some(dialog)) = (ui_weak.upgrade(), weak.upgrade()) else {
            return;
        };
        if save(
            &ui,
            &dialog,
            channel_id,
            &name_entry,
            &url_entry,
            &tags_entry,
            &error,
        ) {
            dialog.force_close();
        }
    });
    header.pack_end(&confirm);

    header
}

fn field(title: &str, widget: &impl IsA<gtk::Widget>) -> gtk::Box {
    let wrapper = gtk::Box::new(gtk::Orientation::Vertical, 0);
    wrapper.add_css_class("ytd-field");

    let label = gtk::Label::new(Some(title));
    label.add_css_class("ytd-field-title");
    label.set_xalign(0.0);
    wrapper.append(&label);
    wrapper.append(widget);
    wrapper
}

fn existing_tag_picker(tags_entry: &gtk::Entry, known_tags: &[String]) -> gtk::Box {
    let wrapper = gtk::Box::new(gtk::Orientation::Vertical, 0);
    wrapper.add_css_class("ytd-existing");

    let hint = gtk::Label::new(Some("Etiquetas que ya usas (clic para añadir)"));
    hint.add_css_class("ytd-existing-label");
    hint.set_xalign(0.0);
    wrapper.append(&hint);

    let chips = gtk::FlowBox::new();
    chips.set_selection_mode(gtk::SelectionMode::None);
    chips.set_max_children_per_line(5);
    chips.set_column_spacing(6);
    chips.set_row_spacing(6);

    for tag in known_tags {
        let button = gtk::Button::with_label(tag);
        button.add_css_class("ytd-chip");
        button.add_css_class("flat");

        let entry_weak = tags_entry.downgrade();
        let tag = tag.clone();
        button.connect_clicked(move |_| {
            if let Some(entry) = entry_weak.upgrade() {
                append_tag(&entry, &tag);
            }
        });

        let cell = gtk::FlowBoxChild::new();
        cell.set_child(Some(&button));
        chips.append(&cell);
    }

    wrapper.append(&chips);
    wrapper
}

fn append_tag(entry: &gtk::Entry, tag: &str) {
    let current = entry.text().to_string();
    let already = parse_tags(&current)
        .iter()
        .any(|t| t.eq_ignore_ascii_case(tag));
    if already {
        return;
    }

    let next = if current.trim().is_empty() {
        tag.to_string()
    } else {
        format!("{}, {tag}", current.trim_end().trim_end_matches(','))
    };
    entry.set_text(&next);
    entry.grab_focus();
}

fn refresh_preview(entry: &gtk::Entry, preview: &gtk::Label) {
    let normalized = normalize_url(&entry.text());
    preview.set_text(&if normalized.is_empty() {
        "Se abrirá en el navegador al hacer clic.".to_string()
    } else {
        format!("Se abrirá: {normalized}")
    });
}

fn save(
    ui: &Rc<Ui>,
    _dialog: &adw::Dialog,
    channel_id: Option<i64>,
    name_entry: &gtk::Entry,
    url_entry: &gtk::Entry,
    tags_entry: &gtk::Entry,
    error: &gtk::Label,
) -> bool {
    let normalized = normalize_url(&url_entry.text());
    if normalized.is_empty() {
        error.set_text("Indica el canal: un @handle o un enlace de YouTube.");
        error.set_visible(true);
        url_entry.add_css_class("error");
        url_entry.grab_focus();
        return false;
    }
    url_entry.remove_css_class("error");

    let raw_name = name_entry.text().trim().to_string();
    let name = if raw_name.is_empty() {
        describe_url(&normalized)
    } else {
        raw_name
    };
    let tags = parse_tags(&tags_entry.text());

    let result = {
        let mut app = ui.app.borrow_mut();
        match channel_id {
            Some(id) => app
                .update_channel(id, &name, &normalized, &tags)
                .map(|_| ()),
            None => app.add_channel(&name, &normalized, &tags).map(|_| ()),
        }
    };

    match result {
        Ok(()) => {
            ui.refresh();
            let verb = if channel_id.is_some() {
                "guardado"
            } else {
                "añadido"
            };
            ui.notify(&format!("«{name}» {verb}"));
            true
        }
        Err(failure) => {
            error.set_text(&format!("No se pudo guardar: {failure}"));
            error.set_visible(true);
            false
        }
    }
}
