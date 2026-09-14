use std::{fs, path::Path};

use gtk4::gio;

const RESOURCE_PREFIX: &str = "/com/anson2251/tequila/fonts";
const FONT_FILES: [&str; 3] = [
    "CascadiaMono-VariableFont_wght.ttf",
    "CascadiaMono-Italic-VariableFont_wght.ttf",
    "OFL.txt",
];

pub fn prepare_app_fonts() {
    gio::resources_register_include!("fonts.gresource").unwrap_or(());
    let base = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("tequila");
    let fonts_dir = base.join("fonts");
    let cache_dir = base.join("fontconfig-cache");
    if extract_fonts(&fonts_dir).is_err() {
        return;
    }
    let fonts_conf = base.join("fonts.conf");
    if fs::write(&fonts_conf, fonts_conf_contents(&fonts_dir, &cache_dir)).is_err() {
        return;
    }
    unsafe { std::env::set_var("FONTCONFIG_FILE", &fonts_conf) };
}

fn extract_fonts(fonts_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(fonts_dir).map_err(|e| e.to_string())?;
    for name in FONT_FILES {
        let target = fonts_dir.join(name);
        let data = gio::resources_lookup_data(
            &format!("{RESOURCE_PREFIX}/{name}"),
            gio::ResourceLookupFlags::NONE,
        )
        .map_err(|e| e.to_string())?;
        if target.exists()
            && fs::metadata(&target)
                .map(|m| m.len() == data.len() as u64)
                .unwrap_or(false)
        {
            continue;
        }
        fs::write(&target, data.as_ref()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn fonts_conf_contents(fonts_dir: &Path, cache_dir: &Path) -> String {
    let mut config = String::from(
        r#"<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<fontconfig>
"#,
    );
    config.push_str(&format!("  <dir>{}</dir>\n", xml_text(fonts_dir)));
    config.push_str(&format!("  <cachedir>{}</cachedir>\n", xml_text(cache_dir)));
    if cfg!(unix) {
        for system_config in [
            "/etc/fonts/fonts.conf",
            "/opt/homebrew/etc/fonts/fonts.conf",
            "/usr/local/etc/fonts/fonts.conf",
        ] {
            config.push_str(&format!(
                "  <include ignore_missing=\"yes\">{system_config}</include>\n"
            ));
        }
    }
    if cfg!(target_os = "macos") {
        config.push_str("  <dir>/System/Library/Fonts</dir>\n");
        config.push_str("  <dir>/Library/Fonts</dir>\n");
    }
    if cfg!(windows) {
        config.push_str("  <dir>WINDOWSFONTDIR</dir>\n");
        config.push_str("  <dir>WINDOWSUSERFONTDIR</dir>\n");
    }
    config.push_str("</fontconfig>\n");
    config
}

fn xml_text(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn fonts_conf_escapes_xml() {
        let contents = fonts_conf_contents(Path::new("/tmp/a&b<c>"), Path::new("/tmp/c"));
        assert!(contents.contains("<dir>/tmp/a&amp;b&lt;c&gt;</dir>"));
    }
}
