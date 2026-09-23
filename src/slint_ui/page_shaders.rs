//! The Shaders page: the library, community downloads with update
//! checks, the assignment dropdowns, the visual editor with its preview
//! scrubber.

use super::common::*;
use super::*;

const SHADERS_UPSTREAM_COMMITS: &str =
    "https://api.github.com/repos/noctalia-dev/community-umbriel-shaders/commits?per_page=1";
const SHADERS_UPSTREAM_MARKER: &str = ".upstream";

/// The community repo's newest commit SHA, or `None` when GitHub is
/// unreachable — an unknown upstream never flips the update marker.
fn fetch_latest_commit_sha() -> Option<String> {
    github_latest_commit(SHADERS_UPSTREAM_COMMITS).map(|(sha, _, _)| sha)
}

/// The SHA recorded when the community collection was last downloaded.
fn installed_commit_sha(target: &Path) -> Option<String> {
    std::fs::read_to_string(target.join(SHADERS_UPSTREAM_MARKER))
        .ok()
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

/// Once per session, compare the downloaded collection's recorded
/// upstream SHA against GitHub's newest and flip the page's
/// update-available hint. The verdict lands straight on the window —
/// the `Shell` can't be reached from a worker thread. An unreachable
/// upstream or a collection downloaded before SHA marking leaves the
/// indicator untouched rather than guessing.
pub(super) fn maybe_check_shader_updates(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    let marker = {
        let mut shell = shell.borrow_mut();
        if !shell.shaders_installed || shell.shaders_update_checked {
            return;
        }
        shell.shaders_update_checked = true;
        let target = shell
            .path
            .parent()
            .map(|dir| dir.join("shaders/community"))
            .unwrap_or_default();
        installed_commit_sha(&target)
    };
    // No recorded SHA: nothing to compare against, so don't guess.
    let Some(marker) = marker else {
        return;
    };
    let weak = app.as_weak();
    std::thread::spawn(move || {
        let Some(upstream) = fetch_latest_commit_sha() else {
            return;
        };
        let update_available = marker != upstream;
        let _ = slint::invoke_from_event_loop(move || {
            let Some(app) = weak.upgrade() else { return };
            app.set_shader_update_available(update_available);
        });
    });
}

/// Download the community shader collection as a tarball over HTTPS —
/// no git required — and unpack it into `<config dir>/shaders/community`.
/// The directory is app-managed: a re-download replaces it wholesale
/// (users' own shaders belong directly in `shaders/`). The staging dir
/// is dot-hidden so a half-finished download never shows in the scan.
/// Returns the user-facing note.
fn download_community_shaders(target: &Path) -> Result<String, String> {
    const URL: &str =
        "https://github.com/noctalia-dev/community-umbriel-shaders/archive/refs/heads/main.tar.gz";
    let updating = target.exists();
    // Recorded so the page can tell "up to date" from "updates waiting".
    // Best effort: a failed lookup just leaves no marker.
    let upstream_sha = fetch_latest_commit_sha();
    let archive = ureq::get(URL)
        .header("User-Agent", "umbriel-config")
        .config()
        .timeout_global(Some(std::time::Duration::from_secs(60)))
        .build()
        .call()
        .and_then(|mut response| response.body_mut().read_to_vec())
        .map_err(|err| format!("Download failed: {err}"))?;

    let staging = target.with_file_name(".community-staging");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)
        .map_err(|err| format!("Could not create {}: {err}", staging.display()))?;

    let archive_path = std::env::temp_dir().join(format!(
        "umbriel-community-shaders-{}.tar.gz",
        std::process::id()
    ));
    if let Err(err) = std::fs::write(&archive_path, &archive) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("Could not store the download: {err}"));
    }
    let extracted = std::process::Command::new("tar")
        .args([
            "-xzf",
            &archive_path.to_string_lossy(),
            "-C",
            &staging.to_string_lossy(),
            "--strip-components=1",
        ])
        .output();
    let _ = std::fs::remove_file(&archive_path);
    match extracted {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!(
                "Extraction failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Err(_) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err("tar is not installed — cannot unpack the download.".to_owned());
        }
    }

    // Everything landed: swap the old collection for the fresh one.
    if target.exists()
        && let Err(err) = std::fs::remove_dir_all(target)
    {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("Could not replace {}: {err}", target.display()));
    }
    if let Err(err) = std::fs::rename(&staging, target) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!(
            "Could not install into {}: {err}",
            target.display()
        ));
    }
    if let Some(sha) = upstream_sha {
        let _ = std::fs::write(target.join(SHADERS_UPSTREAM_MARKER), format!("{sha}\n"));
    }
    Ok(if updating {
        "Community shaders updated.".to_owned()
    } else {
        "Community shaders downloaded.".to_owned()
    })
}

