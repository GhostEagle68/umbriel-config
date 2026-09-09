//! Curated catalog: the human names and grouping for the settings pages.
//! The config file's structure is for umbriel
//! combined tables are split into separate pages, and every page and
//! card gets a real name. New upstream keys inside a claimed sub-section
//! appear automatically; sub-sections the catalog doesn't claim fall
//! through to their area's page or the MORE sidebar group, so nothing
//! is ever hidden.

/// One card on a settings page: a slice of the schema claimed by exact
/// sub-section match.
pub struct Card {
    pub title: &'static str,
    pub section: &'static str,
}

/// One page: a sidebar entry and the cards it shows.
pub struct Page {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    /// Sidebar header this page sits under ("desktop", "look", …).
    pub group: &'static str,
    pub cards: &'static [Card],
}

/// Page id of the live-detected monitors page.
pub const OUTPUTS_ID: &str = "outputs";
/// Sidebar header for fallback pages (top-level sections the catalog
/// doesn't claim).
pub const MORE_GROUP: &str = "more";

pub const PAGES: &[Page] = &[
    Page {
        id: OUTPUTS_ID,
        title: "Outputs",
        description: "Your monitors, resolution, refresh rate, position, HDR.",
        group: "outputs",
        cards: &[],
    },
    Page {
        id: "workspaces",
        title: "Workspaces",
        description: "Workspace behavior and numbering.",
        group: "desktop",
        cards: &[Card {
            title: "Workspaces",
            section: "workspaces",
        }],
    },
    Page {
        id: "overview",
        title: "Overview",
        description: "The zoomed-out workspace overview.",
        group: "desktop",
        cards: &[Card {
            title: "Overview",
            section: "overview",
        }],
    },
    Page {
        id: "hot-corners",
        title: "Hot corners",
        description: "Actions triggered by pushing the pointer into a corner.",
        group: "desktop",
        cards: &[
            Card {
                title: "Top left",
                section: "hot_corners.top_left",
            },
            Card {
                title: "Top right",
                section: "hot_corners.top_right",
            },
            Card {
                title: "Bottom left",
                section: "hot_corners.bottom_left",
            },
            Card {
                title: "Bottom right",
                section: "hot_corners.bottom_right",
            },
        ],
    },
    Page {
        id: "layout",
        title: "Layout",
        description: "Tiling mode, gaps, and per-layout options.",
        group: "desktop",
        cards: &[
            Card {
                title: "Layout",
                section: "layout",
            },
            Card {
                title: "Scrolling",
                section: "layout.scrolling",
            },
            Card {
                title: "Dwindle",
                section: "layout.dwindle",
            },
            Card {
                title: "Master",
                section: "layout.master",
            },
            Card {
                title: "Struts",
                section: "layout.struts",
            },
        ],
    },
    Page {
        id: "animations",
        title: "Animations",
        description: "How windows and workspaces move.",
        group: "desktop",
        cards: &[
            Card {
                title: "Basics",
                section: "animation",
            },
            Card {
                title: "Windows opening",
                section: "animation.windows_in",
            },
            Card {
                title: "Windows closing",
                section: "animation.windows_out",
            },
            Card {
                title: "Window move",
                section: "animation.windows_move",
            },
            Card {
                title: "Workspaces",
                section: "animation.workspaces",
            },
            Card {
                title: "Overview",
                section: "animation.overview",
            },
            Card {
                title: "Scratchpad",
                section: "animation.scratchpad",
            },
            Card {
                title: "Border",
                section: "animation.border",
            },
            Card {
                title: "Dim unfocused",
                section: "animation.dim_unfocused",
            },
            Card {
                title: "Layers",
                section: "animation.layers",
            },
            Card {
                title: "Beziers",
                section: "animation.beziers",
            },
            Card {
                title: "Springs",
                section: "animation.springs",
            },
        ],
    },
    Page {
        id: "appearance",
        title: "Appearance",
        description: "Window decoration: borders, corners, blur, shadows.",
        group: "look",
        cards: &[
            Card {
                title: "Basics",
                section: "appearance",
            },
            Card {
                title: "Blur",
                section: "appearance.blur",
            },
            Card {
                title: "Shadows",
                section: "appearance.shadow",
            },
        ],
    },
    Page {
        id: "colors",
        title: "Colors",
        description: "The palette for panels, text, accents, and borders.",
        group: "look",
        cards: &[
            Card {
                title: "Basics",
                section: "colors",
            },
            Card {
                title: "Border",
                section: "colors.border",
            },
            Card {
                title: "Overview",
                section: "colors.overview",
            },
        ],
    },
    Page {
        id: "input",
        title: "Input basics",
        description: "Pasting and window-drag behavior.",
        group: "input",
        cards: &[Card {
            title: "Input",
            section: "input",
        }],
    },
    Page {
        id: "keyboard",
        title: "Keyboard",
        description: "Layout, repeat rate, Num Lock.",
        group: "input",
        cards: &[Card {
            title: "Keyboard",
            section: "input.keyboard",
        }],
    },
    Page {
        id: "touchpad",
        title: "Touchpad",
        description: "Tap, scroll, and pointer speed.",
        group: "input",
        cards: &[Card {
            title: "Touchpad",
            section: "input.touchpad",
        }],
    },
    Page {
        id: "mouse",
        title: "Mouse",
        description: "Scrolling and pointer speed.",
        group: "input",
        cards: &[Card {
            title: "Mouse",
            section: "input.mouse",
        }],
    },
    Page {
        id: "tablet",
        title: "Tablet",
        description: "Tablet tools, pads, and output mapping.",
        group: "input",
        cards: &[Card {
            title: "Tablet",
            section: "input.tablet",
        }],
    },
    Page {
        id: "cursor",
        title: "Cursor",
        description: "Cursor theme, size, and visibility.",
        group: "input",
        cards: &[Card {
            title: "Cursor",
            section: "input.cursor",
        }],
    },
    Page {
        id: "focus",
        title: "Focus",
        description: "Focus policies, follows mouse, activation requests.",
        group: "input",
        cards: &[Card {
            title: "Focus",
            section: "input.focus",
        }],
    },
    Page {
        id: "general",
        title: "General",
        description: "Core session behavior, XWayland, cheat sheet, autostart.",
        group: "system",
        cards: &[Card {
            title: "General",
            section: "general",
        }],
    },
    Page {
        id: "environment",
        title: "Environment",
        description: "Environment variables set for the session.",
        group: "system",
        cards: &[Card {
            title: "Variables",
            section: "environment",
        }],
    },
    Page {
        id: "events",
        title: "Hardware events",
        description: "Commands run when a laptop lid opens or closes.",
        group: "system",
        cards: &[Card {
            title: "Hooks",
            section: "events",
        }],
    },
];

/// Sidebar headers in display order (MORE is appended after these).
pub const GROUPS: &[&str] = &["outputs", "desktop", "look", "input", "system"];

/// Human header text for a sidebar group id.
pub fn group_title(group: &str) -> &'static str {
    match group {
        "outputs" => "Outputs",
        "desktop" => "Desktop",
        "look" => "Look",
        "input" => "Input",
        "system" => "System",
        MORE_GROUP => "More",
        _ => "",
    }
}

pub fn page(id: &str) -> Option<&'static Page> {
    PAGES.iter().find(|page| page.id == id)
}

/// Every schema sub-section the catalog claims, mapped to its page id.
pub fn claimed_sections() -> impl Iterator<Item = (&'static str, &'static str)> {
    PAGES
        .iter()
        .flat_map(|page| page.cards.iter().map(move |card| (card.section, page.id)))
}

/// The top-level config areas a page covers.
pub fn page_top_levels(page: &Page) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for card in page.cards {
        let top = card.section.split('.').next().unwrap_or(card.section);
        if !out.contains(&top) {
            out.push(top);
        }
    }
    out
}
