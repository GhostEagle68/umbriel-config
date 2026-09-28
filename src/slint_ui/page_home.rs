//! The Home page: the start screen's lists (unsaved changes, newly
//! synced settings) and one jump tile per sidebar group.

use super::common::{chain_path_sets, doc_at, entry_home};
use super::rows::{strip_decor, swatch_for};
use super::sections::{page_id_for_section, page_meta, page_new_counts};
use super::*;

/// Unsaved changes listed on Home; the rest sit behind "and N more".
const HOME_CHANGES: usize = 5;

/// The page a changed key is edited on; empty when no page shows it
/// (the Home link then opens the save popup instead).
fn page_for_key(shell: &Shell, key: &str) -> String {
    if let Some(entry) = shell.schema.iter().find(|entry| entry.dotted() == key) {
        return page_id_for_section(&entry.section);
    }
    if key.starts_with("keybinds.") {
        return "keybinds".to_owned();
    }
    if key.starts_with("output.") {
        return catalog::OUTPUTS_ID.to_owned();
    }
    if key == "environment" || key.starts_with("environment.") {
        return "environment".to_owned();
    }
    ["window-rules", "layer-rules", "security-contexts"]
        .into_iter()
        .find(|page| super::page_rules::rule_family(page) == Some(key))
        .unwrap_or_default()
        .to_owned()
}

fn changes(shell: &Shell) -> Vec<HomeLink> {
    let entries = super::save::build_save_entries(shell);
    let mut links: Vec<HomeLink> = entries
        .iter()
        .take(HOME_CHANGES)
        .map(|entry| HomeLink {
            label: entry.label.clone(),
            detail: entry.value.clone(),
            page_id: page_for_key(shell, &entry.key).into(),
        })
        .collect();
    if entries.len() > HOME_CHANGES {
        links.push(HomeLink {
            label: format!("and {} more", entries.len() - HOME_CHANGES).into(),
            detail: "review all".into(),
            page_id: SharedString::default(),
        });
    }
    links
}

fn new_settings(shell: &Shell) -> Vec<HomeLink> {
    page_new_counts(shell)
        .into_iter()
        .map(|(page_id, count)| HomeLink {
            label: page_meta(&page_id).0.into(),
            detail: format!(
                "{count} new setting{} from umbriel",
                if count == 1 { "" } else { "s" }
            )
            .into(),
            page_id: page_id.into(),
        })
        .collect()
}

/// A key's value as umbriel will read it: the file that sets it last,
/// else the schema default.
fn effective(shell: &Shell, sets: &[BTreeSet<String>], dotted: &str) -> Option<String> {
    let parts: Vec<&str> = dotted.split('.').collect();
    if let Some(home) = entry_home(sets, dotted) {
        let raw = doc_at(shell, home).get_raw(&parts)?;
        return Some(strip_decor(&raw).trim_matches('"').to_owned());
    }
    let entry = shell.schema.iter().find(|entry| entry.dotted() == dotted)?;
    Some(match entry.default.as_ref()? {
        schema::Value::Bool(value) => value.to_string(),
        schema::Value::Integer(value) => value.to_string(),
        schema::Value::Float(value) => value.to_string(),
        schema::Value::Text(value) => value.clone(),
    })
}

/// Tiled window rectangles for `count` windows in a layout mode, as
/// fractions of the monitor: a sketch of how umbriel arranges them.
fn tile(mode: &str, count: usize, master_fraction: f32, master_right: bool) -> Vec<[f32; 4]> {
    match mode {
        // Columns side by side; the strip runs past the right edge.
        "scrolling" if count == 1 => vec![[0.2, 0.0, 0.6, 1.0]],
        "scrolling" => (0..count)
            .map(|i| [i as f32 * 0.5, 0.0, 0.5, 1.0])
            .collect(),
        "master" if count > 1 => {
            let stack = (count - 1) as f32;
            let (master_x, stack_x) = if master_right {
                (1.0 - master_fraction, 0.0)
            } else {
                (0.0, master_fraction)
            };
            let mut rects = vec![[master_x, 0.0, master_fraction, 1.0]];
            rects.extend((0..count - 1).map(|i| {
                [
                    stack_x,
                    i as f32 / stack,
                    1.0 - master_fraction,
                    1.0 / stack,
                ]
            }));
            rects
        }
        // Dwindle: each new window halves the last one, alternating
        // between side by side and stacked.
        _ => {
            let mut rects = Vec::new();
            let mut area = [0.0, 0.0, 1.0, 1.0];
            for i in 0..count {
                if i + 1 == count {
                    rects.push(area);
                    break;
                }
                let [x, y, w, h] = area;
                if i % 2 == 0 {
                    rects.push([x, y, w / 2.0, h]);
                    area = [x + w / 2.0, y, w / 2.0, h];
                } else {
                    rects.push([x, y, w, h / 2.0]);
                    area = [x, y + h / 2.0, w, h / 2.0];
                }
            }
            rects
        }
    }
}