/// Show the builder stack as step cards. Rows only: the code pane is
/// the caller's business.
fn push_builder_rows(app: &AppWindow, steps: &[shaders::builder::BuilderStep]) {
    let rows: Vec<ShaderStep> = steps
        .iter()
        .enumerate()
        .filter_map(|(index, step)| {
            let def = shaders::builder::step_def(step.kind)?;
            // Fixed-shape struct: up to three parameter slots, padded.
            let mut labels: [SharedString; 3] = std::array::from_fn(|_| SharedString::new());
            let mut values = [0.0f32; 3];
            let mut mins = [0.0f32; 3];
            let mut maxs = [0.0f32; 3];
            for (slot, param) in def.params.iter().enumerate() {
                labels[slot] = param.label.into();
                values[slot] = step.params[slot] as f32;
                mins[slot] = param.min as f32;
                maxs[slot] = param.max as f32;
            }
            Some(ShaderStep {
                index: index as i32,
                label: def.label.into(),
                p1_label: labels[0].clone(),
                p1_value: values[0],
                p1_min: mins[0],
                p1_max: maxs[0],
                p2_label: labels[1].clone(),
                p2_value: values[1],
                p2_min: mins[1],
                p2_max: maxs[1],
                p3_label: labels[2].clone(),
                p3_value: values[2],
                p3_min: mins[2],
                p3_max: maxs[2],
                p_count: def.params.len() as i32,
            })
        })
        .collect();
    app.set_shader_steps(Rc::new(VecModel::from(rows)).into());
}

/// Push the builder stack into the step panel and regenerate the code
/// pane from it. Callers only reach this with the builder unlocked, so
/// the code being replaced is itself builder output.
fn regen_builder(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    let (steps, code) = {
        let shell = shell.borrow();
        let steps = shell.builder_steps.clone();
        let code = shaders::builder::generate_stack(&steps);
        (steps, code)
    };
    push_builder_rows(app, &steps);
    app.set_shader_builder_locked(false);
    let preview_text = code.clone();
    app.set_shader_editor_text(code.into());
    // Stack changes regenerate the code, so the preview compiles the new
    // source too (the worker re-renders at the last scrub position).
    shell
        .borrow_mut()
        .preview_command(shader_preview::PreviewCommand::SetSource(preview_text));
}

/// Point the builder at whatever the code pane holds: builder output
/// fills the stack, anything else locks the builder so its next change
/// can't overwrite hand-written code.
fn sync_builder_from_code(app: &AppWindow, shell: &Rc<RefCell<Shell>>, code: &str) {
    match shaders::builder::parse_stack(code) {
        Some(steps) => {
            push_builder_rows(app, &steps);
            shell.borrow_mut().builder_steps = steps;
            app.set_shader_builder_locked(false);
        }
        None => {
            push_builder_rows(app, &[]);
            shell.borrow_mut().builder_steps.clear();
            app.set_shader_builder_locked(true);
        }
    }
}

/// Open the overlay editor pre-loaded with a shader file's content.
/// `editing` reuses that exact file on save; forking leaves the source
/// untouched and saves under a new name.
fn open_shader_editor(
    app: &AppWindow,
    shell: &Rc<RefCell<Shell>>,
    path: &str,
    editing: bool,
    name: String,
) {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            app.set_status(format!("could not read {path}: {err}").into());
            return;
        }
    };
    shell.borrow_mut().shader_editing = editing.then(|| PathBuf::from(path));
    app.set_shader_editor_editing(editing);
    app.set_shader_editor_name(name.into());
    sync_builder_from_code(app, shell, &text);
    app.set_shader_editor_text(text.into());
    app.set_shader_editor_note(String::new().into());
    show_editor(app, shell);
    kick_shader_preview(app, shell);
}

/// Chain index for a brand-new assignment; see
/// [`shaders::new_assignment_home`].
fn new_home(shell: &Shell) -> usize {
    let mut names: Vec<String> = shell
        .includes
        .docs
        .iter()
        .map(|inc| file_name_of(&inc.path))
        .collect();
    names.push(file_name_of(&shell.path));
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    shaders::new_assignment_home(&chain_docs(shell), &names)
}

