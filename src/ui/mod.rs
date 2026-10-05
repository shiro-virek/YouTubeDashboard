pub mod avatar;
pub mod card;
pub mod editor;
mod prefs;
pub mod styles;

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use gtk::glib;

use crate::config;
use crate::model::Channel;
use crate::state::{App, ViewMode};

const APP_ID: &str = "dev.ytdash.YoutubeDashboard";

/// What the empty/status page is currently telling the user.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    None,
    NoChannels,
    NoMatches,
}

/// Tracks what is already on screen so re-rendering can skip needless work.
#[derive(Default)]
struct Rendered {
    revision: Option<u64>,
    view: Option<ViewMode>,
    visible: Vec<i64>,
    tags: Vec<String>,
    selected: Vec<String>,
    status: Option<Status>,
}

pub struct Ui {
    pub app: Rc<RefCell<App>>,
    application: adw::Application,
    window: adw::ApplicationWindow,
    toast: adw::ToastOverlay,
    db_path: PathBuf,

    title: adw::WindowTitle,
    view_toggle: adw::ToggleGroup,
    search: gtk::SearchEntry,
    chip_bar: gtk::FlowBox,
    grid: gtk::FlowBox,
    list: gtk::ListBox,
    view_stack: gtk::Stack,
    page_stack: gtk::Stack,
    empty_page: adw::StatusPage,
    empty_cta: gtk::Button,

    /// Guards against signal handlers reacting to the changes `render` itself
    /// makes, which would re-enter while the app state is already borrowed.
    rendering: Cell<bool>,
    rendered: RefCell<Rendered>,
}

pub fn build(app: Rc<RefCell<App>>, application: &adw::Application) -> Rc<Ui> {
    styles::install();
    let db_path = app.borrow().db_path.clone();

    let window = adw::ApplicationWindow::new(application);
    window.set_title(Some("Canales de YouTube"));
    window.set_default_size(1040, 700);
    window.set_size_request(420, 360);

    let toast = adw::ToastOverlay::new();

    // ---- header bar ----
    let header = adw::HeaderBar::new();
    let title = adw::WindowTitle::new("Canales de YouTube", "");
    header.set_title_widget(Some(&title));

    let view_toggle = adw::ToggleGroup::new();
    for (name, icon, tooltip) in [
        ("grid", "view-grid-symbolic", "Ver en cuadrícula"),
        ("list", "view-list-symbolic", "Ver en lista"),
    ] {
        let toggle = adw::Toggle::new();
        toggle.set_icon_name(Some(icon));
        toggle.set_tooltip(tooltip);
        toggle.set_name(Some(name));
        view_toggle.add(toggle);
    }
    view_toggle.set_active_name(Some(ViewMode::Grid.as_str()));
    header.pack_end(&view_toggle);

    let menu_button = gtk::MenuButton::new();
    menu_button.set_icon_name("open-menu-symbolic");
    menu_button.set_tooltip_text(Some("Menú principal"));
    menu_button.set_menu_model(Some(&primary_menu()));
    header.pack_end(&menu_button);

    let add_button = gtk::Button::from_icon_name("list-add-symbolic");
    add_button.set_tooltip_text(Some("Añadir canal (Ctrl+N)"));
    add_button.set_action_name(Some("win.add"));
    add_button.add_css_class("suggested-action");
    header.pack_end(&add_button);

    // ---- filters ----
    let filter_bar = gtk::Box::new(gtk::Orientation::Vertical, 0);
    filter_bar.add_css_class("ytd-filter");

    let search = gtk::SearchEntry::new();
    search.add_css_class("ytd-search");
    search.set_hexpand(true);
    search.set_placeholder_text(Some("Filtrar por nombre…"));
    search.set_tooltip_text(Some("Filtra los canales por nombre, handle o etiqueta"));
    filter_bar.append(&search);

    let chip_bar = gtk::FlowBox::new();
    chip_bar.add_css_class("ytd-chips");
    chip_bar.set_selection_mode(gtk::SelectionMode::None);
    chip_bar.set_max_children_per_line(12);
    chip_bar.set_column_spacing(6);
    chip_bar.set_row_spacing(6);
    filter_bar.append(&chip_bar);

    // ---- cards ----
    let grid = gtk::FlowBox::new();
    grid.add_css_class("ytd-grid");
    grid.set_selection_mode(gtk::SelectionMode::None);
    grid.set_homogeneous(true);
    grid.set_column_spacing(12);
    grid.set_row_spacing(12);

    let list = gtk::ListBox::new();
    list.add_css_class("ytd-list");
    list.set_selection_mode(gtk::SelectionMode::None);

    let view_stack = gtk::Stack::new();
    view_stack.set_hexpand(true);
    view_stack.set_vexpand(true);
    view_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    view_stack.add_named(&grid, Some(ViewMode::Grid.as_str()));
    view_stack.add_named(&list, Some(ViewMode::List.as_str()));

    let scroller = gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .child(&view_stack)
        .build();

    let empty_page = adw::StatusPage::new();
    empty_page.set_icon_name(Some("emblem-favorite-symbolic"));
    empty_page.set_vexpand(true);
    let empty_cta = gtk::Button::with_label("Añadir canal");
    empty_cta.add_css_class("pill");
    empty_cta.add_css_class("suggested-action");
    empty_page.set_child(Some(&empty_cta));

    let page_stack = gtk::Stack::new();
    page_stack.set_vexpand(true);
    page_stack.add_named(&scroller, Some("content"));
    page_stack.add_named(&empty_page, Some("empty"));

    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.append(&filter_bar);
    body.append(&page_stack);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&body));
    toast.set_child(Some(&toolbar));
    window.set_content(Some(&toast));

    let ui = Rc::new(Ui {
        app,
        application: application.clone(),
        window,
        toast,
        db_path,
        title,
        view_toggle,
        search,
        chip_bar,
        grid,
        list,
        view_stack,
        page_stack,
        empty_page,
        empty_cta,
        rendering: Cell::new(false),
        rendered: RefCell::new(Rendered::default()),
    });

    ui.install_actions();
    ui.connect_signals();
    ui.refresh();
    ui
}

