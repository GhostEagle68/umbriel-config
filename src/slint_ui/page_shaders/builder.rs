//! The effect builder: the step cards, their parameters, and regenerating the
//! code pane from the stack (or following the code back into it).

use super::super::*;
use super::editor::*;

/// One step card's data. Fixed-shape struct: up to three parameter
/// slots, padded.
pub(super) fn step_row(index: usize, step: &shaders::builder::BuilderStep) -> Option<ShaderStep> {
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
}

/// Show the builder stack as step cards. Rows only: the code pane is
/// the caller's business.
pub(super) fn push_builder_rows(app: &AppWindow, steps: &[shaders::builder::BuilderStep]) {
    let rows: Vec<ShaderStep> = steps
        .iter()
        .enumerate()
        .filter_map(|(index, step)| step_row(index, step))
        .collect();
    app.set_shader_steps(Rc::new(VecModel::from(rows)).into());
}

/// Refresh one step card's data in place. Unlike replacing the model,
/// this keeps the card's widgets, so a focused slider stays focused.
pub(super) fn refresh_step_row(app: &AppWindow, shell: &Rc<RefCell<Shell>>, index: usize) {
    let Some(row) = shell
        .borrow()
        .builder_steps
        .get(index)
        .and_then(|step| step_row(index, step))
    else {
        return;
    };
    let model = app.get_shader_steps();
    if let Some(rows) = model.as_any().downcast_ref::<VecModel<ShaderStep>>()
        && index < rows.row_count()
    {
        rows.set_row_data(index, row);
    }
}

/// Push the builder stack into the step panel and regenerate the code
/// pane from it. Callers only reach this with the builder unlocked, so
/// the code being replaced is itself builder output.
pub(super) fn regen_builder(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    let steps = shell.borrow().builder_steps.clone();
    push_builder_rows(app, &steps);
    app.set_shader_builder_locked(false);
    regen_code(app, shell);
}

/// Regenerate the code pane (and the preview's source) from the builder
/// stack, leaving the step cards alone: a slider mid-drag must not be
/// recreated under the pointer.
pub(super) fn regen_code(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    regen_code_grouped(app, shell, false);
}

/// `regen_code`, recording the change as an undo step; `grouped` merges
/// it with a change moments ago (a slider drag's many updates).
pub(super) fn regen_code_grouped(app: &AppWindow, shell: &Rc<RefCell<Shell>>, grouped: bool) {
    let code = shaders::builder::generate_stack(&shell.borrow().builder_steps);
    let end = code.len();
    shell.borrow_mut().shader_code_history.record(
        shaders::code_edit::Snapshot {
            text: code.clone(),
            anchor: end,
            cursor: end,
        },
        grouped,
        std::time::Instant::now(),
    );
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
pub(super) fn set_step_param(
    shell: &Rc<RefCell<Shell>>,
    index: i32,
    param: i32,
    value: f32,
) -> bool {
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
pub(super) fn sync_builder_from_code(app: &AppWindow, shell: &Rc<RefCell<Shell>>, code: &str) {
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

pub(super) fn install(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
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
        // Slider released (mouse up, or an arrow key's release): the
        // live path already regenerated the code, so settle the value and
        // refresh just this card; rebuilding every card would destroy the
        // slider and its keyboard focus after each arrow press.
        app.on_shader_step_param(move |index, param, value| {
            let Some(app) = weak.upgrade() else { return };
            settle_now(&app, &shell);
            if !app.get_shader_builder_locked() && set_step_param(&shell, index, param, value) {
                regen_code(&app, &shell);
                refresh_step_row(&app, &shell, index as usize);
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
                regen_code_grouped(&app, &shell, true);
            }
        });
    }
}
