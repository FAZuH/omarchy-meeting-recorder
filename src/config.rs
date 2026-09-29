//! Where meetings are saved, which whisper model transcribes and how the
//! transcribing page looks: the `output_dir`, `model` and `plain_display` keys
//! in the config file.

use std::path::{Path, PathBuf};

use gtk::glib;

use crate::models::config_file;

fn default_dir() -> PathBuf {
    glib::home_dir().join("Documents/Meetings")
}

/// The folder new meetings go in, read every time: a change takes effect
/// without a restart.
pub fn output_dir() -> PathBuf {
    resolve(&config_text())
}

/// Whether the transcribing page shows the plain display instead of the
/// animation, which repaints the whole window on every frame. Read once, when
/// the window is built, because the page is stacked then: a change needs a new
/// window. `true` in any casing turns it on; anything else, `false` included,
/// leaves the animation.
pub fn plain_display() -> bool {
    plain_display_in(&config_text())
}

fn plain_display_in(text: &str) -> bool {
    value(text, "plain_display").is_some_and(|value| value.eq_ignore_ascii_case("true"))
}

fn config_text() -> String {
    std::fs::read_to_string(config_file()).unwrap_or_default()
}

/// Remembers `dir` as the folder for new meetings. The rest of the config file
/// is kept: models and actions live in it too.
pub fn set_output_dir(dir: &Path) -> Result<(), String> {
    if !dir.is_absolute() {
        return Err("the folder has to be a full path".into());
    }
    set_key("output_dir", &quoted(&dir.display().to_string()))
}

/// The whisper model to transcribe with, as a name from `models::MODELS`.
pub fn set_model(name: &str) -> Result<(), String> {
    set_key("model", &quoted(name))
}

/// Whether the transcribing page shows the plain display. The page is built
/// once, so this one only takes effect on the next start.
pub fn set_plain_display(plain: bool) -> Result<(), String> {
    set_key("plain_display", if plain { "true" } else { "false" })
}

/// Rewrites one top-level key and nothing else, so the actions a user has in
/// the file come back as they were. A key that is not there yet goes above the
/// first `[section]`.
fn set_key(key: &str, value: &str) -> Result<(), String> {
    let file = config_file();
    let text = existing_text(&file)?;
    let updated = with_key(&text, key, &format!("{key} = {value}"));
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&file, updated).map_err(|e| e.to_string())
}

fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The config file as it is, or nothing when there is none yet. Any other read
/// error is reported: rewriting from an empty read would wipe what is in it.
fn existing_text(file: &Path) -> Result<String, String> {
    match std::fs::read_to_string(file) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}

fn resolve(text: &str) -> PathBuf {
    let home = glib::home_dir();
    value(text, "output_dir")
        .map(|value| {
            if value == "~" {
                home
            } else if let Some(rest) = value.strip_prefix("~/") {
                home.join(rest)
            } else {
                PathBuf::from(value)
            }
        })
        .filter(|path| path.is_absolute())
        .unwrap_or_else(default_dir)
}

fn with_key(text: &str, wanted: &str, line: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let top = lines
        .iter()
        .position(|l| l.trim_start().starts_with('['))
        .unwrap_or(lines.len());
    match lines[..top].iter().position(|l| key(l) == wanted) {
        Some(at) => lines[at] = line.to_owned(),
        None => lines.insert(0, line.to_owned()),
    }
    format!("{}\n", lines.join("\n"))
}

fn key(line: &str) -> &str {
    line.split_once('=').map_or("", |(key, _)| key.trim())
}

