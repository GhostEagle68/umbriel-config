# Highlight fragments

One file here per **headline change**: a feature worth a paragraph in
the release notes, not one line per commit. Write it when the feature
lands, while you still remember what it does for the person using it.

```
changes/shaders.md
```

- **Lead with a bold title**, then one to three sentences in plain user
  language. What it lets someone do, not how it is built.
- **One blank line between fragments** is added for you; don't add
  headings — the release step writes the `### Highlights` heading.
- **Filenames order the section** (alphabetical) and are never shown, so
  a number prefix puts them in reading order: `1-shaders.md`,
  `2-shader-editor.md`, `3-updates.md`.
- No emojis: the app's What's-new overlay can't draw them as of now.

Cutting a release folds every fragment into the new CHANGELOG.md section
as **Highlights**, above the per-commit lists, then deletes the files in
the release commit. Nothing here survives a release,
so an empty folder means everything shipped.

Not every change needs one. A bug fix or a small feature is served fine
by its commit subject; reach for a fragment when several commits add up
to one story.
