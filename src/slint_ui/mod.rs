//! Slint shell for umbriel-config; the migration plan lives in
//! `.design/ui-plan.md`. Phase 0: window chrome, sidebar navigation, and a
//! read-only status projection of the loaded config. The UI-agnostic library
//! does all real work — this module only presents it and forwards intents.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;

use slint::{
    CloseRequestResponse, ComponentHandle, LogicalSize, Model, SharedString, VecModel, WindowSize,
};
use umbriel_config::config::{
    discovery, document::ConfigDocument, includes, schema, settings as app_settings,
};

slint::include_modules!();

/// Shell state, re-homing the egui `App` fields with the same semantics.
/// Fields unused in Phase 0 become load-bearing in Phases 1–3.
struct Shell {
    path: PathBuf,
    doc: ConfigDocument,
    healthy: bool,
    load_error: Option<String>,
    schema: Vec<schema::Entry>,
    includes: includes::IncludeChain,
}

impl Shell {
    fn load(path: &Path, env: &discovery::Env) -> Self {
        let (doc, healthy, load_error) = match ConfigDocument::load(path) {
            Ok(doc) => (doc, true, None),
            Err(err) => {
                // A missing file is a fresh start: saving creates it. Anything
                // else (a broken file) keeps saving disabled so it is never
                // overwritten from here.
                let healthy = err.is_not_found();
                let load_error = (!healthy).then_some(err.to_string());
                (
                    ConfigDocument::from_str("").expect("empty TOML parses"),
                    healthy,
                    load_error,
                )
            }
        };
        let schema = discovery::packaged_default(env)
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|text| schema::assemble(&text))
            .unwrap_or_default();
        let includes = includes::load_chain(&doc, path);
        Shell {
            path: path.to_path_buf(),
            doc,
            healthy,
            load_error,
            schema,
            includes,
        }
    }

    fn any_modified(&self) -> bool {
        self.doc.is_modified() || self.includes.docs.iter().any(|inc| inc.doc.is_modified())
    }
}

