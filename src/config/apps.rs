//! Installed apps for the keybind editor's app picker: desktop entries
//! from the XDG data dirs plus loose AppImages in the usual folders, each
//! as the shell command a `spawn:` bind runs.

use std::path::{Path, PathBuf};

use super::discovery;

/// One launchable app: its display name and the command that starts it.
#[derive(Debug, Clone, PartialEq)]
pub struct App {
    pub name: String,
    pub command: String,
}

/// Apps this session can launch, sorted by name.
pub fn installed_apps(env: &discovery::Env) -> Vec<App> {
    let home = env.home.as_ref().map(PathBuf::from);
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|home| home.join(".local/share")));
    let data_dirs = env
        .xdg_data_dirs
        .clone()
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let mut dirs: Vec<PathBuf> = data_home.into_iter().collect();
    dirs.extend(std::env::split_paths(&data_dirs));
    let dirs: Vec<PathBuf> = dirs.iter().map(|dir| dir.join("applications")).collect();
    // Where installed AppImages usually live: AppImageLauncher, Gear
    // Lever, and a PATH folder. Downloads is left out: copies there are
    // usually installers already set up elsewhere.
    let appimages: Vec<PathBuf> = home
        .map(|home| {
            ["Applications", "AppImages", ".local/bin"]
                .iter()
                .map(|dir| home.join(dir))
                .collect()
        })
        .unwrap_or_default();
    scan(&dirs, &appimages)
}

/// Desktop entries from `dirs` (earlier dirs win per file name), then
/// AppImages in the `appimages` folders that aren't already listed: not
/// launched by an entry, and not a copy of a listed app (its name inside
/// the file name, e.g. "Tabby" in `tabby-1.0.236-linux-x64.AppImage`).
fn scan(dirs: &[PathBuf], appimages: &[PathBuf]) -> Vec<App> {
    let mut seen: Vec<std::ffi::OsString> = Vec::new();
    let mut apps: Vec<App> = Vec::new();
    for dir in dirs {
        for path in sorted_files(dir) {
            let Some(file_name) = path.file_name().map(|name| name.to_owned()) else {
                continue;
            };
            if path.extension().is_none_or(|ext| ext != "desktop") || seen.contains(&file_name) {
                continue;
            }
            seen.push(file_name);
            if let Some(app) = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| parse_entry(&text))
            {
                apps.push(app);
            }
        }
    }
    for dir in appimages {
        for path in sorted_files(dir) {
            let is_appimage = path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("appimage"));
            let path_text = path.display().to_string();
            let name = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            let lowered = name.to_lowercase();
            let listed = apps.iter().any(|app| {
                app.command.contains(&path_text)
                    || (app.name.len() >= 3 && lowered.contains(&app.name.to_lowercase()))
            });
            if !is_appimage || listed {
                continue;
            }
            apps.push(App {
                name,
                command: shell_quote(&path_text),
            });
        }
    }
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps
}

fn sorted_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    files.sort();
    files
}

/// Name and command from a desktop entry's `[Desktop Entry]` group; None
/// for hidden entries and anything that isn't a launchable application.
fn parse_entry(text: &str) -> Option<App> {
    let mut in_entry = false;
    let (mut name, mut exec) = (None, None);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        let Some((key, value)) = line.split_once('=').filter(|_| in_entry) else {
            continue;
        };
        match (key.trim(), value.trim()) {
            ("Type", kind) if kind != "Application" => return None,
            ("NoDisplay" | "Hidden", "true") => return None,
            ("Name", value) => name = Some(value.to_owned()),
            ("Exec", value) => exec = Some(value.to_owned()),
            _ => {}
        }
    }
    let command = strip_field_codes(&exec?);
    (!command.is_empty()).then(|| App {
        name: name.unwrap_or_else(|| command.clone()),
        command,
    })
}

