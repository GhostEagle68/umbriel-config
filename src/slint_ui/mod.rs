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
    CloseRequestResponse, ComponentHandle, LogicalSize, SharedString, VecModel, WindowSize,
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
        let value = values
            .get(&dotted)
            .cloned()
            .unwrap_or_else(|| "—".to_owned());
        groups
            .entry(entry.section.clone())
            .or_default()
            .push(FileRow {
                label: entry.label.clone().into(),
                value: value.into(),
            });
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
        .map(|path| FileRow {
            label: path.clone().into(),
            value: values
                .get(path)
                .cloned()
                .unwrap_or_else(|| "—".to_owned())
                .into(),
        })
        .collect();

    (groups, others)
}
