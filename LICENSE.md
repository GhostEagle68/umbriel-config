# MIT License

Copyright 2026 Ghosteagle68

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the “Software”), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

---

## Third-party notices

Umbriel Config bundles and builds on the work of others; their licenses
apply in addition to the MIT license above.

**Fonts (embedded in the binary)**

- **Inter** (Regular, Bold) — Copyright (c) 2016 The Inter Project
  Authors, licensed under the [SIL Open Font License 1.1][ofl]. Full
  text: `assets/fonts/LICENSE-Inter.txt`.
- **JetBrains Mono** (Regular) — Copyright 2020 The JetBrains Mono
  Project Authors, licensed under the [SIL Open Font License 1.1][ofl].
  Full text: `assets/fonts/LICENSE-JetBrainsMono.txt`.

**UI toolkit**

- **[Slint](https://slint.dev)** — the UI toolkit the application is
  built on, distributed by SixtyFPS GmbH under
  `GPL-3.0-only OR Slint-Royalty-free-2.0 OR Slint-Software-3.0`.
  This application uses and distributes Slint under the **Slint
  Royalty-free Desktop, Mobile, and Web Applications License 2.0** —
  full text: [LICENSE-Slint.md](LICENSE-Slint.md) (also included in the
  release tarballs). Its attribution condition (§2a) is satisfied by
  displaying Slint's `AboutSlint` widget on the application's Settings
  → About screen; that widget must stay for the distribution terms to
  hold. Anyone building from source may alternatively rely on Slint's
  GPL-3.0 option.

**Rust dependencies**

- Compiled from crates.io under their own per-crate licenses
  (predominantly MIT or Apache-2.0; the `slint` family as noted above).
  `cargo license` over `Cargo.lock` prints the full list for any given
  build.

[ofl]: https://openfontlicense.org
