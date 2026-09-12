# Research: Frontend Framework Comparison for Embedded WebConsole

**Date**: 2026-09-12  
**Context**: Selecting the frontend technology for `r2drive` WebConsole (`web/` directory), to be compiled into static assets and embedded inside a single Rust binary via `rust-embed`.

---

## Executive Summary & Recommendation

For an embedded web file manager in a single-binary Rust service, the key constraints are:
1. **Binary Footprint**: Minimized static asset size after gzip/brotli compression.
2. **Upload Pipeline UX**: Robust reactive state for multi-part, concurrent, resumable drag-and-drop uploads directly to Cloudflare R2 presigned URLs.
3. **Component Ecosystem**: Availability of headless primitives (dialogs, context menus, trees, progress bars, icon sets).
4. **Maintenance Velocity**: Simple, isolated frontend build step (`pnpm build` -> `dist/`) without complex backend couplings.

| Criteria | React 19 + Vite | Svelte 5 + Vite | Vue 3.5 + Vite |
| :--- | :--- | :--- | :--- |
| **Gzipped Bundle Size** | ~130 KB – 160 KB | **~35 KB – 55 KB** (Best) | ~80 KB – 100 KB |
| **Reactivity for File Uploads** | Needs state library (Zustand/TanStack) | **Native Runes (`$state`)** (Best) | Composition API (`ref`/`reactive`) |
| **File Manager Precedents** | MinIO Console, Cloudflare Dashboard | PocketBase Admin (earlier versions) | **FileBrowser** (`filebrowser/filebrowser`) |
| **Drag & Drop / Tree Ecosystem** | **Vast** (Shadcn, Radix, Lucide-React) | Good (Bits UI, Lucide-Svelte) | Strong (VueUse, Lucide-Vue-Next) |
| **Learning Curve / Boilerplate** | Moderate (Hooks lifecycle, re-renders) | **Very Low** (Clean single-file components) | Low |

### Recommendation: **React (with Vite + TailwindCSS + Lucide Icons)** OR **Vue 3**
- **Option 1: React + Vite + TailwindCSS (Recommended for Speed of Development)**:
  - Unmatched ecosystem of ready-to-use file management primitives (`react-dropzone`, `@tanstack/react-table`, `lucide-react`).
  - The ~140KB gzipped bundle adds less than 0.5% to the final ~45MB Docker image or ~25MB Rust binary.
- **Option 2: Vue 3 + Vite (The "FileBrowser" Path)**:
  - Proven architecture by `filebrowser/filebrowser` (the most popular open-source web file manager, 25k+ stars).
  - Clean separation of templates and reactivity with `@vueuse/core`.

---

## Detailed Evaluation

### 1. Embedded Binary Size Impact
In a Rust project using `rust-embed` or `include_dir!`:
- Assets in `web/dist/` are compressed (gzip/brotli) at compile time.
- React: React 19 core + ReactDOM is ~45KB gzipped. With Tailwind CSS and Lucide icons, total JS+CSS bundle is ~140KB.
- Svelte 5: Compiles away the framework runtime into direct DOM manipulations. Baseline bundle is ~20KB gzipped, total ~50KB.
- **Verdict**: While Svelte produces the smallest bundle, the 80KB difference between Svelte and React is negligible in a Docker container or desktop binary where the Rust executable is ~20MB+.

### 2. Multi-Part Presigned Upload State Handling
The WebConsole must track complex upload states:
- Active bucket profile, current prefix/breadcrumbs.
- Upload queue: each file split into 10MB parts with individual presigned URLs, concurrency limit (e.g. 4 parts in flight), upload progress percentages, retry counts, speed calculation.
- **In React**: Handled cleanly with `zustand` or `tanstack-query`.
- **In Svelte 5**: Handled natively using fine-grained reactivity runes (`$state`).
- **In Vue 3**: Handled via reactive stores (`pinia` or simple `reactive()` modules) and `useDropZone` from `@vueuse/core`.

### 3. Industry Precedents in Self-Hosted Single-Binary Services
1. **FileBrowser (`filebrowser/filebrowser`)**:
   - Stack: Vue 3 + Vite + Go embedded binary.
   - Outcome: Extremely polished, responsive file explorer with breadcrumbs, grid/list view, and multi-file selection.
2. **MinIO Console (`minio/console`)**:
   - Stack: React + TypeScript + Go embedded.
   - Outcome: Scalable enterprise storage management for multi-tenant S3 clusters.
3. **PocketBase (`pocketbase/pocketbase`)**:
   - Stack: Svelte (previously), now ultra-minimal JS + Go embedded.
   - Outcome: Tiny binary footprint (<15MB total).

---

## Decision Matrix for r2drive Phase 1

1. **If maximum community familiarity and rich headless UI components are prioritized**: Choose **React + Vite + Tailwind CSS**.
2. **If following the proven track record of dedicated file manager UIs (FileBrowser) is prioritized**: Choose **Vue 3 + Vite + Tailwind CSS**.
3. **If absolute minimum lines of code and bundle size are prioritized**: Choose **Svelte 5 + Vite**.
