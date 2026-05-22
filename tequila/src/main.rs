use relm4::RelmApp;

fn main() {
    let app = RelmApp::new("com.github.anson2251.tequila");
    ui::initialize_custom_resources();
    app.run::<ui::AppModel>(());
}
