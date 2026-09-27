**Two update channels: Stable and Canary.** Pick in Settings → Updates.
- **Canary.** A new build with every change, published shortly after it lands. It's the default while the app is in beta, so fixes for Umbriel's changes reach you the same day.
- **Stable.** Tested releases only, for when you'd rather update less often.
- **What changed, for you.** Canary shows only the changes since the build you're running, before you install and once more after you restart.
- **Hands-off updates.** Canary can install new builds on launch by itself, and says so in a line under the header.
- **Undo an update.** If a build misbehaves, `umbriel-config rollback` puts back the one it replaced.
- **Canary from the installer.** Add `-s -- --canary` to the one-line install command to start on Canary.
