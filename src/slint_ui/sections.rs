//! The section pages: cards assembled from the catalog + schema, the
//! sidebar model, and the page navigation spine.

use super::common::*;
use super::rows::{schema_row, strip_decor};
use super::*;

/// Refill the section page: every setting of `section` across the chain.
pub(super) fn refill_page(app: &AppWindow, shell: &Shell, page_id: &str) {
    app.set_cards(Rc::new(VecModel::from(page_cards(shell, page_id))).into());
    app.set_changed_count(changed_count(shell));
}

/// Redraw whichever page is showing from the documents, plus the Save
/// button's state. For bulk changes (discard, per-key reset, save with
/// moves) that can touch any page's values, not just settings cards.
pub(super) fn refresh_shown_page(app: &AppWindow, shell: &Shell) {
    match app.get_page() {
        Page::Shaders => super::page_shaders::rebuild_shaders(app, shell),
        Page::Outputs => super::page_outputs::rebuild_outputs(app, shell),
        Page::Rules => super::page_rules::rebuild_rule_page(app, shell),
        Page::Keybinds => {
            // Its rows live outside the shell; rerun the page's own search
            // handler once the caller's borrow of the shell has ended.
            let weak = app.as_weak();
            slint::Timer::single_shot(std::time::Duration::ZERO, move || {
                if let Some(app) = weak.upgrade() {
                    let state = app.global::<KeybindsState>();
                    state.invoke_search_edited(state.get_search());
                }
            });
        }
        _ => refill_page(app, shell, app.get_current_section().as_str()),
    }
    app.set_dirty(shell.any_modified());
    app.set_changed_count(changed_count(shell));
}

/// Which page a schema sub-section belongs to: its claimed page, else the
/// first page covering its top-level area, else the MORE fallback page
/// (whose id is the top-level name itself).
pub(super) fn page_id_for_section(section: &str) -> String {
    if let Some((_, id)) = catalog::claimed_sections().find(|(claimed, _)| *claimed == section) {
        return (*id).to_owned();
    }
    let top = section.split('.').next().unwrap_or(section);
    if let Some((_, id)) =
        catalog::claimed_sections().find(|(claimed, _)| claimed.split('.').next() == Some(top))
    {
        return (*id).to_owned();
    }
    top.to_owned()
}

/// NEW-key counts per page id (sidebar badges).
fn page_new_counts(shell: &Shell) -> BTreeMap<String, i32> {
    let mut counts: BTreeMap<String, i32> = BTreeMap::new();
    for key in &shell.new_keys {
        let page_id = if key.starts_with("output.") {
            catalog::OUTPUTS_ID.to_owned()
        } else {
            match shell
                .schema
                .iter()
                .find(|entry| entry.dotted() == *key)
                .map(|entry| page_id_for_section(&entry.section))
            {
                Some(page_id) => page_id,
                None => continue,
            }
        };
        *counts.entry(page_id).or_default() += 1;
    }
    counts
}

/// Header title + subtitle for a page.
pub(super) fn page_meta(page_id: &str) -> (String, String) {
    match catalog::page(page_id) {
        Some(page) => (page.title.to_owned(), page.description.to_owned()),
        None => (
            prettify(page_id),
            "Settings umbriel hasn't grouped yet, the catalog will catch up.".to_owned(),
        ),
    }
}

/// The sidebar model: grouped human pages (catalog + MORE fallback) with
/// small header rows and NEW counts per page.
pub(super) fn section_nav(shell: &Shell) -> Vec<SectionNav> {
    fn push_page(
        nav: &mut Vec<SectionNav>,
        claimed_tops: &mut Vec<&'static str>,
        counts: &BTreeMap<String, i32>,
        page: &'static catalog::Page,
    ) {
        for top in catalog::page_top_levels(page) {
            if !claimed_tops.contains(&top) {
                claimed_tops.push(top);
            }
        }
        nav.push(SectionNav {
            label: page.title.into(),
            id: page.id.into(),
            new_count: counts.get(page.id).copied().unwrap_or(0),
            is_header: false,
        });
    }

    let counts = page_new_counts(shell);
    let mut nav: Vec<SectionNav> = Vec::new();
    let mut claimed_tops: Vec<&'static str> = Vec::new();
    // Outputs leads the sidebar and needs no redundant group header.
    if let Some(page) = catalog::page(catalog::OUTPUTS_ID) {
        push_page(&mut nav, &mut claimed_tops, &counts, page);
    }
    for group in catalog::GROUPS.iter().filter(|group| **group != "outputs") {
        nav.push(SectionNav {
            label: catalog::group_title(group).to_uppercase().into(),
            id: String::new().into(),
            new_count: 0,
            is_header: true,
        });
        for page in catalog::PAGES.iter().filter(|page| page.group == *group) {
            push_page(&mut nav, &mut claimed_tops, &counts, page);
        }
    }
    // Top-level areas the catalog doesn't claim surface under MORE.
    let more: Vec<SharedString> = section_names(&shell.schema)
        .into_iter()
        .filter(|name| !claimed_tops.contains(&name.as_str()))
        .collect();
    if !more.is_empty() {
        nav.push(SectionNav {
            label: catalog::group_title(catalog::MORE_GROUP)
                .to_uppercase()
                .into(),
            id: String::new().into(),
            new_count: 0,
            is_header: true,
        });
        for name in more {
            nav.push(SectionNav {
                new_count: counts.get(name.as_str()).copied().unwrap_or(0),
                label: prettify(&name).into(),
                id: name.to_string().into(),
                is_header: false,
            });
        }
    }
    nav
}

