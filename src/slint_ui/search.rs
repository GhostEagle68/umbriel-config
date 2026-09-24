//! The header search: settings, whole pages, and keybind chords in one
//! result list.

use super::common::*;
use super::*;

/// Case-insensitive all-terms match — the same rule the settings search
/// applies to keys and labels, used for page titles/descriptions and
/// keybind chords/actions.
fn terms_match(haystack: &str, needle: &str) -> bool {
    let lowered = haystack.to_lowercase();
    needle
        .to_lowercase()
        .split_whitespace()
        .all(|term| lowered.contains(term))
}

pub(super) fn install_search(
    app: &AppWindow,
    shell: &Rc<RefCell<Shell>>,
    kb_actions: &Arc<Mutex<Vec<keybinds::LiveAction>>>,
    keybind_view: &Rc<RefCell<super::page_keybinds::KeybindView>>,
) {
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        let kb_actions = Arc::clone(kb_actions);
        app.on_search_edited(move |query| {
            let Some(app) = weak.upgrade() else { return };
            let shell = shell.borrow();
            let needle = query.trim().to_owned();
            if needle.is_empty() {
                // Empty box = leave search; restore the section overview.
                let section = app.get_current_section().to_string();
                app.set_page(Page::Section);
                let (title, description) = super::sections::page_meta(&section);
                app.set_page_title(title.into());
                app.set_page_description(description.into());
                super::sections::refill_page(&app, &shell, &section);
                return;
            }
            let sets = chain_path_sets(&shell);
            let labels = setting_labels(&shell);
            let mut rows: Vec<SectionRow> = Vec::new();

            // Whole pages first: the query names a destination, not a key.
            for page in catalog::PAGES {
                if terms_match(&format!("{} {}", page.title, page.description), &needle) {
                    rows.push(SectionRow {
                        label: page.title.into(),
                        home_label: "Page".into(),
                        home_section: page.id.into(),
                        kind: "page".into(),
                    });
                }
            }

            // Keybinds: match the chord or what the action does. A hit
            // opens the keybinds page pre-filtered to the query.
            let actions = kb_actions.lock().expect("kb actions").clone();
            let docs = chain_docs(&shell);
            for bind in keybinds::merged_binds(&docs) {
                let summary = keybinds::describe(&bind.action, &actions);
                if terms_match(&format!("{} {}", bind.chord, summary), &needle) {
                    let extras = super::page_keybinds::keybind_extras(&bind);
                    let label = if extras.is_empty() {
                        format!("{} — {summary}", bind.chord)
                    } else {
                        format!("{} — {summary} ({extras})", bind.chord)
                    };
                    rows.push(SectionRow {
                        label: label.into(),
                        home_label: "Keybind".into(),
                        home_section: "keybinds".into(),
                        kind: "keybind".into(),
                    });
                }
            }

            rows.extend(
                shell
                    .schema
                    .iter()
                    .filter(|entry| schema::matches(entry, &needle))
                    .map(|entry| {
                        let dotted = entry.path.join(".");
                        let home = entry_home(&sets, &dotted).unwrap_or(sets.len() - 1);
                        SectionRow {
                            label: entry.label.clone().into(),
                            home_label: labels.get(home).cloned().unwrap_or_default(),
                            home_section: super::sections::page_id_for_section(&entry.section)
                                .into(),
                            kind: String::new().into(),
                        }
                    }),
            );
            app.set_search_rows(Rc::new(VecModel::from(rows)).into());
            app.set_page(Page::Search);
            app.set_page_title(SharedString::from(needle));
        });
    }
    {
        let weak = app.as_weak();
        let keybind_view = Rc::clone(keybind_view);
        let kb_actions = Arc::clone(kb_actions);
        let shell = Rc::clone(shell);
        // A keybind search hit: land on the keybinds page with its
        // filter pre-filled so the chord is right there.
        app.on_keybinds_search_requested(move |filter| {
            let Some(app) = weak.upgrade() else { return };
            app.global::<KeybindsState>().set_search(filter);
            keybind_view.borrow_mut().exact = false;
            app.set_current_section("keybinds".into());
            app.set_page(Page::Keybinds);
            let shell = shell.borrow();
            super::page_keybinds::rebuild_keybind_rows(&app, &shell, &kb_actions, &keybind_view);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_match_is_case_insensitive_all_terms() {
        assert!(terms_match(
            "Window rules — How matching windows open",
            "rules windOw"
        ));
        assert!(terms_match("Keybinds", "keybinds"));
        assert!(!terms_match("Window rules", "window banana"));
        // Blank needle matches anything, mirroring the settings search.
        assert!(terms_match("Anything", "   "));
    }

    #[test]
    fn catalog_pages_are_findable_by_their_titles() {
        let keybinds = catalog::PAGES.iter().find(|p| p.id == "keybinds").unwrap();
        let outputs = catalog::PAGES
            .iter()
            .find(|p| p.id == catalog::OUTPUTS_ID)
            .unwrap();
        assert!(terms_match(keybinds.title, "keybind"));
        assert!(terms_match(
            &format!("{} {}", outputs.title, outputs.description),
            "monitors"
        ));
    }
}
