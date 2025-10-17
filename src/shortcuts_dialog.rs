pub fn create_shortcuts_dialog() -> libadwaita::ShortcutsDialog {
    let dialog = libadwaita::ShortcutsDialog::new();

    let general_section = libadwaita::ShortcutsSection::new(Some("General"));

    general_section.add(libadwaita::ShortcutsItem::from_action(
        "Open File",
        "win.open-file",
    ));
    general_section.add(libadwaita::ShortcutsItem::from_action(
        "Toggle Fullscreen",
        "win.toggle-fullscreen",
    ));
    general_section.add(libadwaita::ShortcutsItem::from_action(
        "View Shortcuts",
        "win.keyboard-shortcuts",
    ));
    general_section.add(libadwaita::ShortcutsItem::from_action("Quit", "app.quit"));

    dialog.add(general_section);

    let viewer_section = libadwaita::ShortcutsSection::new(Some("Viewer"));
    viewer_section.add(libadwaita::ShortcutsItem::new("Rotate Up", "Up k"));
    viewer_section.add(libadwaita::ShortcutsItem::new("Rotate Down", "Down j"));
    viewer_section.add(libadwaita::ShortcutsItem::new("Rotate Right", "Right l"));
    viewer_section.add(libadwaita::ShortcutsItem::new("Rotate Left", "Left h"));
    viewer_section.add(libadwaita::ShortcutsItem::new("Toggle Autorotate", "space"));
    viewer_section.add(libadwaita::ShortcutsItem::new("Zoom In", "plus Page_Up i"));
    viewer_section.add(libadwaita::ShortcutsItem::new(
        "Zoom Out",
        "minus Page_Down o",
    ));

    dialog.add(viewer_section);

    dialog
}
