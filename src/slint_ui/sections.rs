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
        Page::Home => super::page_home::rebuild_home(app, shell),
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
pub(super) fn page_new_counts(shell: &Shell) -> BTreeMap<String, i32> {
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

/// Every page the sidebar can show, in order, as (group, page id,
/// label): catalog pages whose settings this umbriel reads (or whose
/// keys your files set), then the MORE fallback pages for top-level
/// areas the catalog doesn't claim.
fn nav_pages(shell: &Shell) -> Vec<(&'static str, String, String)> {
    let sets = chain_path_sets(shell);
    // A page of cards none of which this umbriel reads (a newer
    // umbriel's settings) would be empty, unless your files set keys
    // there (`[environment]`'s variables).
    let readable = |page: &&catalog::Page| {
        page.cards.is_empty()
            || page.cards.iter().any(|card| {
                let prefix = format!("{}.", card.section);
                shell
                    .schema
                    .iter()
                    .any(|entry| entry.section == card.section)
                    || sets
                        .iter()
                        .any(|set| set.iter().any(|key| key.starts_with(&prefix)))
            })
    };
    let mut pages: Vec<(&'static str, String, String)> = Vec::new();
    let mut claimed_tops: Vec<&'static str> = Vec::new();
    for group in catalog::GROUPS {
        for page in catalog::PAGES
            .iter()
            .filter(|page| page.group == *group)
            .filter(readable)
        {
            claimed_tops.extend(catalog::page_top_levels(page));
            pages.push((group, page.id.to_owned(), page.title.to_owned()));
        }
    }
    for name in section_names(&shell.schema) {
        if !claimed_tops.contains(&name.as_str()) {
            pages.push((catalog::MORE_GROUP, name.to_string(), prettify(&name)));
        }
    }
    pages
}

/// The sidebar model: Outputs, then one collapsible header per group
/// with its pages under it while open. A group is open when the user
/// opened it, or, until they choose, when it holds `current`.
pub(super) fn section_nav(shell: &Shell, current: &str) -> Vec<SectionNav> {
    let counts = page_new_counts(shell);
    let pages = nav_pages(shell);
    let mut nav: Vec<SectionNav> = Vec::new();
    let mut groups: Vec<&str> = catalog::GROUPS.to_vec();
    groups.push(catalog::MORE_GROUP);
    for group in groups {
        let members: Vec<&(&str, String, String)> =
            pages.iter().filter(|(g, _, _)| *g == group).collect();
        let row = |(_, id, label): &(&str, String, String), child: bool, last: bool| SectionNav {
            label: label.as_str().into(),
            id: id.as_str().into(),
            new_count: counts.get(id.as_str()).copied().unwrap_or(0),
            is_header: false,
            group: group.into(),
            expanded: false,
            child,
            last,
        };
        // Outputs is a group of one: a plain row, no header.
        if group == "outputs" {
            nav.extend(members.iter().map(|page| row(page, false, false)));
            continue;
        }
        if members.is_empty() {
            continue;
        }
        let key = format!("group:{group}");
        let holds_current = members.iter().any(|(_, id, _)| id == current);
        let expanded = card_expanded(shell, &key, holds_current);
        nav.push(SectionNav {
            label: catalog::group_title(group).into(),
            id: key.into(),
            new_count: members
                .iter()
                .map(|(_, id, _)| counts.get(id.as_str()).copied().unwrap_or(0))
                .sum(),
            is_header: true,
            group: group.into(),
            expanded,
            child: false,
            last: false,
        });
        if expanded {
            let last = members.len() - 1;
            nav.extend(
                members
                    .iter()
                    .enumerate()
                    .map(|(i, page)| row(page, true, i == last)),
            );
        }
    }
    nav
}

/// One settings page's cards.
pub(super) fn page_cards(shell: &Shell, page_id: &str) -> Vec<SettingsCard> {
    if page_id == catalog::OUTPUTS_ID {
        return super::page_outputs::output_cards(shell);
    }
    if page_id == "environment" {
        return vec![environment_card(shell)];
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
            // Effect selections belong to the Shaders page's assignment UI.
            .filter(|entry| {
                entry.section == card.section
                    && !(entry.section.starts_with("animation.")
                        && entry.path.last() == Some(&"effect".to_owned()))
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
        // Sub-sections no page claims land on the first page of their
        // area, not on every page sharing it.
        if claimed.contains(&entry.section.as_str())
            || page_id_for_section(&entry.section) != page.id
        {
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

/// Row key of the Environment page's "add a variable" box.
pub(super) const ENVIRONMENT_ADD: &str = "environment:add";

/// The Environment page: one row per variable, edited in the file that
/// sets it, then a box that adds one.
fn environment_card(shell: &Shell) -> SettingsCard {
    let sets = chain_path_sets(shell);
    let labels = setting_labels(shell);
    let mut rows: Vec<SettingRow> = Vec::new();
    for home in 0..=shell.includes.docs.len() {
        let doc = doc_at(shell, home);
        let current: BTreeMap<String, String> = doc.leaf_values().into_iter().collect();
        for name in doc.table_keys(&["environment"]) {
            let key = format!("environment.{name}");
            // A variable set in two files shows once, where it wins.
            if entry_home(&sets, &key) != Some(home) {
                continue;
            }
            let path = ["environment", name.as_str()];
            let mut row = blank_row(key.clone(), &name, home as i32);
            row.value = doc
                .get_string(&path)
                .or_else(|| doc.get_raw(&path))
                .unwrap_or_default()
                .into();
            row.hint = "Clear the value to remove the variable.".into();
            row.home_label = labels.get(home).cloned().unwrap_or_default();
            row.changed =
                current.get(&key) != shell.saved.get(home).and_then(|values| values.get(&key));
            rows.push(row);
        }
    }
    let mut add = blank_row(ENVIRONMENT_ADD.to_owned(), "Add a variable", -1);
    add.hint = "Type NAME=value and press Enter.".into();
    rows.push(add);
    let key = "card:environment:Variables";
    SettingsCard {
        title: "Variables".into(),
        key: key.into(),
        expanded: card_expanded(shell, key, true),
        rows: Rc::new(VecModel::from(rows)).into(),
    }
}

/// Write one Environment page edit: `NAME=value` from the add box, or a
/// variable's new value, where an empty value removes it.
pub(super) fn set_environment(shell: &mut Shell, key: &str, raw: &str) -> Result<(), String> {
    let (name, value) = match key.strip_prefix("environment.") {
        Some(name) => (name, raw.trim()),
        None => raw
            .split_once('=')
            .map(|(name, value)| (name.trim(), value.trim()))
            .ok_or("type NAME=value, e.g. MOZ_ENABLE_WAYLAND=1")?,
    };
    let valid = name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid {
        return Err(format!(
            "'{name}' is not a variable name (letters, digits, _)"
        ));
    }
    let main = shell.includes.docs.len();
    // An existing variable is edited where it lives; a new one joins the
    // file that already sets variables.
    let target = entry_home(&chain_path_sets(shell), &format!("environment.{name}"))
        .or_else(|| (0..=main).find(|&i| !doc_at(shell, i).table_keys(&["environment"]).is_empty()))
        .unwrap_or(main);
    let doc = doc_at_mut(shell, target);
    if value.is_empty() && key != ENVIRONMENT_ADD {
        doc.remove_leaf(&["environment", name]);
    } else {
        doc.set_string(&["environment", name], value);
    }
    Ok(())
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
    let claims = schema::managed_claims(&docs, &shell.output_fields);
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
            // A group header opens or closes its pages.
            if name.starts_with("group:") {
                let open = {
                    let shell = shell.borrow();
                    section_nav(&shell, &app.get_current_section())
                        .iter()
                        .find(|entry| entry.id == name)
                        .is_some_and(|entry| !entry.expanded)
                };
                shell
                    .borrow_mut()
                    .card_expanded
                    .insert(name.to_string(), open);
                let nav = section_nav(&shell.borrow(), &app.get_current_section());
                app.set_sections(Rc::new(VecModel::from(nav)).into());
                return;
            }
            app.set_page_group(
                catalog::page(&name)
                    .map_or(catalog::MORE_GROUP, |page| page.group)
                    .into(),
            );
            // The page's group opens with it (unless the user closed it).
            {
                let nav = section_nav(&shell.borrow(), &name);
                app.set_sections(Rc::new(VecModel::from(nav)).into());
            }
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
                let (title, description) = page_meta(&name);
                app.set_page_title(title.into());
                app.set_page_description(description.into());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_edits_land_where_the_variables_live() {
        let dir = std::env::temp_dir().join(format!("umbriel-env-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let main_path = dir.join("config.toml");
        std::fs::write(dir.join("env.toml"), "[environment]\nFOO = \"1\"\n").unwrap();
        std::fs::write(&main_path, "[include]\nfiles = [\"env.toml\"]\n").unwrap();
        let mut shell = Shell::load(&main_path, &discovery::Env::from_process());
        let value = |shell: &Shell, name: &str| {
            shell.includes.docs[0]
                .doc
                .get_string(&["environment", name])
        };

        // A new variable joins the file that already sets variables.
        set_environment(&mut shell, ENVIRONMENT_ADD, " BAR = x=y ").unwrap();
        assert_eq!(value(&shell, "BAR").as_deref(), Some("x=y"));
        set_environment(&mut shell, "environment.FOO", "2").unwrap();
        assert_eq!(value(&shell, "FOO").as_deref(), Some("2"));
        let rows = environment_card(&shell).rows;
        assert_eq!(rows.row_count(), 3);
        // Clearing a value removes the variable.
        set_environment(&mut shell, "environment.FOO", "").unwrap();
        assert_eq!(value(&shell, "FOO"), None);
        assert!(shell.doc.table_keys(&["environment"]).is_empty());
        assert!(set_environment(&mut shell, ENVIRONMENT_ADD, "1BAD=x").is_err());
        assert!(set_environment(&mut shell, ENVIRONMENT_ADD, "no equals").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
