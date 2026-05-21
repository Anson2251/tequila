fn main() {
    glib_build_tools::compile_resources(
        &["assets"],
        "assets/icons.gresource.xml",
        "icons.gresource",
    );
    glib_build_tools::compile_resources(
        &["assets"],
        "assets/css.gresource.xml",
        "css.gresource",
    );
}
