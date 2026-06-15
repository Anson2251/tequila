use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Language identifiers supported by the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// Follow system locale (no LC override)
    System,
    /// 中文（简体）
    ZhCN,
    /// English
    En,
}

impl Language {
    /// Returns the BCP-47 locale tag used for this language.
    pub fn locale(&self) -> &'static str {
        match self {
            Language::System => "",
            Language::ZhCN => "zh_CN.UTF-8",
            Language::En => "en_US.UTF-8",
        }
    }

    /// Returns the JSON file name (without path) for this language.
    pub fn file_name(&self) -> &'static str {
        match self {
            Language::System => self.detect_file_name(),
            Language::ZhCN => "zh-CN.json",
            Language::En => "en.json",
        }
    }

    /// When the setting is System, detect locale from the environment.
    /// Returns the file name of the best matching translation.
    fn detect_file_name(&self) -> &'static str {
        for var in &["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(val) = std::env::var(var) {
                let val = val.to_lowercase();
                if val.starts_with("zh_cn") || val.starts_with("zh-cn") || val.starts_with("zh_hans") || val.starts_with("zh") {
                    return "zh-CN.json";
                }
                if val.starts_with("en") {
                    return "en.json";
                }
            }
        }
        "en.json"
    }

    /// Parse from a settings string (e.g. "system", "zh-CN", "en").
    pub fn from_str(s: &str) -> Self {
        match s {
            "zh-CN" => Language::ZhCN,
            "en" => Language::En,
            _ => Language::System,
        }
    }

    /// Serialize to a settings string.
    pub fn to_str(&self) -> &'static str {
        match self {
            Language::System => "system",
            Language::ZhCN => "zh-CN",
            Language::En => "en",
        }
    }
}

static LOCALE: OnceLock<Language> = OnceLock::new();
static TRANSLATIONS: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Initialize the i18n system with the given language preference.
///
/// Loads the translation JSON file for the selected language.
/// Must be called once before any `t!()` macro usage (typically at app start).
pub fn init(language: Language) {
    let _ = LOCALE.set(language);

    let file_name = language.file_name();
    let mut path = lang_dir();
    path.push(file_name);

    let map = match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(e) => {
            log::warn!("[i18n] failed to load language file {:?}: {}", path, e);
            HashMap::new()
        }
    };
    let _ = TRANSLATIONS.set(map);
}

/// Returns the currently active language.
pub fn current_language() -> Language {
    LOCALE.get().copied().unwrap_or(Language::System)
}

/// Returns the currently active language for display (System resolves to detected language).
pub fn display_language() -> Language {
    let lang = current_language();
    match lang {
        Language::System => {
            // Detect from environment
            for var in &["LC_ALL", "LC_MESSAGES", "LANG"] {
                if let Ok(val) = std::env::var(var) {
                    let val = val.to_lowercase();
                    if val.starts_with("zh") {
                        return Language::ZhCN;
                    }
                }
            }
            Language::En
        }
        other => other,
    }
}

/// Look up a translation key and return the translated string.
///
/// Falls back to the key itself if the translation is not found.
pub fn tr(key: &str) -> String {
    TRANSLATIONS
        .get()
        .and_then(|map| map.get(key))
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

/// Look up a translation key and format with named arguments.
///
/// Replaces `{name}` placeholders in the translated string with the provided values.
pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    let mut s = tr(key);
    for (k, v) in args {
        s = s.replace(&format!("{{{}}}", k), v);
    }
    s
}

/// Return the path to the `lang/` directory containing translation files.
///
/// Search order:
///   1. macOS `.app` bundle: `<bundle>/Contents/Resources/lang/`
///   2. Along ancestors of the executable path: `data/lang/`
///   3. Current working directory: `data/lang/`
///   4. Last resort fallback: `data/lang/` (relative path)
fn lang_dir() -> PathBuf {
    // 1. macOS .app bundle: Resources/lang/ (sibling to MacOS/)
    if let Ok(exe_path) = std::env::current_exe() {
        // Check for standard macOS bundle layout:
        //   Tequila.app/Contents/MacOS/tequila
        //   Tequila.app/Contents/Resources/lang/
        if let Some(parent) = exe_path.parent() {
            // parent = .../MacOS
            if let Some(contents) = parent.parent() {
                // contents = .../Contents
                let bundle_resources = contents.join("Resources").join("lang");
                if bundle_resources.is_dir() {
                    return bundle_resources;
                }
            }
        }

        // 2. Walk ancestors looking for data/lang/ relative to exe
        for ancestor in exe_path.ancestors() {
            let candidate = ancestor.join("data").join("lang");
            if candidate.is_dir() {
                return candidate;
            }
        }
    }

    // 3. Fallback: look for data/ relative to the current working directory
    let cwd_candidate = PathBuf::from("data").join("lang");
    if cwd_candidate.is_dir() {
        return cwd_candidate;
    }

    // 4. Last resort
    PathBuf::from("data").join("lang")
}

/// Macro to look up a translation string by key.
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::tr($key)
    };
}

/// Macro to look up and format a translation string.
#[macro_export]
macro_rules! tf {
    ($key:expr, $($k:expr => $v:expr),* $(,)?) => {
        $crate::i18n::tf($key, &[$(($k, $v)),*])
    };
}