/// Delete one of the user's own shaders, then rescan. An editor open on
/// that same file closes with it.
fn delete_shader(app: &AppWindow, shell: &Rc<RefCell<Shell>>, path: &Path) {
    let result = shell
        .borrow()
        .path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "could not determine the config directory.".to_owned())
        .and_then(|dir| shaders::delete_user_shader(&dir, path));
    match result {
        Ok(()) => {
            {
                let mut shell = shell.borrow_mut();
                if shell.shader_editing.as_deref() == Some(path) {
                    shell.shader_editing = None;
                    app.set_shader_editor_open(false);
                }
                scan_shaders(&mut shell);
            }
            let shell = shell.borrow();
            rebuild_shaders(app, &shell);
            app.set_status(format!("Deleted {}.", path.display()).into());
        }
        Err(err) if app.get_shader_editor_open() => app.set_shader_editor_note(err.into()),
        Err(err) => app.set_status(err.into()),
    }
}

/// Show the editor overlay over whatever code is loaded, remembering it
/// as the unsaved-changes baseline.
fn show_editor(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    shell.borrow_mut().shader_editor_baseline = app.get_shader_editor_text().to_string();
    app.set_shader_editor_confirm_close(false);
    app.set_shader_editor_open(true);
}

/// Park the scrubber mid-animation and hand the freshly loaded code to
/// the preview worker. Not the opening frame: at progress 0 a fade-in
/// is fully transparent and the preview looks broken.
fn kick_shader_preview(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    const START: f32 = 0.5;
    app.set_shader_preview_progress(START);
    app.set_shader_preview_direction(1.0);
    app.set_shader_preview_note(String::new().into());
    let text = app.get_shader_editor_text().to_string();
    let mut shell = shell.borrow_mut();
    shell.preview_command(shader_preview::PreviewCommand::SetSource(text));
    shell.preview_command(shader_preview::PreviewCommand::Render {
        progress: START,
        direction: 1.0,
    });
}

/// Apply pending worker events to the UI. Runs on the poll timer; the
/// worker never touches Slint directly.
pub(super) fn poll_shader_preview(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    let events = match &shell.borrow().shader_preview {
        Some(handle) => handle.drain(),
        None => Vec::new(),
    };
    for event in events {
        match event {
            shader_preview::PreviewEvent::Ready => {
                app.set_shader_preview_ready(true);
            }
            shader_preview::PreviewEvent::Unavailable(err) => {
                let mut shell = shell.borrow_mut();
                shell.shader_preview = None;
                shell.shader_preview_failed = true;
                app.set_shader_preview_ready(false);
                app.set_shader_preview_note(format!("Preview unavailable: {err}").into());
            }
            shader_preview::PreviewEvent::Compiled(problems) => {
                let note = problems.map_or_else(String::new, |err| format!("GLSL error: {err}"));
                app.set_shader_preview_note(note.into());
            }
            shader_preview::PreviewEvent::Frame(width, height, pixels) => {
                let mut buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(width, height);
                let rgba: Vec<slint::Rgba8Pixel> = pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|chunk| slint::Rgba8Pixel::new(chunk[0], chunk[1], chunk[2], chunk[3]))
                    .collect();
                buffer.make_mut_slice().copy_from_slice(&rgba);
                app.set_shader_preview_image(slint::Image::from_rgba8(buffer));
            }
        }
    }
}

/// Rescan shader locations: the main config's directory (its
/// `shaders/` folder holds user copies and the community clone) and
/// umbriel's installed data directory (bundled effects).
pub(super) fn scan_shaders(shell: &mut Shell) {
    let config_dir = shell
        .path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    // Bundled shaders live at <data root>/umbriel/shaders; the
    // packaged default config sits at <data root>/umbriel/config.toml.
    let env = discovery::Env::from_process();
    let data_roots: Vec<PathBuf> = discovery::packaged_default(&env)
        .and_then(|path| path.parent().and_then(Path::parent).map(Path::to_path_buf))
        .into_iter()
        .collect();
    shell.shaders = shaders::scan(&config_dir, &data_roots);
    shell.shaders_installed = config_dir.join("shaders/community").is_dir();
}

