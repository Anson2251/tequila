use relm4::RelmApp;

// Import modules
mod prefix;
mod ui;

use ui::{initialize_custom_icons, AppModel};

fn main() {
    let app = RelmApp::new("com.anson2251.tequila");

    // Initialize custom icons
    initialize_custom_icons();
    
    app.run::<AppModel>(());
}
