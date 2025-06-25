use glib::{self, subclass};
use gtk4::gdk::ModifierType;
use gtk4::{self, CompositeTemplate, prelude::*, subclass::prelude::*};
use libadwaita::{self, subclass::prelude::*};

mod imp {
    use super::*;

    #[derive(CompositeTemplate, Default)]
    #[template(file = "resources/data/shortcuts.blp")]
    pub struct ShortcutsDialog {}

    #[glib::object_subclass]
    impl ObjectSubclass for ShortcutsDialog {
        const NAME: &'static str = "ShortcutsDialog";
        type Type = super::ShortcutsDialog;
        type ParentType = libadwaita::Window;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.add_binding_action(
                gtk4::gdk::Key::Escape,
                ModifierType::empty(),
                "window.close",
            );
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for ShortcutsDialog {}
    impl WidgetImpl for ShortcutsDialog {}
    impl WindowImpl for ShortcutsDialog {}
    impl AdwWindowImpl for ShortcutsDialog {}
}

glib::wrapper! {
    pub struct ShortcutsDialog(ObjectSubclass<imp::ShortcutsDialog>)
        @extends gtk4::Widget, gtk4::Window, libadwaita::Window;
}

impl ShortcutsDialog {
    pub fn new(parent: &impl IsA<gtk4::Window>) -> Self {
        let obj: Self = glib::Object::new();
        obj.set_transient_for(Some(parent));
        obj.set_modal(true);
        obj
    }
}