/// One settings page's cards.
pub(super) fn page_cards(shell: &Shell, page_id: &str) -> Vec<SettingsCard> {
    if page_id == catalog::OUTPUTS_ID {
        return super::page_outputs::output_cards(shell);
    }
    if let Some(page) = catalog::page(page_id) {
        return catalog_page_cards(shell, page);
    }
    fallback_page_cards(shell, page_id)
}

/// A catalog page: one card per curated sub-section, then any
/// sub-sections of the same areas the catalog doesn't claim yet, then
/// the uncovered-keys card.
fn catalog_page_cards(shell: &Shell, page: &catalog::Page) -> Vec<SettingsCard> {
    let sets = chain_path_sets(shell);
    let labels = setting_labels(shell);
    let main = shell.includes.docs.len();
    let current: Vec<BTreeMap<String, String>> = (0..=main)
        .map(|i| doc_at(shell, i).leaf_values().into_iter().collect())
        .collect();

    let mut cards: Vec<SettingsCard> = Vec::new();
    let mut claimed: Vec<&str> = Vec::new();
    for card in page.cards {
        claimed.push(card.section);
        let rows: Vec<SettingRow> = shell
            .schema
            .iter()
            // Shader paths belong to the Shaders page's assignment UI.
            .filter(|entry| {
                entry.section == card.section
                    && !(entry.section.starts_with("animation.")
                        && entry.path.last() == Some(&"shader".to_owned()))
            })
            .map(|entry| schema_row(shell, &sets, &labels, &current, entry))
            .collect();
        if !rows.is_empty() {
            let key = format!("card:{}:{}", page.id, card.title);
            cards.push(SettingsCard {
                title: card.title.into(),
                key: key.clone().into(),
                expanded: card_expanded(shell, &key, true),
                rows: Rc::new(VecModel::from(rows)).into(),
            });
        }
    }

    let tops = catalog::page_top_levels(page);
    let mut auto: BTreeMap<String, Vec<SettingRow>> = BTreeMap::new();
    for entry in &shell.schema {
        let top = entry.section.split('.').next().unwrap_or("");
        if !tops.contains(&top) || claimed.contains(&entry.section.as_str()) {
            continue;
        }
        let row = schema_row(shell, &sets, &labels, &current, entry);
        auto.entry(prettify(&entry.section)).or_default().push(row);
    }
    for (title, rows) in auto {
        let key = format!("card:{}:{title}", page.id);
        cards.push(SettingsCard {
            key: key.clone().into(),
            expanded: card_expanded(shell, &key, true),
            title: title.into(),
            rows: Rc::new(VecModel::from(rows)).into(),
        });
    }
    if let Some(other) = other_card(shell, &sets, &labels, &current, &tops) {
        cards.push(other);
    }
    cards
}

/// A MORE fallback page: every sub-section of one top-level area.
fn fallback_page_cards(shell: &Shell, top: &str) -> Vec<SettingsCard> {
    let sets = chain_path_sets(shell);
    let labels = setting_labels(shell);
    let main = shell.includes.docs.len();
    let current: Vec<BTreeMap<String, String>> = (0..=main)
        .map(|i| doc_at(shell, i).leaf_values().into_iter().collect())
        .collect();

    let mut cards: BTreeMap<String, Vec<SettingRow>> = BTreeMap::new();
    for entry in &shell.schema {
        if entry.section.split('.').next() != Some(top) {
            continue;
        }
        let row = schema_row(shell, &sets, &labels, &current, entry);
        cards.entry(prettify(&entry.section)).or_default().push(row);
    }
    let mut out: Vec<SettingsCard> = cards
        .into_iter()
        .map(|(title, rows)| SettingsCard {
            key: title.clone().into(),
            expanded: true,
            title: title.into(),
            rows: Rc::new(VecModel::from(rows)).into(),
        })
        .collect();
    if let Some(other) = other_card(shell, &sets, &labels, &current, &[top]) {
        out.push(other);
    }
    out
}

