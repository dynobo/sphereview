mod app;
mod image;
mod shortcuts_dialog;
mod window;

use app::App;
use sphereview::{APP_ID, APP_VERSION};

fn main() {
    gtk4::init().expect("Error initializing gtk.");
    libadwaita::init().expect("Error initializing libadwaita.");
    glib::set_application_name("SphereView");
    glib::set_prgname(Some("sphereview"));

    let args: Vec<String> = std::env::args().collect();
    App::new().run(args);
}