pub fn run(path: PathBuf) -> anyhow::Result<()> {
    let env = discovery::Env::from_process();
    let settings = app_settings::load(&env);
    let shell = Rc::new(RefCell::new(Shell::load(&path, &env)));

    let app = AppWindow::new().map_err(|err| anyhow::anyhow!("window creation failed: {err}"))?;

    {
        let shell = shell.borrow();
        app.set_config_path(shell.path.display().to_string().into());
        app.set_include_count(shell.includes.docs.len() as i32);
        app.set_include_note(shell.includes.notes.join("; ").into());
    }
    app.set_dirty(false);

    let sections = section_names(&shell.borrow().schema);
    if let Some(first) = sections.first() {
        app.set_current_section(first.clone());
        app.set_page_title(first.clone());
        refill_section(&app, &shell.borrow(), first);
    }
    app.set_sections(model(sections));
    app.set_files(model(file_labels(&shell.borrow())));
    app.set_status(status_line(&shell.borrow()));

    app.window().set_size(WindowSize::Logical(LogicalSize::new(
        settings.window_width as f32,
        settings.window_height as f32,
    )));

    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_section_selected(move |name| {
            let Some(app) = weak.upgrade() else { return };
            let shell = shell.borrow();
            app.set_current_section(name.clone());
            app.set_page_title(name.clone());
            app.set_page(Page::Section);
            refill_section(&app, &shell, &name);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_open_file(move |index| {
            let Some(app) = weak.upgrade() else { return };
            let shell = shell.borrow();
            app.set_page(Page::File);
            app.set_current_file(index);
            if let Some(label) = file_labels(&shell).get(index as usize) {
                app.set_page_title(label.clone());
            }
            let (groups, others) = file_groups(&shell, index as usize);
            app.set_file_groups(Rc::new(VecModel::from(groups)).into());
            app.set_other_rows(Rc::new(VecModel::from(others)).into());
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_search_edited(move |query| {
            let Some(app) = weak.upgrade() else { return };
            let shell = shell.borrow();
            let needle = query.trim().to_owned();
            if needle.is_empty() {
                // Empty box = leave search; restore the section overview.
                let section = app.get_current_section().to_string();
                app.set_page(Page::Section);
                app.set_page_title(section.clone().into());
                refill_section(&app, &shell, &section);
                return;
            }
            let sets = chain_path_sets(&shell);
            let labels = file_labels(&shell);
            let rows: Vec<SectionRow> = shell
                .schema
                .iter()
                .filter(|entry| schema::matches(entry, &needle))
                .map(|entry| {
                    let dotted = entry.path.join(".");
                    let home = entry_home(&sets, &dotted).unwrap_or(sets.len() - 1);
                    SectionRow {
                        label: entry.label.clone().into(),
                        home_label: labels.get(home).cloned().unwrap_or_default(),
                        home_index: home as i32,
                    }
                })
                .collect();
            app.set_search_rows(Rc::new(VecModel::from(rows)).into());
            app.set_page(Page::Search);
            app.set_page_title(SharedString::from(needle));
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_raw_requested(move || {
            let Some(app) = weak.upgrade() else { return };
            let shell = shell.borrow();
            let groups = raw_groups(&shell);
            app.set_raw_groups(Rc::new(VecModel::from(groups)).into());
            app.set_page(Page::Raw);
            app.set_page_title("Other settings".into());
        });
    }
    {
        let weak = app.as_weak();
        app.on_save_requested(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status("Nothing unsaved yet — editing lands in Phase 2".into());
            }
        });
    }
    {
        let weak = app.as_weak();
        let count = shell.borrow().schema.len();
        app.on_sync_schema_requested(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(format!("{count} settings in the current schema").into());
            }
        });
    }
    {
        let weak = app.as_weak();
        let env = env.clone();
        let check_updates = settings.check_updates_on_start;
        app.on_exit_requested(move || {
            let Some(app) = weak.upgrade() else { return };
            store_window_settings(&app, &env, check_updates);
            let _ = app.hide();
            let _ = slint::quit_event_loop();
        });
    }
    // The WM ✕ goes through here: guard unsaved edits, else persist the
    // window size and let the window close.
    {
        let weak = app.as_weak();
        let env = env.clone();
        let check_updates = settings.check_updates_on_start;
        app.window().on_close_requested(move || {
            let Some(app) = weak.upgrade() else {
                return CloseRequestResponse::HideWindow;
            };
            if app.get_dirty() {
                app.set_show_exit_confirm(true);
                CloseRequestResponse::KeepWindowShown
            } else {
                store_window_settings(&app, &env, check_updates);
                CloseRequestResponse::HideWindow
            }
        });
    }

    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_set_value(move |key, value| {
            let Some(app) = weak.upgrade() else { return };
            let file_index = app.get_current_file();
            if file_index < 0 {
                return;
            }
            let file_index = file_index as usize;
            let formatted = {
                let shell = shell.borrow();
                let kind = shell
                    .schema
                    .iter()
                    .find(|entry| entry.path.join(".") == key.as_str())
                    .map(|entry| &entry.kind);
                commit_value(kind, &value)
            };
            let value_text = match formatted {
                Ok(value_text) => value_text,
                Err(err) => {
                    app.set_status(err.into());
                    return;
                }
            };
            let accepted = {
                let mut shell = shell.borrow_mut();
                let doc: &mut ConfigDocument = if file_index == shell.includes.docs.len() {
                    &mut shell.doc
                } else if let Some(inc) = shell.includes.docs.get_mut(file_index) {
                    &mut inc.doc
                } else {
                    return;
                };
                doc.set_leaf_text(&key, &value_text)
            };
            if !accepted {
                app.set_status(format!("umbriel would reject {key} = {value_text}").into());
                return;
            }
            let shell = shell.borrow();
            app.set_dirty(shell.any_modified());
            refresh_row(&app, &shell, file_index, &key);
        });
    }

    app.run()
        .map_err(|err| anyhow::anyhow!("event loop failed: {err}"))?;
    Ok(())
}

