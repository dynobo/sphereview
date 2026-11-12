use base64::Engine;
use glib::{self, subclass};
use gtk4::{self, CompositeTemplate, gio, prelude::*};
use libadwaita::{self, prelude::*, subclass::prelude::*};
use log::{debug, error};
use once_cell::sync::Lazy;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use webkit6::{self, WebView, prelude::*};

use crate::shortcuts_dialog::create_shortcuts_dialog;

// Default initial directory for Open File dialog
pub static PICTURES_DIR: Lazy<PathBuf> = Lazy::new(|| {
    glib::user_special_dir(glib::UserDirectory::Pictures).unwrap_or_else(glib::home_dir)
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

        // The projection action, kept so the file's own preference can be applied
        // when it is loaded.
        pub projection_action: RefCell<Option<gio::SimpleAction>>,
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

    impl Window {
        /// The projection currently asked for. The action holds it, so the menu,
        /// the shortcut and the viewer cannot disagree.
        pub fn projection(&self) -> Projection {
            self.projection_action
                .borrow()
                .as_ref()
                .and_then(|action| action.state())
                .and_then(|state| state.get::<String>())
                .map(|state| Projection::from_str(&state))
                .unwrap_or(Projection::Equirectangular)
        }

        /// Sets the projection without re-rendering, for a freshly opened file.
        pub fn set_projection(&self, projection: Projection) {
            if let Some(action) = self.projection_action.borrow().as_ref() {
                action.set_state(&projection.as_str().to_variant());
            }
        }
    }
}

glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
        @extends gtk4::Widget, gtk4::Window, gtk4::ApplicationWindow, libadwaita::ApplicationWindow,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget, gtk4::Native, gtk4::Root, gtk4::ShortcutManager, gio::ActionGroup, gio::ActionMap;
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

        // Projection: the flat image, or the two fisheye circles wrapped on a
        // sphere. Always available, whatever the file holds.
        let projection_action = gio::SimpleAction::new_stateful(
            "projection",
            Some(glib::VariantTy::STRING),
            &Projection::Equirectangular.as_str().to_variant(),
        );
        let window_weak = self.downgrade();
        projection_action.connect_change_state(move |action, value| {
            let Some(window) = window_weak.upgrade() else {
                return;
            };
            let Some(state) = value else { return };
            let Some(mode) = state.get::<String>() else {
                return;
            };
            action.set_state(state);
            window.imp().set_projection(Projection::from_str(&mode));
            window.refresh_panorama();
        });
        *self.imp().projection_action.borrow_mut() = Some(projection_action.clone());
        window.add_action(&projection_action);

        // The same thing as one keypress, for the shortcut.
        let toggle_projection_action = gio::SimpleAction::new("toggle-projection", None);
        let window_weak = self.downgrade();
        toggle_projection_action.connect_activate(move |_, _| {
            let Some(window) = window_weak.upgrade() else {
                return;
            };
            let next = window.imp().projection().toggled();
            let Some(action) = window.imp().projection_action.borrow().as_ref().cloned() else {
                return;
            };
            action.change_state(&next.as_str().to_variant());
        });
        window.add_action(&toggle_projection_action);

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

        // Toggle navbar action
        let toggle_navbar_action =
            gio::SimpleAction::new_stateful("toggle-navbar", None, &true.to_variant());
        let window_weak = self.downgrade();
        toggle_navbar_action.connect_change_state(move |action, value| {
            let Some(window) = window_weak.upgrade() else {
                return;
            };
            if let Some(state) = value {
                if let Some(new_state) = state.get::<bool>() {
                    action.set_state(state);
                    window.set_navbar_visibility(new_state);
                }
            }
        });

        window.add_action(&toggle_navbar_action);

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
                .is_some_and(|path| {
                    debug!("File dropped: {:?}", path);
                    window.show_panorama(Some(&path)).is_ok()
                })
        });

        web_view.add_controller(drop_target);
    }

    fn setup_menu(&self) {
        let imp = self.imp();
        let menu = gio::Menu::new();
        menu.append(Some("Show Navigation Bar"), Some("win.toggle-navbar"));

        // Radio items for the projection: both states are named, and both are
        // always offered, whatever the file holds.
        let projection_section = gio::Menu::new();
        for mode in Projection::ALL {
            let item = gio::MenuItem::new(Some(mode.label()), None);
            item.set_action_and_target_value(
                Some("win.projection"),
                Some(&mode.as_str().to_variant()),
            );
            projection_section.append_item(&item);
        }
        menu.append_section(Some("Projection"), &projection_section);

        let other_section = gio::Menu::new();
        other_section.append(Some("Keyboard Shortcuts"), Some("win.keyboard-shortcuts"));
        other_section.append(Some("About SphereView"), Some("win.about"));
        menu.append_section(None, &other_section);
        let popover_menu = gtk4::PopoverMenu::from_model(Some(&menu));

        let menu_button = imp.menu_button.get();
        menu_button.set_popover(Some(&popover_menu));

        let action = gio::SimpleAction::new("toggle-menu", None);
        action.connect_activate(move |_, _| {
            menu_button.activate();
        });
        self.add_action(&action);
    }

    fn setup_open_file_button(&self) {
        let imp = self.imp();
        let window_weak = self.downgrade();
        imp.open_button.connect_clicked(move |_| {
            let Some(window) = window_weak.upgrade() else {
                return;
            };
            let _ = gtk4::prelude::WidgetExt::activate_action(&window, "open-file", None);
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
        for (_, mime_type) in sphereview::image::SUPPORTED_FILE_TYPES {
            images_filter.add_mime_type(mime_type);
        }
        // `.insp` has no MIME type of its own and no glob to be sniffed by, so
        // give the filter a pattern as well as the JPEG sniffing.
        images_filter.add_suffix("insp");

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
            .issue_url("https://github.com/dynobo/sphereview/issues")
            .application_icon(crate::APP_ID)
            .developer_name("by dynobo")
            .debug_info(format!(
                "GTK {}.{}.{}\nAdwaita {}.{}.{}\nWebKitGTK: {}.{}.{}",
                gtk4::major_version(),
                gtk4::minor_version(),
                gtk4::micro_version(),
                libadwaita::major_version(),
                libadwaita::minor_version(),
                libadwaita::micro_version(),
                webkit6::functions::major_version(),
                webkit6::functions::minor_version(),
                webkit6::functions::micro_version()
            ))
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
        let shortcuts_dialog = create_shortcuts_dialog();
        shortcuts_dialog.present(Some(window));
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

    fn set_navbar_visibility(&self, visible: bool) {
        let imp = self.imp();
        let webview = imp.web_view.get();
        let method = if visible { "show" } else { "hide" };
        let js = format!("window.SphereViewer.navbar.{}();", method);

        webview.evaluate_javascript(&js, None, None, None::<&gio::Cancellable>, |result| {
            if let Err(e) = result {
                error!("Failed to evaluate JavaScript: {}", e);
            }
        });
    }

    fn show_panorama(&self, panorama_file: Option<&PathBuf>) -> Result<(), String> {
        // A new file starts with the projection its own metadata asks for.
        let panorama = match panorama_file {
            Some(p) => sphereview::image::from_file(p),
            None => sphereview::image::from_demo(),
        };
        let default = if panorama.dual_fisheye.is_some() {
            Projection::DualFisheye
        } else {
            Projection::Equirectangular
        };
        self.imp().set_projection(default);

        *self.imp().active_panorama_file.borrow_mut() = panorama_file.cloned();
        self.render_panorama_with(panorama)
    }

    /// Re-sends the current file, after the projection was switched.
    fn refresh_panorama(&self) {
        if let Err(e) = self.render_panorama() {
            error!("Failed to refresh image: {}", e);
        }
    }

    /// Sends the active file to the viewer.
    ///
    /// Encode the image bytes to base64, insert into a JS snippet as argument of
    /// JS function defined in /resources/photosphereviewer/src/viewer.js.
    /// Then the JS is executed in the html page of the webview.
    /// In viewer.js, the function will decode the base64 image, store it as a blob
    /// and pass that to the Photo Sphere Viewer.
    ///
    /// Alternative implementations that didn't work out:
    /// - Pass image via custom uri scheme -> PSV couldn't load it for unknown reason
    /// - Load image as data url directly -> Fails for large image because of url size limit
    /// - Open a local webserver -> Quite ugly solution, also unclear security implications
    fn render_panorama(&self) -> Result<(), String> {
        let panorama_file = self.imp().active_panorama_file.borrow().clone();
        let panorama = match &panorama_file {
            Some(p) => sphereview::image::from_file(p),
            None => sphereview::image::from_demo(),
        };
        self.render_panorama_with(panorama)
    }

    fn render_panorama_with(&self, panorama: sphereview::image::ImageData) -> Result<(), String> {
        let imp = self.imp();
        let webview = imp.web_view.get();
        let title = imp.window_title.get();

        title.set_text(&panorama.filename);

        if panorama.data.is_empty() {
            return Err("No panorama data available".to_string());
        }

        // Four arguments even for a flat image: the page keys each photo's
        // rotation by the file key, and needs it whichever way the photo is
        // shown. The third argument is the projection, `null` for an
        // equirectangular image.
        let projection = imp
            .projection()
            .argument(panorama.dual_fisheye.as_deref())
            .unwrap_or_else(|| "null".to_string());
        let key = file_key(imp.active_panorama_file.borrow().as_deref());

        let base64 = base64::engine::general_purpose::STANDARD.encode(&panorama.data);
        let js = format!(
            "window.setPanoramaImageFromBase64('{}', '{}', {}, {});",
            base64, panorama.mime_type, projection, key
        );

        debug!(
            "Injecting panorama file '{:?}', mime_type: {}, base64 length: {}, projection: {:?}, file key: {}",
            imp.active_panorama_file.borrow(),
            panorama.mime_type,
            base64.len(),
            imp.projection(),
            key
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

/// A stable id for a photo within this run, so the page can tell a file it has
/// already seen - a projection switch re-sends the same one - from a new one.
/// It keys the rotation the page remembers per photo. The demo image has no
/// path and hashes to whatever `None` hashes to, which is stable too.
///
/// Cut down to 53 bits, the width of a double's mantissa: the key travels to
/// the page as a number literal, and a wider one would arrive rounded, which
/// would make two photos share a rotation. A collision is now a 1 in 2^53
/// accident instead, and the worst it costs is one photo inheriting another's.
fn file_key(path: Option<&Path>) -> u64 {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish() & ((1 << 53) - 1)
}

/// How the image is shown: flat, or wrapped on a sphere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Projection {
    Equirectangular,
    DualFisheye,
}

impl Projection {
    pub const ALL: [Projection; 2] = [Projection::Equirectangular, Projection::DualFisheye];

    fn label(self) -> &'static str {
        match self {
            Projection::Equirectangular => "Equirectangular",
            Projection::DualFisheye => "Dual Fisheye",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Projection::Equirectangular => "equirectangular",
            Projection::DualFisheye => "dual-fisheye",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "dual-fisheye" => Projection::DualFisheye,
            _ => Projection::Equirectangular,
        }
    }

    fn toggled(self) -> Self {
        match self {
            Projection::Equirectangular => Projection::DualFisheye,
            Projection::DualFisheye => Projection::Equirectangular,
        }
    }

    /// The third argument for the viewer, or `None` for the flat image.
    ///
    /// A calibration is used when the file has one. Without it, dual fisheye
    /// still means something: the viewer falls back to the standard layout of two
    /// 180° circles and says so.
    fn argument(self, calibration: Option<&str>) -> Option<String> {
        match (self, calibration) {
            (Projection::Equirectangular, _) | (Projection::DualFisheye, None) => {
                if self == Projection::DualFisheye {
                    Some("{\"fallback\":true}".to_string())
                } else {
                    None
                }
            }
            (Projection::DualFisheye, Some(calibration)) => Some(calibration.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Projection;

    #[test]
    fn the_flat_mode_sends_no_projection() {
        assert_eq!(Projection::Equirectangular.argument(Some("{}")), None);
        assert_eq!(Projection::Equirectangular.argument(None), None);
    }

    #[test]
    fn a_calibration_is_passed_through() {
        assert_eq!(
            Projection::DualFisheye.argument(Some("{\"lens_count\":2}")),
            Some("{\"lens_count\":2}".to_string())
        );
    }

    #[test]
    fn without_a_calibration_the_viewer_is_asked_to_guess() {
        assert_eq!(
            Projection::DualFisheye.argument(None),
            Some("{\"fallback\":true}".to_string())
        );
    }

    #[test]
    fn the_shortcut_flips_between_the_two_modes() {
        assert_eq!(
            Projection::Equirectangular.toggled(),
            Projection::DualFisheye
        );
        assert_eq!(
            Projection::DualFisheye.toggled(),
            Projection::Equirectangular
        );
    }

    #[test]
    fn menu_states_round_trip_through_their_names() {
        for mode in Projection::ALL {
            assert_eq!(Projection::from_str(mode.as_str()), mode);
        }
    }
}
