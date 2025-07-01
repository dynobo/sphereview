mod app;
mod image;
mod shortcuts_dialog;
mod window;

pub const APP_VERSION: &str = "0.1.0";
pub const APP_ID: &str = "io.github.dynobo.sphereview";

use app::App;

fn main() {
    gtk4::init().expect("Error initializing gtk.");
    libadwaita::init().expect("Error initializing libadwaita.");
    glib::set_application_name("SphereView");
    glib::set_prgname(Some("sphereview"));

    let args: Vec<String> = std::env::args().collect();
    App::new().run(args);
}