/// Persist the app settings with the current window size (logical px).
fn store_window_settings(app: &AppWindow, env: &discovery::Env, check_updates: bool) {
    let scale = app.window().scale_factor();
    let size = app.window().size();
    let _ = app_settings::store(
        env,
        &app_settings::Settings {
            check_updates_on_start: check_updates,
            window_width: (size.width as f32 / scale) as u32,
            window_height: (size.height as f32 / scale) as u32,
        },
    );
}

/// Top-level section names (sorted, deduplicated) across the schema.
fn section_names(entries: &[schema::Entry]) -> Vec<SharedString> {
    let mut names: Vec<String> = entries
        .iter()
        .map(|entry| {
            entry
                .section
                .split('.')
                .next()
                .unwrap_or("other")
                .to_owned()
        })
        .collect();
    names.sort();
    names.dedup();
    names.into_iter().map(SharedString::from).collect()
}

/// Sidebar file labels: includes in chain order, main last (chain indexing
/// convention: include i = i, main = includes.docs.len()).
fn file_labels(shell: &Shell) -> Vec<SharedString> {
    let mut labels: Vec<SharedString> = shell
        .includes
        .docs
        .iter()
        .map(|doc| SharedString::from(doc.label.as_str()))
        .collect();
    let main = shell
        .path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "config.toml".to_owned());
    labels.push(SharedString::from(format!("{main} (main)")));
    labels
}

fn status_line(shell: &Shell) -> SharedString {
    if !shell.healthy {
        let reason = shell.load_error.as_deref().unwrap_or("unknown error");
        return format!("config failed to load: {reason}").into();
    }
    let modified = if shell.doc.is_modified() {
        "modified"
    } else {
        "clean"
    };
    format!(
        "{} settings • {} included file(s) • main {}",
        shell.schema.len(),
        shell.includes.docs.len(),
        modified,
    )
    .into()
}

fn model(strings: Vec<SharedString>) -> slint::ModelRc<SharedString> {
    Rc::new(VecModel::from(strings)).into()
}

/// Per-doc leaf-path sets over the include chain: includes in order, main
/// last (chain indexing: include i = i, main = includes.docs.len()).
fn chain_path_sets(shell: &Shell) -> Vec<BTreeSet<String>> {
    let mut sets: Vec<BTreeSet<String>> = shell
        .includes
        .docs
        .iter()
        .map(|inc| inc.doc.value_paths().into_iter().collect())
        .collect();
    sets.push(shell.doc.value_paths().into_iter().collect());
    sets
}

/// Effective home of a dotted path: main wins over includes, earlier
/// includes win over later (merge order — mirrors egui's entry_home).
fn entry_home(sets: &[BTreeSet<String>], dotted: &str) -> Option<usize> {
    let main = sets.len() - 1;
    if sets[main].contains(dotted) {
        return Some(main);
    }
    sets.iter().position(|set| set.contains(dotted))
}

/// Classification for one section: pointers (set somewhere) + available
/// (schema keys set nowhere). Read-only this lesson; the destination
/// picker arrives with editing in Phase 2.
fn section_rows(shell: &Shell, section: &str) -> (Vec<SectionRow>, Vec<SharedString>) {
    let sets = chain_path_sets(shell);
    let labels = file_labels(shell);
    let mut rows = Vec::new();
    let mut available = Vec::new();
    for entry in &shell.schema {
        if entry.section.split('.').next() != Some(section) {
            continue;
        }
        let dotted = entry.path.join(".");
        match entry_home(&sets, &dotted) {
            Some(home) => rows.push(SectionRow {
                label: entry.label.clone().into(),
                home_label: labels.get(home).cloned().unwrap_or_default(),
                home_index: home as i32,
            }),
            None => available.push(entry.label.clone().into()),
        }
    }
    (rows, available)
}

