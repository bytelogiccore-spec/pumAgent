# Third-Party Licenses

This project is built using various open-source third-party dependencies from both the Rust (Cargo) and Node.js (NPM) ecosystems. We are grateful for the work of the open-source community.

Below is a summary of the major libraries used and their respective licenses:

## Rust Dependencies

Most Rust dependencies in this project are dual-licensed under the **MIT** and **Apache-2.0** licenses. Primary libraries include:

- `tauri` (MIT / Apache-2.0)
- `tokio` (MIT)
- `serde` (MIT / Apache-2.0)
- `scraper` (ISC / MIT)
- `teloxide` (MIT)
- `rquest` (MIT / Apache-2.0)
- `dbx-core` (MIT / Apache-2.0)
- `sqlx` (MIT / Apache-2.0)
- `rand` (MIT / Apache-2.0)
- `regex` (MIT / Apache-2.0)
- `chrono` (MIT / Apache-2.0)
- `rhai` (MIT / Apache-2.0)

_(A full detailed dependency tree is available by running `cargo tree` in the `src-tauri` directory)._

## Node.js / Frontend Dependencies

Most Node.js frontend dependencies are licensed under the **MIT** license. Primary libraries include:

- `svelte` (MIT)
- `vite` (MIT)
- `typescript` (Apache-2.0)
- `@tauri-apps/api` (MIT / Apache-2.0)
- `dompurify` (Apache-2.0 / MPL-2.0)
- `marked` (MIT)

_(A full list of frontend dependencies is available in `package.json` with details accessible via `npm list`)._

---

Full license texts for all third-party dependencies can typically be found within their downloaded packages or by visiting their respective source code repositories on `crates.io` and `npmjs.com`.