/// Drop the desktop-entry field codes (`%U`, `%f`, …), which only mean
/// something to a launcher that passes files; `%%` is a literal `%`.
fn strip_field_codes(exec: &str) -> String {
    let mut out = String::new();
    let mut chars = exec.chars();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        if chars.next() == Some('%') {
            out.push('%');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Quote `text` for the shell when it holds anything but plain path characters.
fn shell_quote(text: &str) -> String {
    let plain = text
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || "/._-+".contains(ch));
    if plain && !text.is_empty() {
        text.to_owned()
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}

/// Whether the program a `spawn:` command starts can be found: an
/// existing path, or a bare name on this session's PATH. Commands that
/// open with a variable assignment or shell syntax are given the benefit
/// of the doubt.
pub fn command_found(command: &str) -> bool {
    let command = command.trim();
    let program = match command.chars().next() {
        Some(quote @ ('"' | '\'')) => command[1..].split(quote).next().unwrap_or_default(),
        _ => command.split_whitespace().next().unwrap_or_default(),
    };
    if program.is_empty() || program.contains(['=', '$', '(', '`']) {
        return true;
    }
    if let Some(rest) = program.strip_prefix("~/") {
        return std::env::var_os("HOME").is_some_and(|home| Path::new(&home).join(rest).exists());
    }
    if program.contains('/') {
        return Path::new(program).exists();
    }
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_field_codes() {
        assert_eq!(
            strip_field_codes(r#""/opt/Tabby.AppImage" --no-sandbox %U"#),
            r#""/opt/Tabby.AppImage" --no-sandbox"#
        );
        assert_eq!(strip_field_codes("printf 100%% %f"), "printf 100%");
    }

    #[test]
    fn skips_hidden_and_non_apps() {
        let app = "[Desktop Entry]\nType=Application\nName=Kitty\nExec=kitty\n";
        assert_eq!(
            parse_entry(app),
            Some(App {
                name: "Kitty".to_owned(),
                command: "kitty".to_owned(),
            })
        );
        assert_eq!(parse_entry(&format!("{app}NoDisplay=true\n")), None);
        assert_eq!(parse_entry("[Desktop Entry]\nType=Link\nExec=x\n"), None);
        // Keys in action groups don't leak into the entry.
        let actions = format!("{app}[Desktop Action new]\nName=New Window\nExec=kitty -1\n");
        assert_eq!(
            parse_entry(&actions).map(|app| app.name),
            Some("Kitty".to_owned())
        );
    }

    #[test]
    fn user_entries_win_and_loose_appimages_are_listed() {
        let root = std::env::temp_dir().join(format!("umbriel-apps-{}", std::process::id()));
        let (user, system, images) = (root.join("user"), root.join("system"), root.join("images"));
        for dir in [&user, &system, &images] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let entry = |name: &str, exec: &str| {
            format!("[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\n")
        };
        std::fs::write(
            user.join("kitty.desktop"),
            entry("Kitty (mine)", "kitty -1"),
        )
        .unwrap();
        std::fs::write(system.join("kitty.desktop"), entry("Kitty", "kitty")).unwrap();
        let listed = images.join("Listed.AppImage");
        std::fs::write(
            system.join("listed.desktop"),
            entry("Listed", &listed.display().to_string()),
        )
        .unwrap();
        std::fs::write(&listed, "").unwrap();
        std::fs::write(images.join("Loose.appimage"), "").unwrap();
        // A second copy of a listed app is left out.
        std::fs::write(images.join("listed-2.0-x86_64.AppImage"), "").unwrap();

        let apps = scan(&[user, system], &[images]);
        let names: Vec<&str> = apps.iter().map(|app| app.name.as_str()).collect();
        assert_eq!(names, ["Kitty (mine)", "Listed", "Loose"]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn quotes_only_when_needed() {
        assert_eq!(shell_quote("/usr/bin/kitty"), "/usr/bin/kitty");
        assert_eq!(
            shell_quote("/home/me/My App.AppImage"),
            "'/home/me/My App.AppImage'"
        );
    }

    #[test]
    fn finds_commands_on_path_and_by_path() {
        assert!(command_found("sh -c true"));
        assert!(command_found("'/bin/sh' -c true"));
        assert!(!command_found("Tabby"));
        assert!(!command_found("/nonexistent/app --flag"));
        assert!(command_found("FOO=1 app"));
    }
}
