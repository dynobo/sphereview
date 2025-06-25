use gio::{self, prelude::*};
use gtk4::prelude::*;
use libadwaita::Application;

pub struct App {
    app: Application,
}

impl App {
    pub fn new() -> Self {
        let app = Application::new(Some(crate::APP_ID), gio::ApplicationFlags::empty());

        App::setup_actions(&app);
        App::setup_accels(&app);

        Self { app }
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

    pub fn run(self, args: Vec<String>) {
        self.app.connect_activate(|app| {
            // Show window
            crate::window::Window::new(app).present();
        });
        self.app.run_with_args(&args);
    }
}
