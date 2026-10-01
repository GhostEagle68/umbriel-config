//! The editor's settings strip: the preset's parameters (`palette`,
//! `speed`, `light.*`, ...) for the shader being edited. They belong to
//! the shader's own preset file, so they are held here while editing and
//! written with the shader on Save (and dropped by Back with the rest of
//! the unsaved changes). The rows come from `ShaderApi`, so a newer
//! Umbriel's keys show up on their own.

use super::super::common::*;
use super::super::*;
use super::library::*;
use shaders::params::{self as preset_params, Control, LIGHT};

/// What the strip shows for the shader being edited, by the kind the
/// editor currently has.
fn editor_kind(app: &AppWindow) -> &'static str {
    usize::try_from(app.get_shader_editor_kind())
        .ok()
        .and_then(|index| shaders::KINDS.get(index).copied())
        .unwrap_or(shaders::KINDS[0])
}

/// Read the parameters of the shader just opened from its preset file;
/// a shader with no preset file yet has none set.
pub(super) fn load_params(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    {
        let mut shell = shell.borrow_mut();
        let entry = shell
            .shader_editing
            .as_ref()
            .and_then(|path| shell.shaders.iter().find(|entry| entry.path == *path));
        let (values, locked) = entry
            .and_then(|entry| {
                let text = std::fs::read_to_string(&entry.preset_file).ok()?;
                Some((
                    preset_params::read(&text, &entry.preset),
                    !preset_params::app_written(&text),
                ))
            })
            .unwrap_or_default();
        shell.shader_params_baseline = values.clone();
        shell.shader_params = values;
        shell.shader_params_locked = locked;
    }
    refresh_params(app, shell);
}

/// Whether the parameters differ from what the preset file has.
pub(super) fn params_unsaved(shell: &Shell) -> bool {
    shell.shader_params != shell.shader_params_baseline
}

/// Rebuild the strip's rows for the editor's current kind.
pub(super) fn refresh_params(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    let shell = shell.borrow();
    let fields = preset_params::fields(&shell.shader_api, editor_kind(app));
    let overlays: Vec<String> = std::iter::once("none".to_owned())
        .chain(
            shaders::presets(&chain_docs(&shell), &data_roots(), "window")
                .into_iter()
                .map(|(name, _)| name),
        )
        .collect();
    let light_on = shell.shader_params.get(LIGHT).is_some_and(|v| v == "true");
    let rows: Vec<ParamRow> = fields
        .iter()
        .map(|field| {
            let value = shell
                .shader_params
                .get(&field.key)
                .map_or(field.default.as_str(), String::as_str);
            let (control, min, max, decimals) = match field.control {
                Control::Switch => (0, 0.0, 1.0, 0),
                Control::Number { min, max, decimals } => (1, min, max, decimals),
                Control::Overlay => (2, 0.0, 1.0, 0),
            };
            let grouped = field.key.starts_with("light.");
            ParamRow {
                key: field.key.clone().into(),
                label: field.label.clone().into(),
                description: field.description.clone().into(),
                control,
                on: value == "true",
                number: value.parse().unwrap_or(0.0),
                min: min as f32,
                max: max as f32,
                decimals: decimals as i32,
                choice: overlays.iter().position(|name| name == value).unwrap_or(0) as i32,
                indent: grouped,
                shown: !grouped || light_on,
            }
        })
        .collect();
    let overlays: Vec<slint::SharedString> = overlays.into_iter().map(Into::into).collect();
    app.set_shader_param_overlays(Rc::new(VecModel::from(overlays)).into());
    app.set_shader_params_locked(shell.shader_params_locked);
    app.set_shader_params(Rc::new(VecModel::from(rows)).into());
}

/// Write the parameters into `entry`'s preset file: created when there is
/// something to say, and only ever a file the app wrote. Keys of another
/// kind (an edit changed the shader's kind) go too, as Umbriel reports
/// them unknown.
pub(super) fn save_params(
    shell: &Shell,
    entry: &shaders::ShaderEntry,
    kind: &str,
) -> Result<(), String> {
    if shell.shader_params_locked {
        return Ok(());
    }
    if !shell.shader_params.is_empty() {
        shaders::ensure_preset_file(entry)?;
    }
    let Ok(text) = std::fs::read_to_string(&entry.preset_file) else {
        return Ok(());
    };
    if !preset_params::app_written(&text) {
        return Ok(());
    }
    let updated = preset_params::apply(
        &text,
        &entry.preset,
        kind,
        &shell.shader_params,
        &shell.shader_api,
    )?;
    if updated != text {
        shaders::write_file_atomic(&entry.preset_file, &updated)?;
    }
    Ok(())
}

pub(super) fn install(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    let weak = app.as_weak();
    let shell = Rc::clone(shell);
    app.on_shader_param_set(move |key, value| {
        let Some(app) = weak.upgrade() else { return };
        {
            let mut shell = shell.borrow_mut();
            let Some(field) = preset_params::fields(&shell.shader_api, editor_kind(&app))
                .into_iter()
                .find(|field| field.key == key.as_str())
            else {
                return;
            };
            if shell.shader_params_locked {
                return;
            }
            // A value back at its default is simply not set.
            if field.is_default(&value) {
                shell.shader_params.remove(key.as_str());
            } else {
                shell
                    .shader_params
                    .insert(key.to_string(), value.to_string());
            }
            if key == LIGHT && value != "true" {
                shell
                    .shader_params
                    .retain(|name, _| !name.starts_with("light."));
            }
        }
        refresh_params(&app, &shell);
    });
}
