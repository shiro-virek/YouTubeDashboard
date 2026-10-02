//! User preferences, stored in a plain text file next to the database so the
//! whole thing stays portable.
//!
//! The only setting for now is which browser opens a channel link.

use std::path::Path;

/// Well known browsers we look for in `PATH`, in preference order.
pub const KNOWN_BROWSERS: &[(&str, &str)] = &[
    ("Firefox", "firefox"),
    ("Firefox ESR", "firefox-esr"),
    ("Google Chrome", "google-chrome"),
    ("Google Chrome Beta", "google-chrome-beta"),
    ("Chromium", "chromium"),
    ("Chromium (snap)", "chromium-browser"),
    ("Brave", "brave-browser"),
    ("Microsoft Edge", "microsoft-edge"),
    ("Vivaldi", "vivaldi"),
    ("Opera", "opera"),
];

/// How a channel URL should be opened.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Browser {
    /// Let the desktop environment decide (the `xdg-open` / GIO default).
    #[default]
    System,
    /// Run this command with the URL appended as the last argument.
    Command(String),
}

impl Browser {
    fn parse(value: &str) -> Self {
        let value = value.trim();
        if value.is_empty()
            || value.eq_ignore_ascii_case("system")
            || value.eq_ignore_ascii_case("default")
        {
            Self::System
        } else {
            Self::Command(value.to_string())
        }
    }

    /// Short human label for the preferences dialog.
    pub fn label(&self) -> String {
        match self {
            Self::System => "Predeterminado del sistema".to_string(),
            Self::Command(command) => command.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub browser: Browser,
}



impl Config {
    /// Reads the config, silently falling back to the defaults when the file
    /// is missing or damaged. A broken config must never stop the app.
    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        Self::from_str(&text)
    }

    pub fn from_str(text: &str) -> Self {
        let mut config = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            if key.trim().eq_ignore_ascii_case("browser") {
                config.browser = Browser::parse(value);
            }
        }
        config
    }

    pub fn to_text(&self) -> String {
        let value = match &self.browser {
            Browser::System => "system".to_string(),
            Browser::Command(command) => command.clone(),
        };
        format!(
            "# ytdash preferences\n\
             #\n\
             # browser = system        use the desktop default handler\n\
             # browser = firefox       run this command with the URL appended\n\
             # browser = firefox -P    arguments are supported too\n\
             browser = {value}\n"
        )
    }

    /// Best effort save: a read-only stick should not break the app.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, self.to_text())
    }
}

/// The known browsers whose executable is actually reachable from `PATH`.
pub fn detect_browsers() -> Vec<&'static (&'static str, &'static str)> {
    KNOWN_BROWSERS
        .iter()
        .filter(|(_, program)| in_path(program))
        .collect()
}

fn in_path(program: &str) -> bool {
    if program.contains('/') {
        return std::path::Path::new(program).is_file();
    }
    let Ok(path) = std::env::var("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(program).is_file())
}

/// Splits a command line into program plus arguments, honouring single and
/// double quotes and backslash escapes: `firefox -P "My Profile"`.
pub fn split_command(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            started = true;
            continue;
        }
        match quote {
            Some(open) => {
                if ch == open {
                    quote = None;
                } else {
                    current.push(ch);
                }
            }
            None => match ch {
                '\\' => {
                    escaped = true;
                    started = true;
                }
                '"' | '\'' => {
                    quote = Some(ch);
                    started = true;
                }
                c if c.is_whitespace() => {
                    if started {
                        parts.push(std::mem::take(&mut current));
                        started = false;
                    }
                }
                c => {
                    current.push(c);
                    started = true;
                }
            },
        }
    }

    if started {
        parts.push(current);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_the_system_handler() {
        assert_eq!(
            Config::load(Path::new("/nonexistent/ytdash.conf")),
            Config::default()
        );
        assert_eq!(Browser::default(), Browser::System);
    }

    #[test]
    fn round_trips_through_text() {
        let config = Config {
            browser: Browser::Command("firefox -P \"Mis perfiles\"".to_string()),
        };
        assert_eq!(Config::from_str(&config.to_text()), config);
    }

    #[test]
    fn system_is_spelled_in_several_ways() {
        for value in ["", "  ", "system", "System", "default"] {
            assert_eq!(
                Browser::parse(value),
                Browser::System,
                "{value:?} should be system"
            );
        }
    }

    #[test]
    fn ignores_comments_unknown_keys_and_junk() {
        let config = Config::from_str(
            "# ytdash preferences\n\
             ; another comment style\n\
             browser = firefox\n\
             unknown = whatever\n\
             not a key value line\n",
        );
        assert_eq!(config.browser, Browser::Command("firefox".to_string()));
    }

    #[test]
    fn a_damaged_value_falls_back_to_the_system() {
        let config = Config::from_str("browser =");
        assert_eq!(config.browser, Browser::System);
    }

    #[test]
    fn splits_commands_like_a_shell() {
        assert_eq!(split_command("firefox"), vec!["firefox"]);
        assert_eq!(split_command("  firefox  "), vec!["firefox"]);
        assert_eq!(
            split_command("firefox -P \"Mis perfiles\""),
            vec!["firefox", "-P", "Mis perfiles"]
        );
        assert_eq!(
            split_command("flatpak 'org.mozilla.firefox' --new-window"),
            vec!["flatpak", "org.mozilla.firefox", "--new-window"]
        );
        assert_eq!(split_command(r"firefox a\ b"), vec!["firefox", "a b"]);
        assert!(split_command("   ").is_empty());
        assert_eq!(split_command("''"), vec![""]);
    }

    #[test]
    fn every_known_browser_is_a_plain_program_name() {
        for (_, program) in KNOWN_BROWSERS {
            assert!(
                !program.contains('/'),
                "{program} should be looked up in PATH"
            );
            assert!(!program.contains(char::is_whitespace));
        }
    }
}