/// Refill both section models; called at startup and on every section pick.
fn refill_section(app: &AppWindow, shell: &Shell, section: &str) {
    let (rows, available) = section_rows(shell, section);
    app.set_section_rows(Rc::new(VecModel::from(rows)).into());
    app.set_available_rows(Rc::new(VecModel::from(available)).into());
}

/// One file page's content: schema keys owned by the doc at `file_index`,
/// grouped by their section, plus keys beyond the schema under "Other".
fn file_groups(shell: &Shell, file_index: usize) -> (Vec<FileGroup>, Vec<FileRow>) {
    let sets = chain_path_sets(shell);
    let Some(owned) = sets.get(file_index) else {
        return (Vec::new(), Vec::new());
    };
    // Chain indexing convention: include i = i, main = includes.docs.len().
    let doc: &ConfigDocument = if file_index == shell.includes.docs.len() {
        &shell.doc
    } else {
        &shell.includes.docs[file_index].doc
    };
    let values: BTreeMap<String, String> = doc.leaf_values().into_iter().collect();

    let mut groups: BTreeMap<String, Vec<FileRow>> = BTreeMap::new();
    let mut schema_paths: BTreeSet<String> = BTreeSet::new();
    for entry in &shell.schema {
        let dotted = entry.path.join(".");
        schema_paths.insert(dotted.clone());
        if !owned.contains(&dotted) {
            continue;
        }
        groups
            .entry(entry.section.clone())
            .or_default()
            .push(file_row(doc, entry));
    }
    let groups = groups
        .into_iter()
        .map(|(title, rows)| FileGroup {
            title: title.into(),
            rows: Rc::new(VecModel::from(rows)).into(),
        })
        .collect();

    let others: Vec<FileRow> = owned
        .iter()
        .filter(|path| !schema_paths.contains(*path))
        .map(|path| {
            let value = values
                .get(path)
                .map(|raw| strip_decor(raw))
                .unwrap_or_else(|| "—".to_owned());
            FileRow {
                label: path.clone().into(),
                value: value.into(),
                key: path.clone().into(),
                kind: ValueKind::Unset,
                choices: choice_model(&schema::Kind::Text),
                swatch: slint::Color::from_argb_u8(0, 0, 0, 0).into(),
                checked: false,
                hint: String::new().into(),
            }
        })
        .collect();

    (groups, others)
}

/// One schema row, editor-ready: typed value, checked state, swatch, hint.
fn file_row(doc: &ConfigDocument, entry: &schema::Entry) -> FileRow {
    let value = typed_value(doc, entry).unwrap_or_else(|| "—".to_owned());
    let parts: Vec<&str> = entry.path.iter().map(String::as_str).collect();
    FileRow {
        label: entry.label.clone().into(),
        value: value.clone().into(),
        key: entry.path.join(".").into(),
        kind: value_kind(&entry.kind),
        choices: choice_model(&entry.kind),
        swatch: if matches!(entry.kind, schema::Kind::Color) {
            swatch_for(&value)
        } else {
            slint::Color::from_argb_u8(0, 0, 0, 0).into()
        },
        checked: doc.get_bool(&parts).unwrap_or(false),
        hint: entry_hint(entry).into(),
    }
}

/// Re-render one schema row in place after a commit. Swapping the whole
/// groups model would rebuild every editor and drop the user's focus.
fn refresh_row(app: &AppWindow, shell: &Shell, file_index: usize, key: &str) {
    let sets = chain_path_sets(shell);
    let Some(owned) = sets.get(file_index) else {
        return;
    };
    let Some(entry) = shell
        .schema
        .iter()
        .find(|entry| entry.path.join(".") == key)
    else {
        return;
    };
    if !owned.contains(key) {
        return;
    }
    let doc: &ConfigDocument = if file_index == shell.includes.docs.len() {
        &shell.doc
    } else {
        &shell.includes.docs[file_index].doc
    };
    let row = file_row(doc, entry);

    let groups = app.get_file_groups();
    let Some(groups) = groups.as_any().downcast_ref::<VecModel<FileGroup>>() else {
        return;
    };
    for gi in 0..groups.row_count() {
        let Some(group) = groups.row_data(gi) else {
            continue;
        };
        let Some(rows) = group.rows.as_any().downcast_ref::<VecModel<FileRow>>() else {
            continue;
        };
        for ri in 0..rows.row_count() {
            let Some(old) = rows.row_data(ri) else {
                continue;
            };
            if old.key.as_str() == key {
                // Same text = nothing to re-render; keeps the editor's focus.
                if old.value != row.value {
                    rows.set_row_data(ri, row);
                }
                return;
            }
        }
    }
}

