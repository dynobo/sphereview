use base64::Engine;
use glib::{self, subclass};
use gtk4::{self, CompositeTemplate, gio, prelude::*};
use libadwaita::{self, prelude::*, subclass::prelude::*};
use log::{debug, error};
use once_cell::sync::Lazy;
use std::cell::RefCell;
use std::path::PathBuf;
use webkit6::{self, WebView, prelude::*};

use crate::shortcuts_dialog::ShortcutsDialog;

// Default initial directory for Open File dialog
pub static PICTURES_DIR: Lazy<PathBuf> = Lazy::new(|| {
    glib::user_special_dir(glib::UserDirectory::Pictures)
        .unwrap_or_else(|| PathBuf::from(glib::home_dir()))
});

mod imp {
    use super::*;

    #[derive(CompositeTemplate, Default)]
    #[template(file = "resources/data/window.blp")]
    pub struct Window {
        #[template_child]
        pub web_view: TemplateChild<WebView>,
        #[template_child]
        pub open_button: TemplateChild<gtk4::Button>,
        #[template_child]
        pub window_title: TemplateChild<gtk4::Label>,
        #[template_child]
        pub menu_button: TemplateChild<gtk4::MenuButton>,
        #[template_child]
        pub headerbar: TemplateChild<libadwaita::HeaderBar>,

        pub open_file_dialog: RefCell<Option<gtk4::FileDialog>>,

        // Used to derive the initial directory for Open File Dialog when the user
        // opened a Panorama file
        pub active_panorama_file: RefCell<Option<PathBuf>>,

        // If a file get passed as cli argument, this will be set via `app.rs`
        pub cli_arg_panorama_file: RefCell<Option<PathBuf>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "Window";
        type Type = super::Window;
        type ParentType = libadwaita::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Window {}
    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends gtk4::Widget, gtk4::Window, gtk4::ApplicationWindow, libadwaita::ApplicationWindow;
}

impl Window {
    pub fn new(app: &libadwaita::Application) -> Self {
        let obj: Self = glib::Object::new();
        obj.set_application(Some(app));
        obj.set_icon_name(Some(crate::APP_ID));
        obj.setup_actions();
        obj.setup_webview();
        obj.setup_webview_drop_handler();
        obj.setup_menu();
        obj.setup_open_file_button();
        obj.setup_open_file_dialog();
        obj.setup_fullscreen_handler();
        obj
    }

    pub fn present(&self) {
        self.upcast_ref::<libadwaita::ApplicationWindow>().present();
    }

    fn setup_actions(&self) {
        let window = self.upcast_ref::<gtk4::ApplicationWindow>();

        // Open File action
        let open_file_action = gtk4::gio::SimpleAction::new("open-file", None);
        let window_weak = self.downgrade();
        open_file_action.connect_activate(move |_, _| {
            let Some(window) = window_weak.upgrade() else {
                return;
            };
            window.show_file_chooser_dialog();
        });
        window.add_action(&open_file_action);

        // About action
        let about_action = gio::SimpleAction::new("about", None);
        let window_weak = self.downgrade();
        about_action.connect_activate(move |_, _| {
            let Some(window) = window_weak.upgrade() else {
                return;
            };
            window.show_about_dialog();
        });
        window.add_action(&about_action);

        // Shortcuts action
        let shortcuts_action = gio::SimpleAction::new("keyboard-shortcuts", None);
        let window_weak = self.downgrade();
        shortcuts_action.connect_activate(move |_, _| {
            let Some(window) = window_weak.upgrade() else {
                return;
            };
            window.show_keyboard_shortcuts_dialog();
        });
        window.add_action(&shortcuts_action);

        // Fullscreen action
        let fullscreen_action = gio::SimpleAction::new("toggle-fullscreen", None);
        let window_weak = self.downgrade();
        fullscreen_action.connect_activate(move |_, _| {
            let Some(window) = window_weak.upgrade() else {
                return;
            };
            if window.is_fullscreen() {
                window.unfullscreen();
            } else {
                window.fullscreen();
            }
        });
        window.add_action(&fullscreen_action);

        // Zoom in
        let zoom_action = gtk4::gio::SimpleAction::new("zoom-in", None);
        let webview = self.imp().web_view.get();
        zoom_action.connect_activate(move |_, _| {
            webview.evaluate_javascript(
                "window.viewer.zoomIn(20)",
                None,
                None,
                None::<&gio::Cancellable>,
                |result| {
                    if let Err(e) = result {
                        error!("Failed to evaluate JavaScript: {}", e);
                    }
                },
            );
        });
        window.add_action(&zoom_action);

        // Zoom out
        let zoom_action = gtk4::gio::SimpleAction::new("zoom-out", None);
        let webview = self.imp().web_view.get();
        zoom_action.connect_activate(move |_, _| {
            webview.evaluate_javascript(
                "window.viewer.zoomOut(20)",
                None,
                None,
                None::<&gio::Cancellable>,
                |result| {
                    if let Err(e) = result {
                        error!("Failed to evaluate JavaScript: {}", e);
                    }
                },
            );
        });
        window.add_action(&zoom_action);
    }

