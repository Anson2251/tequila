use relm4::RelmApp;

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp(None)
        .format_target(false)
        .target(env_logger::Target::Stdout)
        .init();

    log::info!("[tequila] application started");

    let app = RelmApp::new("com.github.anson2251.tequila");
    ui::initialize_custom_resources();
    app.run::<ui::AppModel>(());
}