fn primary_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some("_Añadir canal"), Some("win.add"));
    menu.append(Some("_Limpiar filtros"), Some("win.clear-filters"));

    let view = gio::Menu::new();
    view.append(Some("_Cuadrícula"), Some("win.view-grid"));
    view.append(Some("_Lista"), Some("win.view-list"));
    menu.append_section(Some("Vista"), &view);

    menu.append(
        Some("Abrir _carpeta de datos"),
        Some("win.open-data-folder"),
    );
    menu.append(Some("_Preferencias"), Some("win.preferences"));
    menu.append(Some("_Acerca de"), Some("win.about"));
    menu
}

impl Ui {
    pub fn window(&self) -> &adw::ApplicationWindow {
        &self.window
    }

    /// Re-renders from the current app state.
    pub fn refresh(self: &Rc<Self>) {
        if self.rendering.get() {
            return;
        }
        let app = self.app.borrow();
        self.rendering.set(true);
        self.render(&app);
        self.rendering.set(false);
    }

    fn render(self: &Rc<Self>, app: &App) {
        let visible = app.visible();
        let visible_ids: Vec<i64> = visible.iter().map(|c| c.id).collect();
        let mut rendered = self.rendered.borrow_mut();

        if rendered.revision != Some(app.revision)
            || rendered.view != Some(app.view)
            || rendered.visible != visible_ids
        {
            self.rebuild_cards(app, &visible);
            rendered.revision = Some(app.revision);
            rendered.view = Some(app.view);
            rendered.visible = visible_ids;
        }

        if rendered.tags != app.tags || rendered.selected != app.filter.tags {
            self.rebuild_chips(app);
            rendered.tags = app.tags.clone();
            rendered.selected = app.filter.tags.clone();
        }

        let wanted = if app.view == ViewMode::Grid {
            ViewMode::Grid.as_str()
        } else {
            ViewMode::List.as_str()
        };
        if self.view_stack.visible_child_name().as_deref() != Some(wanted) {
            self.view_stack.set_visible_child_name(wanted);
        }
        if self.view_toggle.active_name().as_deref() != Some(wanted) {
            self.view_toggle.set_active_name(Some(wanted));
        }

        let status = Self::status_for(app, &visible);
        if rendered.status != Some(status) {
            self.apply_status(status, app);
            rendered.status = Some(status);
        } else {
            self.title.set_subtitle(&count_subtitle(app));
        }
    }

