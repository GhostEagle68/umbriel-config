//! The Shaders page: the library, community downloads with update
//! checks, the assignment dropdowns, the visual editor with its preview
//! scrubber.

use super::common::*;
use super::*;

const SHADERS_UPSTREAM_COMMITS: &str =
    "https://api.github.com/repos/noctalia-dev/community-umbriel-shaders/commits?per_page=1";
const SHADERS_UPSTREAM_MARKER: &str = ".upstream";
/// Largest community archive the download accepts.
const ARCHIVE_LIMIT: u64 = 64 * 1024 * 1024;

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
        // ureq's plain read_to_vec stops at 10 MB; the collection only
        // grows, so allow well past that.
        .and_then(|mut response| {
            response
                .body_mut()
                .with_config()
                .limit(ARCHIVE_LIMIT)
                .read_to_vec()
        })
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
            let mut decimals = [0i32; 3];
            for (slot, param) in def.params.iter().enumerate() {
                labels[slot] = param.label.into();
                values[slot] = step.params[slot] as f32;
                mins[slot] = param.min as f32;
                maxs[slot] = param.max as f32;
                decimals[slot] = param.decimals as i32;
            }
            Some(ShaderStep {
                index: index as i32,
                label: def.label.into(),
                p1_label: labels[0].clone(),
                p1_value: values[0],
                p1_min: mins[0],
                p1_max: maxs[0],
                p1_decimals: decimals[0],
                p2_label: labels[1].clone(),
                p2_value: values[1],
                p2_min: mins[1],
                p2_max: maxs[1],
                p2_decimals: decimals[1],
                p3_label: labels[2].clone(),
                p3_value: values[2],
                p3_min: mins[2],
                p3_max: maxs[2],
                p3_decimals: decimals[2],
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
    let steps = shell.borrow().builder_steps.clone();
    push_builder_rows(app, &steps);
    app.set_shader_builder_locked(false);
    regen_code(app, shell);
}

/// Regenerate the code pane (and the preview's source) from the builder
/// stack, leaving the step cards alone: a slider mid-drag must not be
/// recreated under the pointer.
fn regen_code(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    let code = shaders::builder::generate_stack(&shell.borrow().builder_steps);
    let preview_text = code.clone();
    app.set_shader_editor_text(code.into());
    // Stack changes regenerate the code, so the preview compiles the new
    // source too (the worker re-renders at the last scrub position).
    shell
        .borrow_mut()
        .preview_command(shader_preview::PreviewCommand::SetSource(preview_text));
}

/// Set one builder parameter (clamped to its range). False when the
/// step or slot doesn't exist.
fn set_step_param(shell: &Rc<RefCell<Shell>>, index: i32, param: i32, value: f32) -> bool {
    let mut shell = shell.borrow_mut();
    let Some(step) = shell.builder_steps.get_mut(index as usize) else {
        return false;
    };
    let Some(def) = shaders::builder::step_def(step.kind) else {
        return false;
    };
    let Some(slot) = def.params.get(param as usize) else {
        return false;
    };
    step.params[param as usize] = (value as f64).clamp(slot.min, slot.max);
    true
}

/// Point the builder at whatever the code pane holds: builder output
/// fills the stack, anything else locks the builder so its next change
/// can't overwrite hand-written code.
fn sync_builder_from_code(app: &AppWindow, shell: &Rc<RefCell<Shell>>, code: &str) {
    let steps = shaders::builder::parse_stack(code);
    let locked = steps.is_none();
    let steps = steps.unwrap_or_default();
    // Rebuilding the step cards recreates every slider: only do it when
    // the stack or the lock actually changed.
    let unchanged =
        shell.borrow().builder_steps == steps && app.get_shader_builder_locked() == locked;
    if unchanged {
        return;
    }
    push_builder_rows(app, &steps);
    shell.borrow_mut().builder_steps = steps;
    app.set_shader_builder_locked(locked);
}

/// How long typing must pause before the code is re-checked.
const CODE_SETTLE: std::time::Duration = std::time::Duration::from_millis(200);

/// The per-edit work, run once typing settles: lint note, builder sync
/// (the builder follows the code) and a preview compile.
fn code_settled(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    let text = app.get_shader_editor_text().to_string();
    let problems = shaders::lint_source(&text);
    let note = if problems.is_empty() {
        String::new()
    } else {
        format!("⚠ {}", problems.join("; "))
    };
    app.set_shader_editor_note(note.into());
    sync_builder_from_code(app, shell, &text);
    shell
        .borrow_mut()
        .preview_command(shader_preview::PreviewCommand::SetSource(text));
}

/// Run a pending settle now, so a builder action never acts on stale
/// code (typed a moment ago, not yet synced).
fn settle_now(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    let pending = shell.borrow().shader_code_settle.running();
    if pending {
        shell.borrow().shader_code_settle.stop();
        code_settled(app, shell);
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
    app.set_shader_editor_used_by(used_by(&shell.borrow(), Path::new(path)).into());
    app.set_shader_editor_name(name.into());
    sync_builder_from_code(app, shell, &text);
    app.set_shader_editor_text(text.into());
    app.set_shader_editor_note(String::new().into());
    show_editor(app, shell);
    kick_shader_preview(app, shell);
}

/// Point `event` at `shader`, or clear it with `None`, as an unsaved
/// change. An existing key is edited where it lives; a brand-new one
/// starts beside the other assignments (the save popup can move it).
/// The value is spelled for the file it lands in, since umbriel
/// resolves relative paths from there, and written as a typed string
/// so it is always quoted.
fn assign_event(shell: &mut Shell, event: &str, shader: Option<&Path>) {
    let home = shaders::assignment_home(&chain_docs(shell), event);
    let key = ["animation", event, "shader"];
    match (shader, home) {
        (Some(shader), home) => {
            let target = home.unwrap_or_else(|| new_home(shell));
            let value = shaders::value_for(shader, &chain_paths(shell)[target]);
            doc_at_mut(shell, target).set_string(&key, &value);
        }
        (None, Some(home)) => {
            doc_at_mut(shell, home).remove_leaf(&key);
        }
        (None, None) => {}
    }
}

/// The "Use for" checklist: every event, what it uses now, ticked when
/// it already uses the shader being saved or is the one being previewed.
fn use_for_rows(shell: &Shell) -> Vec<ShaderUse> {
    let docs = chain_docs(shell);
    let paths = chain_paths(shell);
    let this = shell.shader_editing.as_deref();
    shaders::EVENTS
        .iter()
        .enumerate()
        .map(|(index, event)| {
            let resolved = shaders::current_assignment(&docs, event)
                .map(|(value, doc)| (shaders::resolve(&value, &paths[doc]), value));
            let uses_this = match (&resolved, this) {
                (Some((resolved, _)), Some(this)) => shaders::same_file(resolved, this),
                _ => false,
            };
            let current = match &resolved {
                None => String::new(),
                Some(_) if uses_this => "uses this shader".to_owned(),
                Some((resolved, value)) => {
                    let name = shell
                        .shaders
                        .iter()
                        .find(|entry| shaders::same_file(&entry.path, resolved))
                        .map_or(value.as_str(), |entry| entry.name.as_str());
                    format!("uses {name}")
                }
            };
            ShaderUse {
                label: prettify(event).into(),
                current: current.into(),
                checked: uses_this || index == shell.shader_preview_event,
            }
        })
        .collect()
}

/// Apply the "Use for" checklist to the saved shader at `path`: ticked
/// events switch to it, unticked ones that used it are cleared, as
/// unsaved changes. Returns what changed, for the status line.
fn apply_use_for(shell: &mut Shell, app: &AppWindow, path: &Path) -> Vec<String> {
    let rows = app.get_shader_use_for();
    let using: Vec<&str> = assignments_of(shell, path)
        .iter()
        .map(|(event, _)| *event)
        .collect();
    let mut changes = Vec::new();
    for (index, event) in shaders::EVENTS.iter().enumerate() {
        let Some(row) = rows.row_data(index) else {
            continue;
        };
        let uses = using.contains(event);
        if row.checked && !uses {
            assign_event(shell, event, Some(path));
            changes.push(format!("now used for {}", prettify(event)));
        } else if !row.checked && uses {
            assign_event(shell, event, None);
            changes.push(format!("no longer used for {}", prettify(event)));
        }
    }
    changes
}

/// Chain index for a brand-new assignment; see
/// [`shaders::new_assignment_home`].
fn new_home(shell: &Shell) -> usize {
    let names: Vec<String> = chain_paths(shell)
        .iter()
        .map(|path| file_name_of(path))
        .collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    shaders::new_assignment_home(&chain_docs(shell), &names)
}

/// Every event whose assignment resolves to `shader`, with the chain
/// index of the document holding it.
fn assignments_of(shell: &Shell, shader: &Path) -> Vec<(&'static str, usize)> {
    let docs = chain_docs(shell);
    let paths = chain_paths(shell);
    shaders::EVENTS
        .iter()
        .filter_map(|event| {
            let (value, doc) = shaders::current_assignment(&docs, event)?;
            shaders::same_file(&shaders::resolve(&value, &paths[doc]), shader)
                .then_some((*event, doc))
        })
        .collect()
}

/// Write assignment changes straight to their files (`None` clears).
/// Renaming or deleting a shader happens on disk at once, so the
/// assignments that follow it must too: left unsaved, a Discard would
/// point them at a file that no longer exists. Other unsaved edits in
/// those files stay unsaved.
fn write_assignments(
    shell: &mut Shell,
    edits: &[(&'static str, usize, Option<String>)],
) -> Result<(), String> {
    let paths = chain_paths(shell);
    for (event, doc, value) in edits {
        let key = ["animation", event, "shader"];
        doc_at_mut(shell, *doc)
            .write_through(&paths[*doc], |d| match value {
                Some(value) => d.set_string(&key, value),
                None => {
                    d.remove_leaf(&key);
                }
            })
            .map_err(|err| format!("could not update {}: {err}", paths[*doc].display()))?;
        // The written key's new on-disk value is its saved baseline. Only
        // that key: the rest of the baseline may hold values that aren't
        // on disk yet (the guided setup's suggestions) and must stay.
        let on_disk: ConfigDocument = doc_at(shell, *doc)
            .original_text()
            .parse()
            .map_err(|err| format!("{err}"))?;
        if let Some(slot) = shell.saved.get_mut(*doc) {
            rebase_saved_key(slot, &key.join("."), &on_disk);
        }
    }
    Ok(())
}

/// Set one key's saved baseline to its value in `on_disk` (dropping it
/// when the file no longer has it), leaving every other key alone.
fn rebase_saved_key(saved: &mut BTreeMap<String, String>, dotted: &str, on_disk: &ConfigDocument) {
    let repr = on_disk
        .leaf_values()
        .into_iter()
        .find_map(|(path, repr)| (path == dotted).then_some(repr));
    match repr {
        Some(repr) => {
            saved.insert(dotted.to_owned(), repr);
        }
        None => {
            saved.remove(dotted);
        }
    }
}

/// "Windows out, Overview" for the events assigned `shader`.
fn used_by(shell: &Shell, shader: &Path) -> String {
    assignments_of(shell, shader)
        .iter()
        .map(|(event, _)| prettify(event))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Delete one of the user's own shaders, then rescan. An editor open on
/// that same file closes with it, and assignments pointing at it are
/// cleared (unsaved) so no event is left on a missing file.
fn delete_shader(app: &AppWindow, shell: &Rc<RefCell<Shell>>, path: &Path) {
    // Found before the delete: matching needs the file on disk.
    let assigned = assignments_of(&shell.borrow(), path);
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
                let clears: Vec<_> = assigned
                    .iter()
                    .map(|(event, doc)| (*event, *doc, None))
                    .collect();
                if let Err(err) = write_assignments(&mut shell, &clears) {
                    app.set_status(err.into());
                }
                scan_shaders(&mut shell);
            }
            let shell = shell.borrow();
            app.set_dirty(shell.any_modified());
            rebuild_shaders(app, &shell);
            let status = if assigned.is_empty() {
                format!("Deleted {}.", path.display())
            } else {
                let events: Vec<String> =
                    assigned.iter().map(|(event, _)| prettify(event)).collect();
                format!(
                    "Deleted {} and cleared it from {}.",
                    path.display(),
                    events.join(", ")
                )
            };
            app.set_status(status.into());
        }
        Err(err) if app.get_shader_editor_open() => app.set_shader_editor_note(err.into()),
        Err(err) => app.set_status(err.into()),
    }
}

/// Show the editor overlay over whatever code is loaded, remembering it
/// as the unsaved-changes baseline.
fn show_editor(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    // A settle left over from the last session would re-check old code.
    shell.borrow().shader_code_settle.stop();
    shell.borrow_mut().shader_editor_baseline = app.get_shader_editor_text().to_string();
    shell.borrow_mut().shader_editor_baseline_name = app.get_shader_editor_name().to_string();
    app.set_shader_editor_confirm_close(false);
    app.set_shader_editor_open(true);
}

/// Park the scrubber mid-animation and hand the freshly loaded code to
/// the preview worker. Not the opening frame: at progress 0 a fade-in
/// is fully transparent and the preview looks broken.
fn kick_shader_preview(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    const START: f32 = 0.5;
    app.set_shader_preview_progress(START);
    app.set_shader_preview_note(String::new().into());
    let text = app.get_shader_editor_text().to_string();
    shell
        .borrow_mut()
        .preview_command(shader_preview::PreviewCommand::SetSource(text));
    // Preview as the event the shader is assigned to (the first, when
    // several), else Windows in.
    let event = {
        let shell = shell.borrow();
        shell
            .shader_editing
            .as_deref()
            .and_then(|path| {
                assignments_of(&shell, path)
                    .first()
                    .map(|(event, _)| *event)
            })
            .and_then(|event| shaders::EVENTS.iter().position(|e| *e == event))
            .unwrap_or(0)
    };
    set_preview_event(app, shell, event);
}

/// What an event's shader animates, for the stand-in frame.
fn preview_target(event: &str) -> shader_preview::Target {
    use shader_preview::Target;
    match event {
        "workspaces" => Target::Workspace,
        "overview" => Target::Overview,
        "scratchpad" => Target::Scratchpad,
        "layers" => Target::Layer,
        "border" => Target::Border,
        _ => Target::Window,
    }
}

/// Switch the preview to play as `index` (into `shaders::EVENTS`): its
/// stand-in, its timing from the config, and its natural direction.
fn set_preview_event(app: &AppWindow, shell: &Rc<RefCell<Shell>>, index: usize) {
    let Some(event) = shaders::EVENTS.get(index) else {
        return;
    };
    let timeline =
        umbriel_config::config::curves::event_timeline(&chain_docs(&shell.borrow()), event);
    {
        let mut shell = shell.borrow_mut();
        shell.shader_preview_event = index;
        shell.shader_preview_timeline = timeline;
        shell.preview_command(shader_preview::PreviewCommand::SetTarget(preview_target(
            event,
        )));
    }
    app.set_shader_preview_event(index as i32);
    app.set_shader_preview_length_ms(timeline.length_ms() as i32);
    // Closing is the only event umbriel runs purely outward.
    app.set_shader_preview_direction(if *event == "windows_out" { -1.0 } else { 1.0 });
    render_preview_at(app, shell, app.get_shader_preview_progress());
}

/// Render at timeline position `linear`, eased by the event's curve.
fn render_preview_at(app: &AppWindow, shell: &Rc<RefCell<Shell>>, linear: f32) {
    let linear = linear.clamp(0.0, 1.0);
    let mut shell = shell.borrow_mut();
    let eased = umbriel_config::config::curves::ease(
        shell.shader_preview_timeline.curve,
        f64::from(linear),
    ) as f32;
    shell.preview_command(shader_preview::PreviewCommand::Render {
        linear,
        eased,
        direction: app.get_shader_preview_direction(),
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
    // Resolve each event's assignment and each library file once; the
    // cards and rows below only compare these (no per-pair filesystem
    // lookups).
    let docs = chain_docs(shell);
    let paths = chain_paths(shell);
    let assigned: Vec<Option<(String, PathBuf, PathBuf)>> = shaders::EVENTS
        .iter()
        .map(|event| {
            shaders::current_assignment(&docs, event).map(|(value, doc)| {
                let resolved = shaders::resolve(&value, &paths[doc]);
                let key = shaders::file_key(&resolved);
                (value, resolved, key)
            })
        })
        .collect();
    let entry_keys: Vec<PathBuf> = shell
        .shaders
        .iter()
        .map(|entry| shaders::file_key(&entry.path))
        .collect();
    let infos: Vec<ShaderInfo> = shell
        .shaders
        .iter()
        .zip(&entry_keys)
        .map(|(entry, entry_key)| ShaderInfo {
            name: entry.name.clone().into(),
            value: entry.value.clone().into(),
            source: entry.source.label().into(),
            description: entry.description.clone().into(),
            invalid: entry.invalid.clone().unwrap_or_default().into(),
            path: entry.path.display().to_string().into(),
            is_own: entry.source == shaders::Source::ConfigDir,
            used_by: shaders::EVENTS
                .iter()
                .zip(&assigned)
                .filter(|(_, current)| current.as_ref().is_some_and(|(_, _, key)| key == entry_key))
                .map(|(event, _)| prettify(event))
                .collect::<Vec<_>>()
                .join(", ")
                .into(),
        })
        .collect();
    app.set_shaders(Rc::new(VecModel::from(infos)).into());

    let rows: Vec<ShaderAssignment> = shaders::EVENTS
        .iter()
        .zip(assigned)
        .map(|(event, current)| {
            let mut choices: Vec<SharedString> = vec!["(no shader)".into()];
            choices.extend(shell.shaders.iter().map(|entry| entry.label.clone().into()));
            // Match by the file umbriel would actually read, so
            // "./shaders/x.glsl" and an absolute path both find x.glsl.
            let index = current
                .as_ref()
                .and_then(|(_, _, key)| entry_keys.iter().position(|entry_key| entry_key == key));
            let warning = match &current {
                Some((value, resolved, _)) => {
                    shaders::assignment_problem(value, resolved).unwrap_or_default()
                }
                None => "",
            };
            let current = current.map(|(value, _, _)| value);
            // A value outside the library gets its own trailing entry, so
            // the dropdown shows it and "(no shader)" is a real change.
            let current_index = match (index, &current) {
                (Some(position), _) => position + 1,
                (None, Some(value)) => {
                    let label = if warning.is_empty() {
                        format!("{value} (outside the library)")
                    } else {
                        format!("⚠ {value} (missing)")
                    };
                    choices.push(label.into());
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
                warning: warning.into(),
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
                let shader = match usize::try_from(index) {
                    Ok(0) | Err(_) => None,
                    Ok(index) => shell.shaders.get(index - 1).map(|entry| entry.path.clone()),
                };
                match shader {
                    Some(shader) => assign_event(&mut shell, &event, Some(&shader)),
                    None if index == 0 => assign_event(&mut shell, &event, None),
                    None => {}
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
            // "reveal-fork", or "reveal-fork-2" when that's taken.
            let fork_name = match shell.borrow().path.parent() {
                Some(dir) => shaders::unused_shader_name(dir, &format!("{name}-fork")),
                None => format!("{name}-fork"),
            };
            open_shader_editor(&app, &shell, &path, false, fork_name);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        // Save's first step: which events should use this shader.
        app.on_shader_editor_save_request(move || {
            let Some(app) = weak.upgrade() else { return };
            let rows = use_for_rows(&shell.borrow());
            app.set_shader_use_for(Rc::new(VecModel::from(rows)).into());
            app.set_shader_use_open(true);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_editor_save(move || {
            let Some(app) = weak.upgrade() else { return };
            let text = app.get_shader_editor_text().to_string();
            // Every successful save closes the editor; creating only
            // changes the status wording.
            let creating = shell.borrow().shader_editing.is_none();
            let name = app.get_shader_editor_name().to_string();
            // Events to repoint when an edited shader is renamed.
            let mut renamed: Vec<(&'static str, usize)> = Vec::new();
            let result = {
                let shell = shell.borrow();
                match shell.shader_editing.clone() {
                    // Save the code first, then rename: a failed rename
                    // still leaves the edits on disk under the old name.
                    Some(path) => match shell.path.parent().map(Path::to_path_buf) {
                        Some(dir) => shaders::write_user_shader(&path, &text).and_then(|()| {
                            let assigned = assignments_of(&shell, &path);
                            let new = shaders::rename_user_shader(&dir, &path, &name)?;
                            if new != path {
                                renamed = assigned;
                            }
                            Ok(new)
                        }),
                        None => Err("could not determine the config directory.".to_owned()),
                    },
                    None => match shell.path.parent().map(Path::to_path_buf) {
                        Some(dir) => shaders::save_user_shader(&dir, &name, &text),
                        None => Err("could not determine the config directory.".to_owned()),
                    },
                }
            };
            match result {
                Ok(path) => {
                    // What the "Use for" step switched on or off.
                    let use_changes = {
                        let mut shell = shell.borrow_mut();
                        shell.shader_editing = Some(path.clone());
                        shell.shader_editor_baseline = text.clone();
                        shell.shader_editor_baseline_name = name.clone();
                        // A renamed shader keeps its assignments: each is
                        // rewritten (unsaved) to the new file, spelled for
                        // the document it lives in.
                        let paths = chain_paths(&shell);
                        let repoints: Vec<_> = renamed
                            .iter()
                            .map(|(event, doc)| {
                                (*event, *doc, Some(shaders::value_for(&path, &paths[*doc])))
                            })
                            .collect();
                        if let Err(err) = write_assignments(&mut shell, &repoints) {
                            app.set_status(err.into());
                        }
                        let changes = apply_use_for(&mut shell, &app, &path);
                        scan_shaders(&mut shell);
                        changes
                    };
                    let shell = shell.borrow();
                    app.set_dirty(shell.any_modified());
                    rebuild_shaders(&app, &shell);
                    app.set_shader_editor_note(String::new().into());
                    app.set_shader_editor_used_by(used_by(&shell, &path).into());
                    app.set_shader_editor_open(false);
                    let mut status = if creating {
                        format!("Created {}.", path.display())
                    } else if !renamed.is_empty() {
                        let events: Vec<String> =
                            renamed.iter().map(|(event, _)| prettify(event)).collect();
                        format!(
                            "Saved as {} and pointed {} at it.",
                            path.display(),
                            events.join(", ")
                        )
                    } else {
                        format!("Saved {} — umbriel live-reloads it.", path.display())
                    };
                    if !use_changes.is_empty() {
                        status.push_str(&format!(
                            " {}: save your config to apply.",
                            use_changes.join(", ")
                        ));
                    }
                    app.set_status(status.into());
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
        app.on_shader_editor_text_changed(move |_| {
            let Some(app) = weak.upgrade() else { return };
            // Restart the settle timer; the work runs once typing pauses.
            let weak = app.as_weak();
            let settle_shell = Rc::downgrade(&shell);
            shell.borrow().shader_code_settle.start(
                slint::TimerMode::SingleShot,
                CODE_SETTLE,
                move || {
                    if let (Some(app), Some(shell)) = (weak.upgrade(), settle_shell.upgrade()) {
                        code_settled(&app, &shell);
                    }
                },
            );
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
            let unsaved = {
                let shell = shell.borrow();
                app.get_shader_editor_text().as_str() != shell.shader_editor_baseline
                    || app.get_shader_editor_name().as_str() != shell.shader_editor_baseline_name
            };
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
            // A pending settle must not re-lock after the reset.
            shell.borrow().shader_code_settle.stop();
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
            settle_now(&app, &shell);
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
            settle_now(&app, &shell);
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
            settle_now(&app, &shell);
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
            settle_now(&app, &shell);
            if !app.get_shader_builder_locked() && set_step_param(&shell, index, param, value) {
                regen_builder(&app, &shell);
            }
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_step_param_live(move |index, param, value| {
            let Some(app) = weak.upgrade() else { return };
            settle_now(&app, &shell);
            if !app.get_shader_builder_locked() && set_step_param(&shell, index, param, value) {
                regen_code(&app, &shell);
            }
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_preview_scrub(move |progress| {
            let Some(app) = weak.upgrade() else { return };
            render_preview_at(&app, &shell, progress);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_preview_event_picked(move |index| {
            let Some(app) = weak.upgrade() else { return };
            set_preview_event(&app, &shell, index.max(0) as usize);
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
            render_preview_at(&app, &shell, app.get_shader_preview_progress());
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn rebasing_one_key_leaves_the_rest_of_the_baseline() {
        // A guide suggestion sits in the baseline but not on disk.
        let mut saved = BTreeMap::from([
            ("general.xwayland".to_owned(), "true".to_owned()),
            (
                "animation.windows_out.shader".to_owned(),
                "\"shaders/old.glsl\"".to_owned(),
            ),
        ]);
        let on_disk =
            ConfigDocument::from_str("[animation.windows_out]\nshader = \"shaders/new.glsl\"\n")
                .unwrap();
        rebase_saved_key(&mut saved, "animation.windows_out.shader", &on_disk);
        assert_eq!(
            saved["animation.windows_out.shader"],
            "\"shaders/new.glsl\""
        );
        assert_eq!(saved["general.xwayland"], "true", "untouched");
        // A key the file no longer has leaves the baseline.
        let cleared = ConfigDocument::from_str("").unwrap();
        rebase_saved_key(&mut saved, "animation.windows_out.shader", &cleared);
        assert!(!saved.contains_key("animation.windows_out.shader"));
        assert_eq!(saved.len(), 1);
    }
}
