use gtk::gdk;
use gtk4::gio;
use relm4::gtk;

pub fn initialize_custom_resources() {
    gio::resources_register_include!("icons.gresource").unwrap();
    gio::resources_register_include!("css.gresource").unwrap();

    let display = gdk::Display::default().unwrap();
    let theme = gtk::IconTheme::for_display(&display);
    theme.add_resource_path("/com/anson2251/tequila/icons");

    let provider = gtk::CssProvider::new();
    provider.load_from_resource("/com/anson2251/tequila/css/style.css");
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