    fn rebuild_cards(self: &Rc<Self>, _app: &App, visible: &[&Channel]) {
        self.grid.remove_all();
        self.list.remove_all();

        for channel in visible {
            let widget = card::build(self, channel, ViewMode::Grid);
            self.grid.append(&card::grid_child(widget));
        }
        for channel in visible {
            let widget = card::build(self, channel, ViewMode::List);
            self.list.append(&card::list_row(widget));
        }
    }

    fn rebuild_chips(self: &Rc<Self>, app: &App) {
        self.chip_bar.remove_all();
        self.chip_bar.set_visible(!app.tags.is_empty());

        for tag in &app.tags {
            let selected = app.filter.tags.iter().any(|t| t.eq_ignore_ascii_case(tag));

            let chip = gtk::ToggleButton::with_label(tag);
            chip.add_css_class("ytd-chip");
            // Set before wiring the handler so rebuilding never fires a toggle.
            chip.set_active(selected);

            let ui_weak = Rc::downgrade(self);
            let tag_clone = tag.clone();
            chip.connect_toggled(move |_| {
                let Some(ui) = ui_weak.upgrade() else {
                    return;
                };
                if ui.rendering.get() {
                    return;
                }
                ui.app.borrow_mut().toggle_tag(&tag_clone);
                ui.refresh();
            });
            let ui_weak2 = Rc::downgrade(self);
            let tag_for_menu = tag.clone();
            let chip_menu = chip.clone();
            let gesture = gtk::GestureClick::builder().button(gtk::gdk::BUTTON_SECONDARY).build();
            gesture.connect_released(move |_, _, x, y| {
                if let Some(ui) = ui_weak2.upgrade() {
                    ui.open_tag_menu(&chip_menu, &tag_for_menu, x, y);
                }
            });
            chip.add_controller(gesture);

            let cell = gtk::FlowBoxChild::new();
            cell.set_child(Some(&chip));
            self.chip_bar.append(&cell);
        }
    }

    fn status_for(app: &App, visible: &[&Channel]) -> Status {
        if app.total_count() == 0 {
            Status::NoChannels
        } else if visible.is_empty() && app.filter.is_active() {
            Status::NoMatches
        } else {
            Status::None
        }
    }

    fn apply_status(self: &Rc<Self>, status: Status, app: &App) {
        match status {
            Status::NoChannels => {
                self.empty_page.set_title("Aún no hay canales");
                self.empty_page
                    .set_description(Some("Añade tu primer canal de YouTube para empezar."));
                self.empty_page
                    .set_icon_name(Some("emblem-favorite-symbolic"));
                self.page_stack.set_visible_child_name("empty");
            }
            Status::NoMatches => {
                self.empty_page.set_title("Ningún canal coincide");
                self.empty_page
                    .set_description(Some("Prueba con otro nombre o quita los filtros activos."));
                self.empty_page.set_icon_name(Some("edit-find-symbolic"));
                self.page_stack.set_visible_child_name("empty");
            }
            Status::None => {
                self.page_stack.set_visible_child_name("content");
            }
        }

        let subtitle = count_subtitle(app);
        self.title.set_subtitle(&subtitle);
    }

    fn connect_signals(self: &Rc<Self>) {
        let ui_weak = Rc::downgrade(self);
        self.search.connect_search_changed(move |entry| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            if ui.rendering.get() {
                return;
            }
            ui.app.borrow_mut().set_query(&entry.text());
            ui.refresh();
        });