/// Chain-wide sweep of keys beyond the schema and the dedicated editors,
/// grouped per file (mirrors egui's raw_rows + Raw page).
fn raw_groups(shell: &Shell) -> Vec<FileGroup> {
    let mut docs: Vec<&ConfigDocument> = shell.includes.docs.iter().map(|inc| &inc.doc).collect();
    docs.push(&shell.doc);
    let claims = schema::managed_claims(&docs);
    let schema_keys = schema::key_set(&shell.schema);
    let sets = chain_path_sets(shell);
    shell
        .includes
        .docs
        .iter()
        .map(|inc| inc.label.as_str())
        .chain(std::iter::once("Other settings"))
        .zip(docs)
        .zip(sets)
        .map(|((title, doc), set)| {
            let values: BTreeMap<String, String> = doc.leaf_values().into_iter().collect();
            let paths: Vec<String> = set.iter().cloned().collect();
            let rows: Vec<FileRow> = schema::uncovered(&paths, &schema_keys, &claims)
                .into_iter()
                .map(|path| {
                    let value = values
                        .get(&path)
                        .map(|raw| strip_decor(raw))
                        .unwrap_or_else(|| "—".to_owned());
                    FileRow {
                        label: path.clone().into(),
                        value: value.into(),
                        key: path.clone().into(),
                        kind: ValueKind::Unset,
                        choices: choice_model(&schema::Kind::Text),
                        swatch: slint::Color::from_argb_u8(0, 0, 0, 0).into(),
                        checked: false,
                        hint: String::new().into(),
                    }
                })
                .collect();
            FileGroup {
                title: title.into(),
                rows: Rc::new(VecModel::from(rows)).into(),
            }
        })
        .collect()
}

/// Dropdown vocabulary for Choice kinds; empty for everything else.
fn choice_model(kind: &schema::Kind) -> slint::ModelRc<SharedString> {
    let choices: Vec<SharedString> = match kind {
        schema::Kind::Choice(values) => values.iter().map(SharedString::from).collect(),
        schema::Kind::Curve => schema::BUILTIN_CURVES
            .iter()
            .map(|name| SharedString::from(*name))
            .collect(),
        _ => Vec::new(),
    };
    Rc::new(VecModel::from(choices)).into()
}

fn value_kind(kind: &schema::Kind) -> ValueKind {
    match kind {
        schema::Kind::Bool => ValueKind::Boolean,
        schema::Kind::Integer { .. } => ValueKind::Integer,
        schema::Kind::Float { .. } => ValueKind::Float,
        schema::Kind::Text => ValueKind::Text,
        schema::Kind::List => ValueKind::List,
        schema::Kind::Choice(_) => ValueKind::Choice,
        schema::Kind::Color => ValueKind::Color,
        schema::Kind::Curve => ValueKind::Curve,
    }
}

