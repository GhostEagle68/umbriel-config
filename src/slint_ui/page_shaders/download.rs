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

/// Unpack a GitHub tarball into `staging`, dropping the archive's top
/// folder (`<repo>-<branch>/`). Only files and folders are extracted, and
/// any path that isn't plainly inside `staging` fails the download.
fn unpack_archive(archive: &[u8], staging: &Path) -> Result<(), String> {
    let mut tarball = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for entry in tarball.entries().map_err(|err| err.to_string())? {
        let mut entry = entry.map_err(|err| err.to_string())?;
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            continue;
        }
        let dest = {
            let path = entry.path().map_err(|err| err.to_string())?;
            let mut parts = path.components();
            parts.next();
            let relative = parts.as_path();
            if relative.as_os_str().is_empty() {
                continue;
            }
            if !relative
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
            {
                return Err(format!("unsafe path in archive: {}", path.display()));
            }
            staging.join(relative)
        };
        if kind.is_dir() {
            std::fs::create_dir_all(&dest).map_err(|err| err.to_string())?;
        } else {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
            }
            entry.unpack(&dest).map_err(|err| err.to_string())?;
        }
    }
    Ok(())
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

    if let Err(err) = unpack_archive(&archive, &staging) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("Extraction failed: {err}"));
    }

    // Everything landed: swap the old collection for the fresh one,
    // keeping the preset files assignments rely on.
    shaders::carry_preset_files(target, &staging);
    let old = target.with_file_name(".community-old");
    let _ = std::fs::remove_dir_all(&old);
    let had_old = target.exists();
    if had_old && let Err(err) = std::fs::rename(target, &old) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!("Could not replace {}: {err}", target.display()));
    }
    if let Err(err) = std::fs::rename(&staging, target) {
        if had_old {
            let _ = std::fs::rename(&old, target);
        }
        let _ = std::fs::remove_dir_all(&staging);
        return Err(format!(
            "Could not install into {}: {err}",
            target.display()
        ));
    }
    let _ = std::fs::remove_dir_all(&old);
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
                    toast(
                        &app,
                        ToastKind::Error,
                        "Couldn't find the config directory to download into.",
                        "",
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
                            // Thumbnails for the newly arrived shaders.
                            app.invoke_shader_thumbs_wanted();
                            toast(&app, ToastKind::Success, note, "");
                        }
                        Err(note) => toast(&app, ToastKind::Error, note, ""),
                    }
                });
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive(files: &[(&str, &str)]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (path, body) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, path, body.as_bytes())
                .unwrap();
        }
        let tar = builder.into_inner().unwrap();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut gz, &tar).unwrap();
        gz.finish().unwrap()
    }

    #[test]
    fn unpack_drops_the_top_folder() {
        let staging = std::env::temp_dir().join(format!("umbriel-unpack-{}", std::process::id()));
        std::fs::remove_dir_all(&staging).ok();
        std::fs::create_dir_all(&staging).unwrap();
        let bytes = archive(&[("repo-main/animation/glow/shader.glsl", "vec4 x;")]);
        unpack_archive(&bytes, &staging).unwrap();
        let text = std::fs::read_to_string(staging.join("animation/glow/shader.glsl")).unwrap();
        assert_eq!(text, "vec4 x;");
        std::fs::remove_dir_all(&staging).ok();
    }

    #[test]
    fn unpack_rejects_paths_that_climb_out() {
        let staging = std::env::temp_dir().join(format!("umbriel-climb-{}", std::process::id()));
        std::fs::remove_dir_all(&staging).ok();
        std::fs::create_dir_all(&staging).unwrap();
        let mut bytes = Vec::new();
        {
            // `append_data` refuses `..`, so write the raw name into the header.
            let mut builder = tar::Builder::new(Vec::new());
            let mut header = tar::Header::new_gnu();
            header.set_size(1);
            header.set_mode(0o644);
            let name = b"repo-main/../evil";
            header.as_old_mut().name[..name.len()].copy_from_slice(name);
            header.set_cksum();
            builder.append(&header, &b"x"[..]).unwrap();
            let tar = builder.into_inner().unwrap();
            let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
            std::io::Write::write_all(&mut gz, &tar).unwrap();
            bytes.extend(gz.finish().unwrap());
        }
        let err = unpack_archive(&bytes, &staging).unwrap_err();
        assert!(err.contains("unsafe path"), "{err}");
        assert!(!staging.join("evil").exists());
        assert!(!staging.parent().unwrap().join("evil").exists());
        std::fs::remove_dir_all(&staging).ok();
    }
}