pub(super) fn rebuild_shaders(app: &AppWindow, shell: &Shell) {
    let infos: Vec<ShaderInfo> = shell
        .shaders
        .iter()
        .map(|entry| ShaderInfo {
            name: entry.name.clone().into(),
            value: entry.value.clone().into(),
            source: entry.source.label().into(),
            description: entry.description.clone().into(),
            invalid: entry.invalid.clone().unwrap_or_default().into(),
            path: entry.path.display().to_string().into(),
            is_own: entry.source == shaders::Source::ConfigDir,
        })
        .collect();
    app.set_shaders(Rc::new(VecModel::from(infos)).into());

    let docs = chain_docs(shell);
    let choice_values: Vec<String> = shell
        .shaders
        .iter()
        .map(|entry| entry.value.clone())
        .collect();
    let rows: Vec<ShaderAssignment> = shaders::EVENTS
        .iter()
        .map(|event| {
            let mut choices: Vec<SharedString> = vec!["(no shader)".into()];
            choices.extend(shell.shaders.iter().map(|entry| entry.label.clone().into()));
            let current = shaders::current_assignment(&docs, event).map(|(value, _)| value);
            let index = current.as_ref().and_then(|value| {
                choice_values
                    .iter()
                    .position(|candidate| candidate == value)
            });
            // A value no scan found gets its own trailing entry, so the
            // dropdown shows it and "(no shader)" is a real change.
            let missing = current.is_some() && index.is_none();
            let current_index = match (index, &current) {
                (Some(position), _) => position + 1,
                (None, Some(value)) => {
                    choices.push(format!("⚠ {value} (missing)").into());
                    choices.len() - 1
                }
                (None, None) => 0,
            };
            ShaderAssignment {
                key: format!("animation.{event}.shader").into(),
                label: prettify(event).into(),
                choices: Rc::new(VecModel::from(choices)).into(),
                current: current_index as i32,
                current_value: current.unwrap_or_default().into(),
                missing,
            }
        })
        .collect();
    app.set_shader_assignments(Rc::new(VecModel::from(rows)).into());
    app.set_shader_download_note(shell.shader_note.clone().into());
    app.set_shader_include_missing(shaders::missing_include(&shell.doc, &shell.path).is_some());
    app.set_shader_community_installed(shell.shaders_installed);
    app.set_changed_count(changed_count(shell));
}

