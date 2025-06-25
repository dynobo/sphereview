mod app;
mod assets;
mod image;
mod shortcuts_dialog;
mod window;

pub const APP_VERSION: &str = "0.1.0";
pub const APP_ID: &str = "com.github.dynobo.sphereview";

use app::App;

fn main() {
    let mut args: Vec<String> = std::env::args().collect();

    // Version flag
    if args.iter().any(|arg| arg == "--version") {
        println!("SphereView v{}", APP_VERSION);
        return;
    }

    // Logging
    let log_level = if args.iter().any(|arg| arg == "-v" || arg == "--verbose") {
        args.retain(|arg| arg != "-v" && arg != "--verbose");
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Warn
    };
    env_logger::Builder::new().filter_level(log_level).init();

    // Gtk
    gtk4::init().expect("Error initializing gtk.");
    libadwaita::init().expect("Error initializing libadwaita.");
    gtk4::Window::set_default_icon_name(APP_ID);
    glib::set_application_name("SphereView");
    glib::set_prgname(Some("sphereview"));

    App::new().run(args);
}