        let ui_weak = Rc::downgrade(self);
        self.view_toggle.connect_active_notify(move |toggle| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            if ui.rendering.get() {
                return;
            }
            let Some(name) = toggle.active_name() else {
                return;
            };
            let view = if name.as_str() == ViewMode::List.as_str() {
                ViewMode::List
            } else {
                ViewMode::Grid
            };
            {
                let mut app = ui.app.borrow_mut();
                if app.view == view {
                    return;
                }
                app.set_view(view);
            }
            ui.refresh();
        });

        let ui_weak = Rc::downgrade(self);
        self.empty_cta.connect_clicked(move |_| {
            if let Some(ui) = ui_weak.upgrade() {
                editor::open(&ui, None);
            }
        });

        let ui_weak = Rc::downgrade(self);
        self.window.connect_close_request(move |_| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.app.borrow().flush();
            }
            glib::Propagation::Proceed
        });
    }

    fn install_actions(self: &Rc<Self>) {
        self.add_action("add", move |ui| {
            editor::open(ui, None);
        });
        self.add_action("focus-search", |ui| {
            ui.search.grab_focus();
        });
        self.add_action("clear-filters", move |ui| {
            ui.search.set_text("");
            ui.app.borrow_mut().clear_filters();
            ui.refresh();
        });
        self.add_action("view-grid", move |ui| ui.set_view(ViewMode::Grid));
        self.add_action("view-list", move |ui| ui.set_view(ViewMode::List));
        self.add_action("open-data-folder", move |ui| ui.open_data_folder());
        self.add_action("preferences", prefs::open);
        self.add_action("about", move |ui| ui.show_about());
    }

    fn add_action<F>(self: &Rc<Self>, name: &str, handler: F)
    where
        F: Fn(&Rc<Ui>) + 'static,
    {
        let action = gio::SimpleAction::new(name, None);
        let ui_weak = Rc::downgrade(self);
        action.connect_activate(move |_, _| {
            if let Some(ui) = ui_weak.upgrade() {
                handler(&ui);
            }
        });
        self.application.add_action(&action);
        self.window.add_action(&action);
    }

    // ---- actions used by the cards and the menu ----

    pub fn open_channel(self: &Rc<Self>, id: i64) {
        let Some(url) = self.app.borrow().channel(id).map(|c| c.url.clone()) else {
            return;
        };
        let browser = self.app.borrow().config.browser.clone();

        match browser {
            config::Browser::System => self.launch_with_desktop(&url),
            config::Browser::Command(command) => {
                if let Err(message) = self.launch_with_command(&command, &url) {
                    self.notify(&message);
                }
            }
        }
    }

    /// Hands the URL to the desktop's default handler.
    fn launch_with_desktop(self: &Rc<Self>, url: &str) {
        let toast_weak = self.toast.downgrade();
        gtk::UriLauncher::new(url).launch(
            Some(&self.window),
            gio::Cancellable::NONE,
            move |result| {
                if let Err(error) = result {
                    if let Some(overlay) = toast_weak.upgrade() {
                        let toast = adw::Toast::new(&format!("No se pudo abrir el canal: {error}"));
                        toast.set_timeout(4);
                        overlay.add_toast(toast);
                    }
                }
            },
        );
    }

    /// Runs the configured browser with the URL appended, for instance
    /// `firefox -P "Perfil" https://…`.
    fn launch_with_command(self: &Rc<Self>, command: &str, url: &str) -> Result<(), String> {
        let mut parts = config::split_command(command);
        if parts.is_empty() {
            return Err("El comando del navegador está vacío".to_string());
        }
        let program = parts.remove(0);

        std::process::Command::new(&program)
            .args(&parts)
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|error| {
                format!("No se pudo ejecutar «{program}»: {error}. Revisa las preferencias.")
            })
    }

    pub fn copy_channel_url(&self, id: i64) {
        let Some(url) = self.app.borrow().channel(id).map(|c| c.url.clone()) else {
            return;
        };
        self.window.clipboard().set_text(&url);
        self.notify("Enlace copiado");
    }

    pub fn delete_channel(self: &Rc<Self>, id: i64) {
        let label = self.app.borrow().channel(id).map(Channel::label);
        let result = self.app.borrow_mut().delete_channel(id);
        match result {
            Ok(()) => {
                self.refresh();
                if let Some(label) = label {
                    self.notify(&format!("«{label}» eliminado"));
                }
            }
            Err(error) => self.notify(&format!("No se pudo eliminar: {error}")),
        }
    }

    pub fn reorder(self: &Rc<Self>, source: i64, target: i64, after: bool) {
        if let Err(error) = self.app.borrow_mut().move_channel(source, target, after) {
            self.notify(&format!("No se pudo reordenar: {error}"));
        }
        self.refresh();
    }

    pub fn notify(&self, message: &str) {
        let toast = adw::Toast::new(message);
        toast.set_timeout(2);
        self.toast.add_toast(toast);
    }

    fn set_view(self: &Rc<Self>, view: ViewMode) {
        {
            let mut app = self.app.borrow_mut();
            if app.view == view {
                return;
            }
            app.set_view(view);
        }
        self.refresh();
    }

    fn open_data_folder(self: &Rc<Self>) {
        let folder = self.db_path.parent().unwrap_or(Path::new("."));
        let uri = gtk::glib::filename_to_uri(folder, None)
            .unwrap_or_else(|_| format!("file://{}", folder.display()).into());
        let ui_weak = Rc::downgrade(self);
        gtk::UriLauncher::new(&uri).launch(
            Some(&self.window),
            gio::Cancellable::NONE,
            move |result| {
                if let (Err(error), Some(ui)) = (result, ui_weak.upgrade()) {
                    ui.notify(&format!("No se pudo abrir la carpeta: {error}"));
                }
            },
        );
    }

    pub fn open_tag_menu(self: &Rc<Self>, parent: &gtk::ToggleButton, tag: &str, x: f64, y: f64) {
        let popover = gtk::Popover::new();
        popover.set_has_arrow(false);
        popover.set_parent(parent);
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 4);
        menu.set_margin_top(6);
        menu.set_margin_bottom(6);
        menu.set_margin_start(6);
        menu.set_margin_end(6);
        popover.set_child(Some(&menu));
        let ui_weak = Rc::downgrade(self);
        let tag_for_rename = tag.to_string();
        let btn = gtk::Button::new();
        btn.set_child(Some(&{
            let r = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            r.append(&gtk::Image::from_icon_name("document-edit-symbolic"));
            r.append(&gtk::Label::new(Some("Renombrar etiqueta")));
            r.upcast::<gtk::Widget>()
        }));
        btn.add_css_class("flat");
        btn.set_hexpand(true);
        let ui_weak_r = ui_weak.clone();
        let t = tag_for_rename.clone();
        btn.connect_clicked(move |_| {
            if let Some(ui) = ui_weak_r.upgrade() {
                ui.rename_tag_dialog(&t);
            }
        });
        menu.append(&btn);
        let ui_weak_d = Rc::downgrade(self);
        let t2 = tag.to_string();
        let btn = gtk::Button::new();
        btn.set_child(Some(&{
            let r = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            r.append(&gtk::Image::from_icon_name("user-trash-symbolic"));
            r.append(&gtk::Label::new(Some("Eliminar etiqueta")));
            r.upcast::<gtk::Widget>()
        }));
        btn.add_css_class("flat");
        btn.set_hexpand(true);
        btn.add_css_class("error");
        btn.connect_clicked(move |_| {
            if let Some(ui) = ui_weak_d.upgrade() {
                ui.delete_tag_confirm(&t2);
            }
        });
        menu.append(&btn);
        popover.set_pointing_to(Some(&gtk::gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        popover.popup();
    }
    pub fn rename_tag_dialog(self: &Rc<Self>, tag: &str) {
        let dialog = adw::Dialog::new(); dialog.set_title("Renombrar etiqueta"); dialog.set_content_width(420);
        let toolbar = adw::ToolbarView::new(); let header = adw::HeaderBar::new();
        let cancel = gtk::Button::with_label("Cancelar"); cancel.add_css_class("flat");
        let weak=dialog.downgrade(); cancel.connect_clicked(move |_| if let Some(d)=weak.upgrade(){d.force_close();});
        header.pack_start(&cancel);
        let save_btn = gtk::Button::with_label("Guardar"); save_btn.add_css_class("suggested-action"); header.pack_end(&save_btn);
        let form = gtk::Box::new(gtk::Orientation::Vertical,10); form.set_margin_top(20); form.set_margin_bottom(20); form.set_margin_start(20); form.set_margin_end(20);
        let name = gtk::Entry::new(); name.set_text(tag); name.set_activates_default(true);
        let row = gtk::Box::new(gtk::Orientation::Horizontal,10);
        let icon = gtk::Image::from_icon_name("tag-symbolic");
        let lbl = gtk::Label::new(Some("Nuevo nombre"));
        row.append(&icon); row.append(&lbl); form.append(&row); form.append(&name);
        toolbar.add_top_bar(&header); toolbar.set_content(Some(&form)); dialog.set_child(Some(&toolbar));
        let ui_weak=Rc::downgrade(self); let tag_old=tag.to_string(); let weak=dialog.downgrade(); let name_clone=name.clone();
        save_btn.connect_clicked(move |_| {
            let Some(ui)=ui_weak.upgrade() else{return}; let Some(d)=weak.upgrade() else{return};
            let newn=name_clone.text().trim().to_string(); if newn.is_empty(){return;}
            if let Err(e)=ui.app.borrow_mut().rename_tag(&tag_old,&newn){ ui.notify(&format!("No se pudo renombrar: {e}")); return; }
            ui.refresh(); d.force_close();
        });
        dialog.set_default_widget(Some(&save_btn)); dialog.present(Some(&self.window));
    }
    pub fn delete_tag_confirm(self: &Rc<Self>, tag: &str) {
        let dialog = adw::AlertDialog::new(Some("Eliminar etiqueta"), Some(&format!("¿Eliminar la etiqueta «{}» de todos los canales?", tag)));
        dialog.add_response("cancel","Cancelar"); dialog.add_response("delete","Eliminar");
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        let ui_weak=Rc::downgrade(self); let tag_clone=tag.to_string();
        dialog.connect_response(None, move |_,res| {
            if res=="delete" {
                if let Some(ui)=ui_weak.upgrade() {
                    if let Err(e)=ui.app.borrow_mut().delete_tag(&tag_clone){ ui.notify(&format!("No se pudo eliminar: {e}")); } else { ui.refresh(); }
                }
            }
        });
        dialog.present(Some(&self.window));
    }

    fn show_about(&self) {
        let dialog = adw::AboutDialog::builder()
            .application_name("Canales de YouTube")
            .application_icon("applications-internet")
            .developer_name(env!("CARGO_PKG_NAME"))
            .version(env!("CARGO_PKG_VERSION"))
            .comments(format!(
                "Panel portable de canales de YouTube.\n\nAtajos: Ctrl+N añadir · Ctrl+F buscar · Ctrl+L limpiar filtros · Ctrl+1 cuadrícula · Ctrl+2 lista\n\nBase de datos: {}",
                self.db_path.display()
            ))
            .license_type(gtk::License::MitX11)
            .build();

        dialog.present(Some(&self.window));
    }
}

fn count_subtitle(app: &App) -> String {
    let visible = app.visible_count();
    if app.filter.is_active() {
        format!("{visible} de {}", app.total_count())
    } else {
        let total = app.total_count();
        if total == 1 {
            "1 canal".to_string()
        } else {
            format!("{total} canales")
        }
    }
}

pub fn register(app: &adw::Application) {
    app.set_accels_for_action("win.add", &["<Ctrl>n", "<Ctrl>i"]);
    app.set_accels_for_action("win.focus-search", &["<Ctrl>f", "<Control>slash"]);
    app.set_accels_for_action("win.clear-filters", &["<Ctrl>l", "<Ctrl>k"]);
    app.set_accels_for_action("win.view-grid", &["<Ctrl>1"]);
    app.set_accels_for_action("win.view-list", &["<Ctrl>2"]);
    app.set_accels_for_action("win.preferences", &["<Ctrl>comma"]);
}

pub fn app_id() -> &'static str {
    APP_ID
}