fn value(text: &str, wanted: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
        .find_map(|line| {
            let (found, rest) = line.split_once('=')?;
            (found.trim() == wanted).then(|| crate::actions::unquote(rest.trim()))
        })
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_configured_folder_wins() {
        assert_eq!(
            resolve("model = \"small\"\noutput_dir = \"/mnt/Meetings\"\n"),
            PathBuf::from("/mnt/Meetings")
        );
        assert_eq!(
            resolve("output_dir = \"/mnt/my#drive\""),
            PathBuf::from("/mnt/my#drive")
        );
        assert_eq!(
            resolve("output_dir = \"/data/my \\\"meetings\\\"\""),
            PathBuf::from("/data/my \"meetings\"")
        );
    }

    #[test]
    fn a_tilde_is_the_home_directory() {
        let home = glib::home_dir();
        assert_eq!(
            resolve("output_dir = \"~/Notes/Meetings\""),
            home.join("Notes/Meetings")
        );
        assert_eq!(resolve("output_dir = \"~\""), home);
    }

    #[test]
    fn an_unusable_value_falls_back_to_the_default() {
        let default = default_dir();
        assert_eq!(resolve(""), default);
        assert_eq!(resolve("model = \"small\"\n"), default);
        assert_eq!(resolve("output_dir = \"\"\n"), default);
        assert_eq!(resolve("output_dir = \"Meetings\"\n"), default);
        assert_eq!(resolve("output_dir = \"~root/Meetings\"\n"), default);
    }

    #[test]
    fn an_action_field_is_not_the_meetings_folder() {
        let text =
            "[[action]]\nname = \"Send\"\ncommand = \"send.sh\"\noutput_dir = \"/tmp/action\"\n";
        assert_eq!(resolve(text), default_dir());
        let written = with_key(text, "output_dir", "output_dir = \"/mnt/Meetings\"");
        assert!(written.starts_with("output_dir = \"/mnt/Meetings\"\n[[action]]"));
        assert!(written.contains("output_dir = \"/tmp/action\""));
        assert_eq!(written.matches("output_dir").count(), 2);
    }

    #[test]
    fn an_unreadable_config_is_reported_and_a_missing_one_is_not() {
        let dir = std::env::temp_dir().join("omr-config-unreadable");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("config.toml");
        let broken = b"model = \"small\"\nname = \"caf\xe9\"\n";
        std::fs::write(&file, broken).unwrap();
        assert!(existing_text(&file).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), broken);
        assert!(
            existing_text(&dir.join("nothing-here.toml"))
                .unwrap()
                .is_empty()
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_plain_display_is_off_unless_the_config_says_true() {
        assert!(!plain_display_in(""));
        assert!(!plain_display_in("model = \"small\"\n"));
        assert!(!plain_display_in("plain_display = false\n"));
        assert!(!plain_display_in("plain_display = \"\"\n"));
        assert!(plain_display_in("plain_display = true\n"));
        assert!(plain_display_in("plain_display = True\n"));
        assert!(plain_display_in("plain_display = \"TRUE\"\n"));
        assert!(plain_display_in(
            "model = \"small\"\nplain_display = true\n"
        ));
        assert!(!plain_display_in("[[action]]\nplain_display = \"true\"\n"));
    }

    #[test]
    fn setting_the_folder_keeps_the_rest_of_the_file() {
        let file = "model = \"small\"\n\n[[action]]\nname = \"Send\"\ncommand = \"send.sh\"\n";
        let written = with_key(file, "output_dir", "output_dir = \"/mnt/Meetings\"");
        assert!(written.starts_with("output_dir = \"/mnt/Meetings\"\nmodel = \"small\""));
        assert!(written.contains("[[action]]\nname = \"Send\"\ncommand = \"send.sh\"\n"));
        assert!(!written.contains("\n\noutput_dir"));

        let again = with_key(&written, "output_dir", "output_dir = \"/data/Meetings\"");
        assert_eq!(again.matches("output_dir").count(), 1);
        assert!(again.contains("output_dir = \"/data/Meetings\"\nmodel = \"small\""));
    }

    #[test]
    fn setting_a_second_key_leaves_the_first_one_alone() {
        let file = "output_dir = \"/mnt/Meetings\"\n\n[[action]]\nname = \"Send\"\ncommand = \"send.sh\"\n";
        let written = with_key(file, "plain_display", "plain_display = true");
        assert!(written.starts_with("plain_display = true\noutput_dir = \"/mnt/Meetings\""));
        assert!(plain_display_in(&written));
        assert!(written.contains("[[action]]\nname = \"Send\"\ncommand = \"send.sh\"\n"));
        assert_eq!(
            with_key(&written, "output_dir", "output_dir = \"/data\"")
                .matches("output_dir =")
                .count(),
            1
        );
    }

    #[test]
    fn a_key_lands_above_the_first_section_and_not_inside_it() {
        let file = "[general]\nmodel = \"small\"\n";
        let written = with_key(file, "model", "model = \"large-v3\"");
        assert!(written.starts_with("model = \"large-v3\"\n[general]\n"));
        // The same key inside a section is somebody else's to say.
        assert!(written.contains("[general]\nmodel = \"small\"\n"));
    }
}
