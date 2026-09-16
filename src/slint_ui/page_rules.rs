//! The rule pages: window rules, layer rules, security contexts — one
//! family per page, every chain document's rules editable in place.

use super::common::*;
use super::page_outputs::output_names;
use super::*;

/// Which rule family a section page edits, if any.
pub(super) fn rule_family(section: &str) -> Option<&'static str> {
    match section {
        "window-rules" => Some("window_rule"),
        "layer-rules" => Some("layer_rule"),
        "security-contexts" => Some("security_context_rule"),
        _ => None,
    }
}

/// The chain document that owns a rule family: the first include with
/// rules of that family in it, else the main config.
pub(super) fn rule_target(shell: &Shell, family: &str) -> usize {
    let main = shell.includes.docs.len();
    shell
        .includes
        .docs
        .iter()
        .position(|inc| inc.doc.rule_count(family) > 0)
        .unwrap_or(main)
}

/// One rule field as a settings row. Rules have no defaults: unset
/// fields show blank, choices offer "(unset)", and only what the user
/// fills in is written. The key carries the chain document so edits
/// land where the rule lives; the badge opens that file.
fn rule_row(
    shell: &Shell,
    doc_index: usize,
    family: &str,
    index: usize,
    field: &rules::Field,
) -> SettingRow {
    let doc = doc_at(shell, doc_index);
    let mut row = blank_row(
        rules::rule_key(family, doc_index, index, field.key),
        field.label,
        doc_index as i32,
    );
    row.home_label = setting_labels(shell)
        .get(doc_index)
        .cloned()
        .unwrap_or_default();
    let text = rules::field_text(doc, family, index, field);
    match &field.kind {
        rules::FieldKind::Text
        | rules::FieldKind::List
        | rules::FieldKind::SizePx
        | rules::FieldKind::SizeFraction
        | rules::FieldKind::Position => {
            row.value = text.into();
        }
        rules::FieldKind::Toggle => {
            row.kind = ValueKind::Boolean;
            row.checked = text == "true";
            row.value = text.into();
        }
        rules::FieldKind::Choice(options) => {
            row.kind = ValueKind::Choice;
            let mut choices: Vec<SharedString> = vec!["(unset)".into()];
            choices.extend(options.iter().map(|option| (*option).into()));
            row.choices = Rc::new(VecModel::from(choices)).into();
            row.value = if text.is_empty() {
                "(unset)".into()
            } else {
                text.into()
            };
        }
        rules::FieldKind::OutputChoice => {
            row.kind = ValueKind::OpenChoice;
            row.choices = Rc::new(VecModel::from(
                output_names(shell)
                    .into_iter()
                    .map(SharedString::from)
                    .collect::<Vec<_>>(),
            ))
            .into();
            row.value = text.into();
        }
        rules::FieldKind::Float { min, max } => {
            row.kind = ValueKind::Float;
            row.min = *min as f32;
            row.max = *max as f32;
            row.value = if text.is_empty() {
                min.to_string().into()
            } else {
                text.into()
            };
        }
        rules::FieldKind::Integer { min, max } => {
            row.kind = ValueKind::Integer;
            row.min = *min as f32;
            row.max = *max as f32;
            row.value = if text.is_empty() {
                min.to_string().into()
            } else {
                text.into()
            };
        }
    }
    row
}

/// The rule page model: every chain document's rules of the family,
/// includes first then the main config — all editable in place.
fn rule_cards(shell: &Shell, family: &str) -> Vec<RuleCard> {
    let main = shell.includes.docs.len();
    let labels = setting_labels(shell);
    let (match_fields, setting_fields) = rules::fields(family);
    let mut cards: Vec<RuleCard> = Vec::new();
    for doc_index in 0..=main {
        let doc = doc_at(shell, doc_index);
        for index in 0..doc.rule_count(family) {
            let rows = |fields: &[rules::Field]| {
                fields
                    .iter()
                    .map(|field| rule_row(shell, doc_index, family, index, field))
                    .collect::<Vec<_>>()
            };
            cards.push(RuleCard {
                title: rules::rule_title(doc, family, index, match_fields).into(),
                index: index as i32,
                file: doc_index as i32,
                file_label: labels.get(doc_index).cloned().unwrap_or_default(),
                expanded: false,
                match_rows: Rc::new(VecModel::from(rows(match_fields))).into(),
                setting_rows: Rc::new(VecModel::from(rows(setting_fields))).into(),
            });
        }
    }
    // A lone rule opens by default; otherwise the titles read like a
    // collapsed list until a card is expanded.
    let default = cards.len() == 1;
    let family_key = family.to_owned();
    for (position, card) in cards.iter_mut().enumerate() {
        let key = format!("rule:{family_key}:{}:{}", card.file, card.index);
        card.expanded = card_expanded(shell, &key, default && position == 0);
    }
    cards
}

/// New rules join the file where that family already has the most
/// rules — where the user keeps them; a tie (or no rules anywhere)
/// goes to the main config.
fn rule_add_target(shell: &Shell, family: &str) -> usize {
    let main = shell.includes.docs.len();
    (0..=main)
        .max_by_key(|doc_index| doc_at(shell, *doc_index).rule_count(family))
        .unwrap_or(main)
}