    fn setup_webview(&self) {
        let imp = self.imp();
        let web_view = imp.web_view.get();
        if let Some(settings) = webkit6::prelude::WebViewExt::settings(&web_view) {
            // Print js console messages when log level is Debug
            if log::log_enabled!(log::Level::Debug) {
                settings.set_enable_write_console_messages_to_stdout(true);
            }

            // Disable unnecessary webview features
            settings.set_allow_modal_dialogs(false);
            settings.set_enable_html5_database(false);
            settings.set_enable_media(false);
            settings.set_enable_media_stream(false);
            settings.set_enable_mediasource(false);
            settings.set_enable_tabs_to_links(false);
            settings.set_enable_webaudio(false);
            settings.set_enable_webrtc(false);
        }

        // Index.html embeddes the PhotoSphereViewer.js and custom logic for loading
        // panorama images via JS. All CSS and JS is inlined and minified.
        // See ./resources/photospherviewer for the source files and build process.
        let html = gio::resources_lookup_data(
            "/io/github/dynobo/sphereview/assets/index.html",
            gio::ResourceLookupFlags::NONE,
        )
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_else(|_| "<h1>index.html not found in GIO resources</h1>".to_string());

        web_view.connect_context_menu(move |_, _, _| true);
        web_view.load_html(&html, Some("sphere://viewer"));

        let window_weak = self.downgrade();
        web_view.connect_load_changed(move |_, event| {
            if event != webkit6::LoadEvent::Finished {
                return;
            }

            let Some(window) = window_weak.upgrade() else {
                return;
            };

            // Load CLI panorama if provided, otherwise demo panorama will be shown
            match window.imp().cli_arg_panorama_file.borrow().as_ref() {
                Some(cli_file) => {
                    if let Err(e) = window.show_panorama(Some(cli_file)) {
                        error!("Failed to load panorama from CLI argument: {}", e);
                    }
                }
                None => {
                    if let Err(e) = window.show_panorama(None) {
                        error!("Failed to load demo panorama: {}", e);
                    }
                }
            }
        });
    }

    fn setup_webview_drop_handler(&self) {
        let imp = self.imp();
        let web_view = imp.web_view.get();

        let drop_target = gtk4::DropTarget::builder()
            .actions(gtk4::gdk::DragAction::COPY)
            .formats(&gtk4::gdk::ContentFormats::for_type(
                gtk4::gio::File::static_type(),
            ))
            .build();

        let window_weak = self.downgrade();
        drop_target.connect_drop(move |_, value, _, _| {
            let Some(window) = window_weak.upgrade() else {
                return false;
            };

            value
                .get::<gtk4::gio::File>()
                .ok()
                .and_then(|file| file.path())
                .map_or(false, |path| {
                    debug!("File dropped: {:?}", path);
                    window.show_panorama(Some(&path)).is_ok()
                })
        });

        web_view.add_controller(drop_target);
    }

    fn setup_menu(&self) {
        let imp = self.imp();

        let menu = gio::Menu::new();
        menu.append(Some("Keyboard Shortcuts"), Some("win.keyboard-shortcuts"));
        menu.append(Some("About SphereView"), Some("win.about"));

        let popover_menu = gtk4::PopoverMenu::from_model(Some(&menu));

        let menu_button = imp.menu_button.get();
        menu_button.set_popover(Some(&popover_menu));
    }

    fn setup_open_file_button(&self) {
        let imp = self.imp();
        let window_weak = self.downgrade();
        imp.open_button.connect_clicked(move |_| {
            let Some(window) = window_weak.upgrade() else {
                return;
            };
            let _ = window.activate_action("open-file", None);
        });
    }

    fn setup_open_file_dialog(&self) {
        let file_dialog = gtk4::FileDialog::builder()
            .title("Open Image")
            .accept_label("Open")
            .modal(true)
            .build();

        // Add file filter for supported image types
        let images_filter = gtk4::FileFilter::new();
        images_filter.set_name(Some("Images"));
        for (_, mime_type) in crate::image::SUPPORTED_FILE_TYPES {
            images_filter.add_mime_type(mime_type);
        }

        // Add file filter for all files
        let all_files_filter = gtk4::FileFilter::new();
        all_files_filter.set_name(Some("All Files"));
        all_files_filter.add_pattern("*");

        let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&images_filter);
        filters.append(&all_files_filter);