/// Noctalia's wallpapers (per monitor, "" for its default), from the
/// settings its wallpaper picker writes. A `color:#RRGGBB` wallpaper
/// becomes a one-pixel image of that color. Empty without Noctalia.
pub(super) fn noctalia_wallpapers(env: &discovery::Env) -> BTreeMap<String, slint::Image> {
    let Some(state) = discovery::state_dir(env).parent().map(Path::to_path_buf) else {
        return BTreeMap::new();
    };
    let Ok(text) = std::fs::read_to_string(state.join("noctalia/settings.toml")) else {
        return BTreeMap::new();
    };
    let Ok(doc) = text.parse::<toml_edit::DocumentMut>() else {
        return BTreeMap::new();
    };
    let path_of = |table: &toml_edit::Item| table.get("path")?.as_str().map(str::to_owned);
    let mut paths: Vec<(String, String)> = Vec::new();
    if let Some(path) = doc
        .get("wallpaper")
        .and_then(|w| w.get("default"))
        .and_then(path_of)
    {
        paths.push((String::new(), path));
    }
    if let Some(monitors) = doc
        .get("wallpaper")
        .and_then(|w| w.get("monitors"))
        .and_then(toml_edit::Item::as_table_like)
    {
        for (name, table) in monitors.iter() {
            if let Some(path) = path_of(table) {
                paths.push((name.to_owned(), path));
            }
        }
    }
    let mut loaded: BTreeMap<String, slint::Image> = BTreeMap::new();
    let mut decoded: BTreeMap<String, slint::Image> = BTreeMap::new();
    for (name, path) in paths {
        let image = match path.strip_prefix("color:") {
            Some(hex) => {
                let slint::Brush::SolidColor(color) = swatch_for(hex) else {
                    continue;
                };
                let mut pixel = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(1, 1);
                pixel.make_mut_slice()[0] =
                    slint::Rgba8Pixel::new(color.red(), color.green(), color.blue(), 255);
                slint::Image::from_rgba8(pixel)
            }
            // The same file on every monitor decodes once.
            None => match decoded.get(&path) {
                Some(image) => image.clone(),
                None => match slint::Image::load_from_path(Path::new(&path)) {
                    Ok(image) => {
                        decoded.insert(path.clone(), image.clone());
                        image
                    }
                    Err(_) => continue,
                },
            },
        };
        loaded.insert(name, image);
    }
    loaded
}

/// The monitors to draw, in desktop pixels: live ones when umbriel
/// answers, else the configured outputs, else one 1080p screen.
fn monitors(shell: &Shell) -> Vec<(String, [f32; 4])> {
    let live: Vec<(String, [f32; 4])> = shell
        .guide_monitors
        .iter()
        .filter(|monitor| monitor.enabled)
        .filter_map(|monitor| {
            let mode = monitor.modes.get(monitor.current?)?;
            let scale = if monitor.scale > 0.0 {
                monitor.scale
            } else {
                1.0
            } as f32;
            Some((
                monitor.name.clone(),
                [
                    monitor.position.0 as f32,
                    monitor.position.1 as f32,
                    mode.width as f32 / scale,
                    mode.height as f32 / scale,
                ],
            ))
        })
        .collect();
    if !live.is_empty() {
        return live;
    }
    let mut configured: Vec<(String, [f32; 4])> = Vec::new();
    for doc in super::common::chain_docs(shell) {
        for name in outputs::configured(doc) {
            if configured.iter().any(|(known, _)| *known == name) {
                continue;
            }
            let (w, h) = doc
                .get_string(&["output", &name, "mode"])
                .and_then(|mode| {
                    let (w, rest) = mode.split_once('x')?;
                    let h = rest.split('@').next()?;
                    Some((w.parse().ok()?, h.parse().ok()?))
                })
                .unwrap_or((1920.0, 1080.0));
            let x_default = configured
                .iter()
                .map(|(_, r)| r[0] + r[2])
                .fold(0.0, f32::max);
            let (x, y) = match doc.get_integers(&["output", &name, "position"]).as_deref() {
                Some([x, y]) => (*x as f32, *y as f32),
                _ => (x_default, 0.0),
            };
            configured.push((name, [x, y, w, h]));
        }
    }
    if configured.is_empty() {
        configured.push(("Monitor".to_owned(), [0.0, 0.0, 1920.0, 1080.0]));
    }
    configured
}