pub(super) fn install_shaders(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shaders_download(move || {
            let Some(app) = weak.upgrade() else { return };
            // The collection lands next to the config the app already
            // opened — not in a re-derived XDG path, which may be unset.
            let target = match shell.borrow().path.parent() {
                Some(dir) => dir.join("shaders/community"),
                None => {
                    app.set_shader_download_note(
                        "Could not determine the config directory.".into(),
                    );
                    return;
                }
            };
            app.set_shader_downloading(true);
            app.set_shader_download_note(String::new().into());
            let weak_for_thread = weak.clone();
            std::thread::spawn(move || {
                let weak = weak_for_thread;
                let result = download_community_shaders(&target);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(app) = weak.upgrade() else { return };
                    app.set_shader_downloading(false);
                    match result {
                        Ok(note) => {
                            // Re-enter the page's own open path: rescans
                            // the library and rebuilds the assignments.
                            // Elsewhere, leave the user where they are;
                            // the page rescans when next opened.
                            if app.get_page() == Page::Shaders {
                                app.invoke_section_selected("shaders".into());
                            }
                            app.set_shader_update_available(false);
                            app.set_shader_download_note(note.into());
                        }
                        Err(note) => app.set_shader_download_note(note.into()),
                    }
                });
            });
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_assign(move |key, index| {
            let Some(app) = weak.upgrade() else { return };
            let Some(event) = key.rsplit('.').nth(1).map(str::to_owned) else {
                return;
            };
            {
                let mut shell = shell.borrow_mut();
                // Resolve the pick against the list the dropdown was built
                // from; an index past it changes nothing.
                let value = match usize::try_from(index) {
                    Ok(0) => None,
                    Ok(index) => shell
                        .shaders
                        .get(index - 1)
                        .map(|entry| entry.value.clone()),
                    Err(_) => None,
                };
                let clearing = index == 0;
                let home = shaders::assignment_home(&chain_docs(&shell), &event);
                let path = ["animation", event.as_str(), "shader"];
                match (value, home) {
                    // A typed string write, so the path is always quoted.
                    // A brand-new key starts beside the other assignments;
                    // the save popup can still move it.
                    (Some(value), home) => {
                        let target = home.unwrap_or_else(|| new_home(&shell));
                        doc_at_mut(&mut shell, target).set_string(&path, &value);
                    }
                    (None, Some(home)) if clearing => {
                        doc_at_mut(&mut shell, home).remove_leaf(&path);
                    }
                    _ => {}
                }
            }
            // Always rebuild so the dropdowns mirror the documents, even
            // when the pick changed nothing.
            let shell = shell.borrow();
            app.set_dirty(shell.any_modified());
            rebuild_shaders(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_fix_include(move || {
            let Some(app) = weak.upgrade() else { return };
            let Some(entry) = ({
                let shell = shell.borrow();
                shaders::missing_include(&shell.doc, &shell.path)
            }) else {
                return;
            };
            {
                let mut shell = shell.borrow_mut();
                let mut files = shell
                    .doc
                    .get_strings(&["include", "files"])
                    .unwrap_or_default();
                files.push(entry);
                shell.doc.set_strings(&["include", "files"], &files);
            }
            let shell = shell.borrow();
            app.set_status("Added shaders.toml to [include] — save to apply.".into());
            app.set_dirty(shell.any_modified());
            rebuild_shaders(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_editor_new(move || {
            let Some(app) = weak.upgrade() else { return };
            {
                let mut shell = shell.borrow_mut();
                shell.shader_editing = None;
                shell.builder_steps = shaders::builder::default_steps();
            }
            app.set_shader_editor_editing(false);
            app.set_shader_editor_name(String::new().into());
            regen_builder(&app, &shell);
            app.set_shader_editor_note(String::new().into());
            show_editor(&app, &shell);
            kick_shader_preview(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        // Editing loads one of the user's own shaders in place; forking
        // loads any shader's code into a fresh, unsaved editor.
        app.on_shader_editor_edit(move |path| {
            let Some(app) = weak.upgrade() else { return };
            let own = shell.borrow().shaders.iter().any(|entry| {
                entry.source == shaders::Source::ConfigDir
                    && entry.path.to_string_lossy() == path.as_str()
            });
            if !own {
                return;
            }
            let stem = Path::new(path.as_str())
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            open_shader_editor(&app, &shell, &path, true, stem);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_editor_fork(move |path, name| {
            let Some(app) = weak.upgrade() else { return };
            open_shader_editor(&app, &shell, &path, false, format!("{name}-fork"));
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_editor_save(move || {
            let Some(app) = weak.upgrade() else { return };
            let text = app.get_shader_editor_text().to_string();
            let result = {
                let shell = shell.borrow_mut();
                match shell.shader_editing.clone() {
                    Some(path) => shaders::write_user_shader(&path, &text).map(|_| path),
                    None => match shell.path.parent().map(Path::to_path_buf) {
                        Some(dir) => shaders::save_user_shader(
                            &dir,
                            app.get_shader_editor_name().as_str(),
                            &text,
                        ),
                        None => Err("could not determine the config directory.".to_owned()),
                    },
                }
            };
            match result {
                Ok(path) => {
                    {
                        let mut shell = shell.borrow_mut();
                        shell.shader_editing = Some(path.clone());
                        shell.shader_editor_baseline = text.clone();
                        scan_shaders(&mut shell);
                    }
                    let shell = shell.borrow();
                    rebuild_shaders(&app, &shell);
                    // Subsequent saves overwrite the same file.
                    app.set_shader_editor_editing(true);
                    app.set_shader_editor_note(String::new().into());
                    app.set_status(
                        format!("Saved {} — umbriel live-reloads it.", path.display()).into(),
                    );
                }
                Err(err) => app.set_shader_editor_note(err.into()),
            }
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_editor_delete(move || {
            let Some(app) = weak.upgrade() else { return };
            let Some(path) = shell.borrow().shader_editing.clone() else {
                return;
            };
            delete_shader(&app, &shell, &path);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        // A library card's Delete names its own file; it never goes
        // through the editor's remembered path, which may be stale.
        app.on_shader_delete_file(move |path| {
            let Some(app) = weak.upgrade() else { return };
            let path = PathBuf::from(path.as_str());
            let own = shell
                .borrow()
                .shaders
                .iter()
                .any(|entry| entry.source == shaders::Source::ConfigDir && entry.path == path);
            if own {
                delete_shader(&app, &shell, &path);
            }
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_editor_text_changed(move |text| {
            let Some(app) = weak.upgrade() else { return };
            let problems = shaders::lint_source(&text);
            let note = if problems.is_empty() {
                String::new()
            } else {
                format!("⚠ {}", problems.join("; "))
            };
            app.set_shader_editor_note(note.into());
            // The builder follows the code: typing builder-shaped code
            // updates the steps, anything else locks the builder.
            sync_builder_from_code(&app, &shell, &text);
            // Live GLSL checking: the preview worker compiles as you type.
            shell
                .borrow_mut()
                .preview_command(shader_preview::PreviewCommand::SetSource(text.to_string()));
        });
    }
    // Code-editor keys: pure text surgery, see shaders::code_edit.
    app.on_shader_code_key(|text, anchor, cursor, kind| {
        let key = match kind.as_str() {
            "outdent" => shaders::code_edit::Key::Outdent,
            "newline" => shaders::code_edit::Key::Newline,
            _ => shaders::code_edit::Key::Indent,
        };
        let offset = |value: i32| usize::try_from(value).unwrap_or(0);
        let (text, anchor, cursor) =
            shaders::code_edit::apply(&text, offset(anchor), offset(cursor), key);
        CodeEdit {
            text: text.into(),
            anchor: anchor as i32,
            cursor: cursor as i32,
        }
    });
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        // Cancel and the scrim both land here: unsaved code asks first.
        app.on_shader_editor_close(move || {
            let Some(app) = weak.upgrade() else { return };
            let unsaved =
                app.get_shader_editor_text().as_str() != shell.borrow().shader_editor_baseline;
            if unsaved {
                app.set_shader_editor_confirm_close(true);
            } else {
                app.set_shader_editor_open(false);
            }
        });
    }
    {
        // Locked builder's way out: discard the code for a fresh stack.
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_builder_reset(move || {
            let Some(app) = weak.upgrade() else { return };
            shell.borrow_mut().builder_steps = shaders::builder::default_steps();
            regen_builder(&app, &shell);
        });
    }
    {
        // The builder stack: every mutation regenerates the code.
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_step_add(move |label| {
            let Some(app) = weak.upgrade() else { return };
            if app.get_shader_builder_locked() {
                return;
            }
            let Some(def) = shaders::builder::STEP_DEFS
                .iter()
                .find(|def| def.label == label.as_str())
            else {
                return;
            };
            shell.borrow_mut().builder_steps.push(def.default_step());
            regen_builder(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_step_remove(move |index| {
            let Some(app) = weak.upgrade() else { return };
            if app.get_shader_builder_locked() {
                return;
            }
            {
                let mut shell = shell.borrow_mut();
                if (index as usize) < shell.builder_steps.len() {
                    shell.builder_steps.remove(index as usize);
                }
            }
            regen_builder(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_step_move(move |index, delta| {
            let Some(app) = weak.upgrade() else { return };
            if app.get_shader_builder_locked() {
                return;
            }
            {
                let mut shell = shell.borrow_mut();
                let from = index as usize;
                let to = from.saturating_add_signed(delta as isize);
                if to < shell.builder_steps.len() {
                    shell.builder_steps.swap(from, to);
                }
            }
            regen_builder(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_step_param(move |index, param, value| {
            let Some(app) = weak.upgrade() else { return };
            if app.get_shader_builder_locked() {
                return;
            }
            {
                let mut shell = shell.borrow_mut();
                let Some(step) = shell.builder_steps.get_mut(index as usize) else {
                    return;
                };
                let Some(def) = shaders::builder::step_def(step.kind) else {
                    return;
                };
                let Some(slot) = def.params.get(param as usize) else {
                    return;
                };
                let clamped = (value as f64).clamp(slot.min, slot.max);
                step.params[param as usize] = clamped;
            }
            regen_builder(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_preview_scrub(move |progress| {
            let Some(app) = weak.upgrade() else { return };
            shell
                .borrow_mut()
                .preview_command(shader_preview::PreviewCommand::Render {
                    progress: progress.clamp(0.0, 1.0),
                    direction: app.get_shader_preview_direction(),
                });
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_preview_direction_toggled(move || {
            let Some(app) = weak.upgrade() else { return };
            let flipped = if app.get_shader_preview_direction() > 0.0 {
                -1.0
            } else {
                1.0
            };
            app.set_shader_preview_direction(flipped);
            shell
                .borrow_mut()
                .preview_command(shader_preview::PreviewCommand::Render {
                    progress: app.get_shader_preview_progress().clamp(0.0, 1.0),
                    direction: flipped,
                });
        });
    }
}