        file_dialog.set_filters(Some(&filters));
        file_dialog.set_default_filter(Some(&images_filter));

        self.imp().open_file_dialog.replace(Some(file_dialog));
    }

    fn setup_fullscreen_handler(&self) {
        // Hide HeaderBar during fullscreen mode
        let imp = self.imp();
        let headerbar = imp.headerbar.get();
        self.connect_notify_local(Some("fullscreened"), move |window, _| {
            headerbar.set_visible(!window.is_fullscreen());
        });
    }

    fn show_about_dialog(&self) {
        let window = self.upcast_ref::<gtk4::Window>();
        let about = libadwaita::AboutDialog::builder()
            .application_name("SphereView")
            .license_type(gtk4::License::MitX11)
            .version(crate::APP_VERSION)
            .website("https://github.com/dynobo/sphereview")
            .issue_url("https://github.com/dynobo/sphereview")
            .application_icon(crate::APP_ID)
            .developer_name("by dynobo")
            .comments(
                "Image viewer for 360° equirectangular photospheres and panoramas.\n\n\
                Standing on the shoulders of the JavaScript library Photo Sphere Viewer.\n\n\
                Written in Rust and JavaScript.",
            )
            .build();
        about.present(Some(window));
    }

    fn show_keyboard_shortcuts_dialog(&self) {
        let window = self.upcast_ref::<gtk4::Window>();
        let shortcuts_window = ShortcutsDialog::new(window);
        shortcuts_window.present();
    }

    fn show_file_chooser_dialog(&self) {
        let window = self.upcast_ref::<gtk4::Window>();
        let imp = self.imp();

        let file_dialog = imp
            .open_file_dialog
            .borrow()
            .clone()
            .expect("File dialog not created");

        let initial_folder = self
            .imp()
            .active_panorama_file
            .borrow()
            .clone()
            .and_then(|path| path.parent().map(gio::File::for_path))
            .unwrap_or_else(|| gio::File::for_path(PICTURES_DIR.clone()));

        file_dialog.set_initial_folder(Some(&initial_folder));

        let window_weak = self.downgrade();
        file_dialog.open(Some(window), None::<&gio::Cancellable>, move |res| {
            let Some(window) = window_weak.upgrade() else {
                return;
            };
            let Some(path) = res.ok().and_then(|file| file.path()) else {
                return;
            };
            if let Err(e) = window.show_panorama(Some(&path)) {
                error!("Failed to show image: {}", e);
            }
        });
    }

    fn show_panorama(&self, panorama_file: Option<&PathBuf>) -> Result<(), String> {
        // Encode the image bytes to base64, insert into a JS snippet as argument of
        // JS function defined in /resources/photosphereviewer/src/viewer.js.
        // Then the JS is executed in the html page of the webview.
        // In viewer.js, the function will decode the base64 image, store it as a blob
        // and pass that to the Photo Sphere Viewer.
        //
        // Alternative implementations that didn't work out:
        // - Pass image via custom uri scheme -> PSV couldn't load it for unknown reason
        // - Load image as data url directly -> Fails for large image because of url size limit
        // - Open a local webserver -> Quite ugly solution, also unclear security implications

        let imp = self.imp();
        let webview = imp.web_view.get();
        let title = imp.window_title.get();

        *imp.active_panorama_file.borrow_mut() = panorama_file.cloned();

        let panorama = match panorama_file {
            Some(p) => crate::image::from_file(p),
            None => crate::image::from_demo(),
        };

        title.set_text(&panorama.filename);

        if panorama.data.is_empty() {
            return Err("No panorama data available".to_string());
        }

        let base64 = base64::engine::general_purpose::STANDARD.encode(&panorama.data);
        let js = format!(
            "window.setPanoramaImageFromBase64('{}', '{}');",
            base64, panorama.mime_type
        );

        debug!(
            "Injecting panorama file '{:?}', mime_type: {}, base64 length: {}",
            panorama_file,
            panorama.mime_type,
            base64.len()
        );

        webview.evaluate_javascript(&js, None, None, None::<&gio::Cancellable>, |result| {
            if let Err(e) = result {
                error!("Failed to evaluate JavaScript: {}", e);
            }
        });

        Ok(())
    }

    pub fn set_initial_file(&self, path: Option<PathBuf>) {
        *self.imp().cli_arg_panorama_file.borrow_mut() = path;
    }
}
