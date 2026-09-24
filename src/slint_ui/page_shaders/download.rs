//! The community collection: downloading it over HTTPS and the once-per-session
//! update check.

use super::super::common::*;
use super::super::*;

pub(super) const SHADERS_UPSTREAM_COMMITS: &str =
    "https://api.github.com/repos/noctalia-dev/community-umbriel-shaders/commits?per_page=1";

pub(super) const SHADERS_UPSTREAM_MARKER: &str = ".upstream";

/// Largest community archive the download accepts.
pub(super) const ARCHIVE_LIMIT: u64 = 64 * 1024 * 1024;

/// The community repo's newest commit SHA, or `None` when GitHub is
/// unreachable — an unknown upstream never flips the update marker.
pub(super) fn fetch_latest_commit_sha() -> Option<String> {
    github_latest_commit(SHADERS_UPSTREAM_COMMITS).map(|(sha, _, _)| sha)
}

/// The SHA recorded when the community collection was last downloaded.
pub(super) fn installed_commit_sha(target: &Path) -> Option<String> {
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
pub(in crate::slint_ui) fn maybe_check_shader_updates(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
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
pub(super) fn download_community_shaders(target: &Path) -> Result<String, String> {
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

    // Beside the staging folder in the user's own config dir, not a
    // guessable shared /tmp name, and created fresh (never through a
    // pre-existing file or symlink).
    let archive_path = target.with_file_name(".community-download.tar.gz");
    let _ = std::fs::remove_file(&archive_path);
    let stored = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&archive_path)
        .and_then(|mut file| std::io::Write::write_all(&mut file, &archive));
    if let Err(err) = stored {
        let _ = std::fs::remove_file(&archive_path);
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

pub(super) fn install(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
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
}