pub(super) fn rebuild_rule_page(app: &AppWindow, shell: &Shell) {
    let section = app.get_current_section().to_string();
    let Some(family) = rule_family(&section) else {
        return;
    };
    app.set_rule_cards(Rc::new(VecModel::from(rule_cards(shell, family))).into());
    app.set_changed_count(changed_count(shell));
}

pub(super) fn install_rules(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_rule_add(move || {
            let Some(app) = weak.upgrade() else { return };
            let section = app.get_current_section().to_string();
            let Some(family) = rule_family(&section) else {
                return;
            };
            let target = rule_add_target(&shell.borrow(), family);
            doc_at_mut(&mut shell.borrow_mut(), target).add_rule(family);
            // The new rule sits at the end of its file; open it so the
            // fields are right there to fill in.
            let new_index = doc_at(&shell.borrow(), target).rule_count(family) - 1;
            shell
                .borrow_mut()
                .card_expanded
                .insert(format!("rule:{family}:{target}:{new_index}"), true);
            let shell = shell.borrow();
            app.set_dirty(shell.any_modified());
            rebuild_rule_page(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_rule_remove(move |file, index| {
            let Some(app) = weak.upgrade() else { return };
            let section = app.get_current_section().to_string();
            let Some(family) = rule_family(&section) else {
                return;
            };
            doc_at_mut(&mut shell.borrow_mut(), file as usize).remove_rule(family, index as usize);
            let shell = shell.borrow();
            app.set_dirty(shell.any_modified());
            rebuild_rule_page(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_toggle_rule(move |file, index| {
            let Some(app) = weak.upgrade() else { return };
            let Some(family) = rule_family(&app.get_current_section()) else {
                return;
            };
            let key = format!("rule:{family}:{file}:{index}");
            // The default (open when alone) depends on the card count,
            // so flip whatever the model shows right now.
            let open = {
                let shell = shell.borrow();
                rule_cards(&shell, family)
                    .into_iter()
                    .find(|card| card.file == file && card.index == index)
                    .map(|card| !card.expanded)
            };
            let Some(open) = open else { return };
            shell.borrow_mut().card_expanded.insert(key, open);
            let shell = shell.borrow();
            rebuild_rule_page(&app, &shell);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_card_expansion_defaults_and_persists() {
        let dir = std::env::temp_dir().join(format!("umbriel-expand-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let main_path = dir.join("config.toml");
        std::fs::write(&main_path, "[[window_rule]]\nmatch.app_id = \"kitty\"\n").unwrap();
        let env = discovery::Env::from_process();

        // A lone rule card opens by default.
        let mut shell = Shell::load(&main_path, &env);
        assert!(rule_cards(&shell, "window_rule")[0].expanded);

        // A second rule collapses the list — and the user's choice
        // survives the rebuild that every edit triggers.
        shell.doc.add_rule("window_rule");
        let cards = rule_cards(&shell, "window_rule");
        assert!(cards.iter().all(|card| !card.expanded));
        let key = format!("rule:window_rule:{}:{}", cards[0].file, cards[0].index);
        shell.card_expanded.insert(key, true);
        assert!(rule_cards(&shell, "window_rule")[0].expanded);
        assert!(!rule_cards(&shell, "window_rule")[1].expanded);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rule_cards_collect_every_chain_document() {
        let dir = std::env::temp_dir().join(format!("umbriel-rules-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let main_path = dir.join("config.toml");
        let inc_path = dir.join("windowrules.toml");
        std::fs::write(
            &inc_path,
            "[[window_rule]]\nmatch.app_id = \"steam\"\n\n[[window_rule]]\nmatch.app_id = \"discord\"\n",
        )
        .unwrap();
        std::fs::write(
            &main_path,
            "[include]\nfiles = [\"windowrules.toml\"]\n\n[[window_rule]]\ndefault_floating = true\n",
        )
        .unwrap();
        let env = discovery::Env::from_process();
        let shell = Shell::load(&main_path, &env);
        let cards = rule_cards(&shell, "window_rule");
        // Include rules first, then the main config's own rule.
        assert_eq!(cards.len(), 3);
        assert_eq!(cards[0].title, "app_id = steam");
        assert_eq!(cards[0].file, 0);
        assert_eq!(cards[2].file, 1);
        assert!(cards[2].file_label.contains("config.toml"));
        // Editing a card's row writes through the doc-qualified key.
        let key = rules::rule_key("window_rule", 0, 0, "default_floating");
        assert_eq!(key, "window_rule[0:0].default_floating");
        assert_eq!(
            rules::parse_rule_key(&key),
            Some(("window_rule", Some(0), 0, "default_floating"))
        );
        // New rules join the file holding the most rules of the family.
        assert_eq!(rule_add_target(&shell, "window_rule"), 0);
        std::fs::remove_dir_all(&dir).ok();
    }
}
