//! Portable desktop dashboard to organize and open YouTube channels.
//!
//! Built with Rust + GTK4 + libadwaita and a single embedded SQLite file, so
//! the whole application is one binary plus one `.db` file that can live next
//! to the executable.

mod config;
mod db;
mod model;
mod state;
mod ui;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use db::Database;
use state::App;

const HELP: &str = "\
Canales de YouTube — panel portable de canales

USO:
    ytdash [OPCIONES]

OPCIONES:
    -d, --db <RUTA>    Usa esta base de datos SQLite
        --version      Muestra la versión y termina
    -h, --help         Muestra esta ayuda

Si no se indica una ruta, la base de datos se crea junto al ejecutable
(«portable»). Si ese directorio no permite escritura, se usa
$XDG_DATA_HOME/ytdash/ytdash.db. La variable YTDASH_DB tiene prioridad sobre
la detección automática.
";

struct Args {
    db_path: Option<PathBuf>,
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut db_path = None;
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(None);
            }
            "--version" | "-V" => {
                println!("ytdash {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "-d" | "--db" => match args.next() {
                Some(path) => db_path = Some(PathBuf::from(path)),
                None => return Err(format!("{arg} necesita una ruta")),
            },
            other if other.starts_with("--db=") => {
                db_path = Some(PathBuf::from(&other[5..]));
            }
            other => return Err(format!("opción desconocida: {other}")),
        }
    }

    Ok(Some(Args { db_path }))
}

fn main() -> glib::ExitCode {
    let args = match parse_args() {
        Ok(None) => return glib::ExitCode::SUCCESS,
        Ok(Some(args)) => args,
        Err(message) => {
            eprintln!("ytdash: {message}\n");
            eprint!("{HELP}");
            return glib::ExitCode::FAILURE;
        }
    };

    let db_path = db::resolve_db_path(args.db_path);
    let database = match Database::open(&db_path) {
        Ok(database) => database,
        Err(error) => {
            eprintln!("ytdash: no se pudo abrir {}: {error}", db_path.display());
            return glib::ExitCode::FAILURE;
        }
    };

    let app_state = match App::new(database, db_path.clone()) {
        Ok(state) => state,
        Err(error) => {
            eprintln!("ytdash: no se pudo leer {}: {error}", db_path.display());
            return glib::ExitCode::FAILURE;
        }
    };

    let state = Rc::new(RefCell::new(app_state));

    let application = adw::Application::builder()
        .application_id(ui::app_id())
        .build();
    ui::register(&application);

    // The signal handlers only hold weak references to `Ui`, so something has to
    // keep a strong one alive for as long as the window exists. Without this the
    // whole UI is dropped right after `build` returns and every callback
    // silently does nothing.
    let window = Rc::new(std::cell::RefCell::new(None::<Rc<ui::Ui>>));

    let window_ref = Rc::downgrade(&window);
    application.connect_activate(move |app| {
        let existing = window_ref.upgrade().and_then(|slot| slot.borrow().clone());
        if let Some(ui) = existing {
            ui.window().present();
            return;
        }

        let ui = ui::build(state.clone(), app);
        if let Some(slot) = window_ref.upgrade() {
            *slot.borrow_mut() = Some(ui.clone());
        }
        ui.window().present();
    });

    application.run()
}
