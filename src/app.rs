use gio::{self, prelude::*};
use gtk4::prelude::*;
use libadwaita::Application;

static GRESOURCE_BYTES: &[u8] =
    include_bytes!("../resources/com.github.dynobo.sphereview.gresource");

pub struct App {
    app: Application,
}

impl App {
    pub fn new() -> Self {
        let app = Application::new(Some(crate::APP_ID), gio::ApplicationFlags::HANDLES_OPEN);

        App::setup_flags(&app);
        App::setup_resources(&app);
        App::setup_actions(&app);
        App::setup_accels(&app);

        Self { app }
    }

    fn setup_resources(_app: &Application) {
        let resource = gio::Resource::from_data(&glib::Bytes::from_static(GRESOURCE_BYTES))
            .expect("Failed to load resource");
        gio::resources_register(&resource);
    }

    fn setup_actions(app: &Application) {
        let quit_action = gio::SimpleAction::new("quit", None);
        let app_weak = app.downgrade();
        quit_action.connect_activate(move |_, _| {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            app.quit();
        });
        app.add_action(&quit_action);
    }

    fn setup_accels(app: &Application) {
        app.set_accels_for_action("app.quit", &["<Ctrl>q"]);
        app.set_accels_for_action("win.keyboard-shortcuts", &["<Ctrl>question"]);
        app.set_accels_for_action("win.toggle-fullscreen", &["F11"]);
        app.set_accels_for_action("win.open-file", &["<Ctrl>o"]);
    }

    fn setup_flags(app: &Application) {
        app.add_main_option(
            "version",
            glib::Char::from(0),
            glib::OptionFlags::NONE,
            glib::OptionArg::None,
            "Show version",
            None,
        );
        app.add_main_option(
            "verbose",
            glib::Char::from(b'v'),
            glib::OptionFlags::NONE,
            glib::OptionArg::None,
            "Enable verbose logging",
            None,
        );
        app.set_option_context_parameter_string(Some("[FILE]"));
        app.set_option_context_summary(Some(
            "Image viewer for 360° equirectangular photospheres and panoramas.",
        ));
    }

    pub fn run(self, args: Vec<String>) {
        // Version
        if args.iter().any(|arg| arg == "--version") {
            println!("SphereView v{}", crate::APP_VERSION);
            std::process::exit(0);
        }

        // Logging
        let log_level = if args.iter().any(|arg| arg == "-v" || arg == "--verbose") {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Warn
        };
        env_logger::Builder::new().filter_level(log_level).init();

        // Default starting behavior
        self.app.connect_activate(move |app| {
            crate::window::Window::new(app).present();
        });

        // Triggered when started with a file as argument
        self.app.connect_open(|app, files, _| {
            let Some(file) = files.first() else { return };
            let Some(path) = file.path() else { return };

            let window = crate::window::Window::new(app);
            window.set_initial_file(Some(path));
            window.present();
        });

        self.app.run_with_args(&args);
    }
}