/// Commit form: turn editor input into the value text `set_leaf_text`
/// expects, validating/clamping per Kind. `Err` = user-facing rejection.
fn commit_value(kind: Option<&schema::Kind>, raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    match kind {
        Some(schema::Kind::List) => {
            let inner = raw.trim().trim_start_matches('[').trim_end_matches(']');
            Ok(format!("[{inner}]"))
        }
        Some(schema::Kind::Bool) => Ok(raw.to_owned()),
        Some(schema::Kind::Integer { min, max }) => {
            let mut value: i64 = raw
                .parse()
                .map_err(|_| format!("'{raw}' is not a whole number"))?;
            if let Some(min) = min {
                value = value.max(*min);
            }
            if let Some(max) = max {
                value = value.min(*max);
            }
            Ok(value.to_string())
        }
        Some(schema::Kind::Float { min, max }) => {
            let mut value: f64 = raw
                .parse()
                .map_err(|_| format!("'{raw}' is not a number"))?;
            if let Some(min) = min {
                value = value.max(*min);
            }
            if let Some(max) = max {
                value = value.min(*max);
            }
            Ok(value.to_string())
        }
        // Strings in the file are quoted; Debug-format escapes the same way
        // TOML basic strings do for the ASCII values configs use.
        Some(
            schema::Kind::Text
            | schema::Kind::Choice(_)
            | schema::Kind::Color
            | schema::Kind::Curve,
        ) => Ok(format!("{raw:?}")),
        _ => Ok(raw.to_owned()),
    }
}

/// Hex color (#RRGGBB or #RRGGBBAA) to swatch brush; black when unparseable.
fn swatch_for(value: &str) -> slint::Brush {
    let hex = value.trim().trim_start_matches('#');
    let channel = |range: std::ops::Range<usize>| {
        hex.get(range)
            .and_then(|bits| u8::from_str_radix(bits, 16).ok())
            .unwrap_or(0)
    };
    slint::Brush::from(slint::Color::from_rgb_u8(
        channel(0..2),
        channel(2..4),
        channel(4..6),
    ))
}

/// Clean, editor-ready text for a schema key, read through the typed
/// getters — never the TOML repr, which carries trailing comments.
fn typed_value(doc: &ConfigDocument, entry: &schema::Entry) -> Option<String> {
    let parts: Vec<&str> = entry.path.iter().map(String::as_str).collect();
    match &entry.kind {
        schema::Kind::Bool => doc.get_bool(&parts).map(|v| v.to_string()),
        schema::Kind::Integer { .. } => doc.get_integer(&parts).map(|v| v.to_string()),
        schema::Kind::Float { .. } => doc.get_float(&parts).map(|v| v.to_string()),
        schema::Kind::List => list_text(doc, &parts),
        schema::Kind::Text
        | schema::Kind::Choice(_)
        | schema::Kind::Color
        | schema::Kind::Curve => doc.get_string(&parts),
    }
}

/// Comma-joined list display, mirroring egui's array_text.
fn list_text(doc: &ConfigDocument, parts: &[&str]) -> Option<String> {
    if let Some(values) = doc.get_integers(parts) {
        return Some(
            values
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if let Some(values) = doc.get_floats(parts) {
        return Some(
            values
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    doc.get_strings(parts).map(|values| values.join(", "))
}

/// Mined metadata shown beside the editor: range from the Kind payload,
/// then the mined unit. Empty when the key carries neither.
fn entry_hint(entry: &schema::Entry) -> String {
    let range = match &entry.kind {
        schema::Kind::Integer { min, max } => Some(range_text(
            min.map(|v| v.to_string()),
            max.map(|v| v.to_string()),
        )),
        schema::Kind::Float { min, max } => Some(range_text(
            min.map(|v| v.to_string()),
            max.map(|v| v.to_string()),
        )),
        _ => None,
    };
    let mut hints: Vec<String> = range.into_iter().filter(|r| !r.is_empty()).collect();
    if let Some(unit) = &entry.unit {
        hints.push(unit.clone());
    }
    hints.join(" • ")
}

fn range_text(min: Option<String>, max: Option<String>) -> String {
    match (min, max) {
        (Some(min), Some(max)) => format!("{min}–{max}"),
        (Some(min), None) => format!("≥ {min}"),
        (None, Some(max)) => format!("≤ {max}"),
        (None, None) => String::new(),
    }
}

/// Best-effort decor strip for non-schema values (repr may carry comments).
fn strip_decor(raw: &str) -> String {
    let trimmed = raw.trim();
    match trimmed.split_once(" #") {
        Some((value, _)) => value.trim_end().to_owned(),
        None => trimmed.to_owned(),
    }
}