/// Keys beyond the schema fold onto the page that owns their area: one
/// read-only row per key, owned like any other row. Only keys whose
/// top-level matches one of `tops` are shown.
fn other_card(
    shell: &Shell,
    sets: &[BTreeSet<String>],
    labels: &[SharedString],
    current: &[BTreeMap<String, String>],
    tops: &[&str],
) -> Option<SettingsCard> {
    let docs: Vec<&ConfigDocument> = shell.includes.docs.iter().map(|inc| &inc.doc).collect();
    let claims = schema::managed_claims(&docs);
    let schema_keys = schema::key_set(&shell.schema);
    let mut other: BTreeMap<String, SettingRow> = BTreeMap::new();
    for (i, set) in sets.iter().enumerate() {
        let paths: Vec<String> = set.iter().cloned().collect();
        for path in schema::uncovered(&paths, &schema_keys, &claims) {
            let top = path.split('.').next().unwrap_or_default();
            if !tops.contains(&top) {
                continue;
            }
            // One row per key: the file that owns it (shadowed copies
            // in later includes are skipped).
            let Some(home) = entry_home(sets, &path) else {
                continue;
            };
            if home != i || other.contains_key(&path) {
                continue;
            }
            let value = current
                .get(home)
                .and_then(|values| values.get(&path))
                .map(|raw| strip_decor(raw))
                .unwrap_or_else(|| "—".to_owned());
            other.insert(
                path.clone(),
                SettingRow {
                    label: path.clone().into(),
                    value: value.into(),
                    key: path.clone().into(),
                    kind: ValueKind::Unset,
                    choices: choice_model(&schema::Kind::Text),
                    swatch: slint::Color::from_argb_u8(0, 0, 0, 0).into(),
                    checked: false,
                    hint: String::new().into(),
                    min: 0.0,
                    max: 0.0,
                    home: home as i32,
                    home_label: labels.get(home).cloned().unwrap_or_default(),
                    changed: current.get(home).and_then(|values| values.get(&path))
                        != shell.saved.get(home).and_then(|values| values.get(&path)),
                    available: false,
                    is_new: false,
                    preview: String::new().into(),
                    error: String::new().into(),
                },
            );
        }
    }
    if other.is_empty() {
        return None;
    }
    Some(SettingsCard {
        key: "other".into(),
        expanded: true,
        title: "other".into(),
        rows: Rc::new(VecModel::from(other.into_values().collect::<Vec<_>>())).into(),
    })
}

/// Page navigation + card collapse toggles.
pub(super) fn install_navigation(
    app: &AppWindow,
    shell: &Rc<RefCell<Shell>>,
    keybind_view: &Rc<RefCell<super::page_keybinds::KeybindView>>,
    kb_actions: &Arc<Mutex<Vec<keybinds::LiveAction>>>,
) {
    {
        let weak = app.as_weak();
        let keybind_view = Rc::clone(keybind_view);
        let kb_actions = Arc::clone(kb_actions);
        let shell = Rc::clone(shell);
        app.on_section_selected(move |name| {
            let Some(app) = weak.upgrade() else { return };
            // Outputs page: its own surface; rescan on open —
            // fast-fails when no compositor is reachable and keeps the
            // last detection.
            if name.as_str() == catalog::OUTPUTS_ID {
                super::page_outputs::scan_outputs(&mut shell.borrow_mut());
                app.set_current_section(name.clone());
                let (title, description) = page_meta(&name);
                app.set_page_title(title.into());
                app.set_page_description(description.into());
                app.set_page(Page::Outputs);
                super::page_outputs::rebuild_outputs(&app, &shell.borrow());
                return;
            }
            // Shaders page: rescan on open — cheap directory walk.
            if name.as_str() == "shaders" {
                super::page_shaders::scan_shaders(&mut shell.borrow_mut());
                app.set_current_section(name.clone());
                let (title, description) = page_meta(&name);
                app.set_page_title(title.into());
                app.set_page_description(description.into());
                app.set_page(Page::Shaders);
                super::page_shaders::rebuild_shaders(&app, &shell.borrow());
                super::page_shaders::maybe_check_shader_updates(&app, &shell);
                return;
            }
            // Rule pages: their own surface, one family per page.
            if super::page_rules::rule_family(&name).is_some() {
                app.set_current_section(name.clone());
                let (title, description) = page_meta(&name);
                app.set_page_title(title.into());
                app.set_page_description(description.into());
                app.set_page(Page::Rules);
                super::page_rules::rebuild_rule_page(&app, &shell.borrow());
                return;
            }
            // Keybinds page: its own surface, not a CategoryPage.
            if name.as_str() == "keybinds" {
                app.set_current_section(name.clone());
                app.set_page(Page::Keybinds);
                let shell = shell.borrow();
                super::page_keybinds::rebuild_keybind_rows(
                    &app,
                    &shell,
                    &kb_actions,
                    &keybind_view,
                );
                return;
            }
            let shell = shell.borrow();
            app.set_current_section(name.clone());
            let (title, description) = page_meta(&name);
            app.set_page_title(title.into());
            app.set_page_description(description.into());
            app.set_page(Page::Section);
            refill_page(&app, &shell, &name);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_toggle_card(move |key| {
            let Some(app) = weak.upgrade() else { return };
            let open = {
                let shell = shell.borrow();
                // The guide renders through the same cards; its step
                // must survive the rebuild.
                let in_guide = shell.guide.is_some();
                (in_guide, !card_expanded(&shell, &key, true))
            };
            shell
                .borrow_mut()
                .card_expanded
                .insert(key.to_string(), open.1);
            let shell = shell.borrow();
            if open.0 {
                super::guide::guide_show_step(&app, &shell);
            } else {
                refill_page(&app, &shell, app.get_current_section().as_str());
            }
        });
    }
}