fn desktop(shell: &Shell) -> (Vec<DeskMonitor>, DeskStyle) {
    let sets = chain_path_sets(shell);
    let value = |key: &str| effective(shell, &sets, key).unwrap_or_default();
    let number = |key: &str| value(key).parse::<f32>().unwrap_or(0.0);
    let color = |key: &str| match swatch_for(&value(key)) {
        slint::Brush::SolidColor(color) => color,
        _ => slint::Color::default(),
    };
    let mode = value("layout.mode");
    let master_fraction = value("layout.master.default_width_fraction")
        .parse::<f32>()
        .unwrap_or(0.55)
        .clamp(0.1, 0.9);
    let master_right = value("layout.master.position") == "right";

    let mut screens = monitors(shell);
    // Left to right, top to bottom: the busy screen is the leftmost.
    screens.sort_by(|(_, a), (_, b)| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    let min_x = screens.iter().map(|(_, r)| r[0]).fold(f32::MAX, f32::min);
    let min_y = screens.iter().map(|(_, r)| r[1]).fold(f32::MAX, f32::min);
    let max_x = screens.iter().map(|(_, r)| r[0] + r[2]).fold(0.0, f32::max);
    let max_y = screens.iter().map(|(_, r)| r[1] + r[3]).fold(0.0, f32::max);
    // A busy first screen, calmer ones after it; the focus sits on the
    // first screen's first window.
    let monitors = screens
        .iter()
        .enumerate()
        .map(|(index, (name, [x, y, w, h]))| {
            // A third screen stays empty so the wallpaper shows.
            let count = [3, 2, 0][index.min(2)];
            let windows: Vec<DeskWindow> = tile(&mode, count, master_fraction, master_right)
                .into_iter()
                .enumerate()
                .map(|(i, [wx, wy, ww, wh])| DeskWindow {
                    x: wx,
                    y: wy,
                    w: ww,
                    h: wh,
                    focused: index == 0 && i == 0,
                })
                .collect();
            let wallpaper = shell
                .wallpapers
                .get(name)
                .or_else(|| shell.wallpapers.get(""));
            DeskMonitor {
                has_wallpaper: wallpaper.is_some(),
                wallpaper: wallpaper.cloned().unwrap_or_default(),
                name: name.clone().into(),
                x: x - min_x,
                y: y - min_y,
                w: *w,
                h: *h,
                windows: Rc::new(VecModel::from(windows)).into(),
            }
        })
        .collect();
    let style = DeskStyle {
        gap: number("layout.gap"),
        border: number("appearance.border_width"),
        radius: number("appearance.corner_radius"),
        focused: color("colors.border.focused"),
        unfocused: color("colors.border.unfocused"),
        background: color("colors.background"),
        accent: color("colors.accent_primary"),
        accent_2: color("colors.accent_secondary"),
        shadow: value("appearance.shadow.enabled") == "true",
        strut_top: number("layout.struts.top"),
        strut_bottom: number("layout.struts.bottom"),
        strut_left: number("layout.struts.left"),
        strut_right: number("layout.struts.right"),
        extent_w: max_x - min_x,
        extent_h: max_y - min_y,
        mode: mode.into(),
    };
    (monitors, style)
}

pub(super) fn rebuild_home(app: &AppWindow, shell: &Shell) {
    let (monitors, style) = desktop(shell);
    app.set_home_monitors(Rc::new(VecModel::from(monitors)).into());
    app.set_home_desk_style(style);
    app.set_home_changes(Rc::new(VecModel::from(changes(shell))).into());
    app.set_home_new_settings(Rc::new(VecModel::from(new_settings(shell))).into());
}

pub(super) fn install_home(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    let weak = app.as_weak();
    let shell = Rc::clone(shell);
    app.on_home_requested(move || {
        let Some(app) = weak.upgrade() else { return };
        app.set_current_section(SharedString::default());
        app.set_page(Page::Home);
        let nav = super::sections::section_nav(&shell.borrow(), "");
        app.set_sections(Rc::new(VecModel::from(nav)).into());
        // The drawing wants the real monitors; one scan per run.
        if shell.borrow().guide_monitors.is_empty() {
            super::page_outputs::scan_outputs(&mut shell.borrow_mut());
        }
        rebuild_home(&app, &shell.borrow());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_links_lead_to_the_page_that_edits_the_key() {
        let dir = std::env::temp_dir().join(format!("umbriel-home-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let main_path = dir.join("config.toml");
        std::fs::write(&main_path, "").unwrap();
        let shell = Shell::load(&main_path, &discovery::Env::from_process());
        assert_eq!(page_for_key(&shell, "keybinds.Mod+T"), "keybinds");
        assert_eq!(
            page_for_key(&shell, "output.DP-1.mode"),
            catalog::OUTPUTS_ID
        );
        assert_eq!(page_for_key(&shell, "window_rule"), "window-rules");
        assert_eq!(page_for_key(&shell, "environment.FOO"), "environment");
        assert_eq!(page_for_key(&shell, "nothing_here"), "");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn layouts_tile_the_monitor_like_umbriel() {
        // Dwindle halves the last window, alternating direction.
        assert_eq!(
            tile("dwindle", 3, 0.55, false),
            vec![
                [0.0, 0.0, 0.5, 1.0],
                [0.5, 0.0, 0.5, 0.5],
                [0.5, 0.5, 0.5, 0.5]
            ]
        );
        // Master: one big window, the rest stacked beside it.
        let master = tile("master", 3, 0.75, true);
        assert_eq!(master[0], [0.25, 0.0, 0.75, 1.0]);
        assert_eq!(master[2], [0.0, 0.5, 0.25, 0.5]);
        // Scrolling columns run off the right edge.
        assert_eq!(tile("scrolling", 3, 0.55, false)[2], [1.0, 0.0, 0.5, 1.0]);
    }
}
