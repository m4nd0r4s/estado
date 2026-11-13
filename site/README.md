# estado site

Landing page and documentation, built with SvelteKit (Svelte 5) as a static site and published to GitHub Pages by `.github/workflows/site.yml`.

The content is not duplicated here: the docs pages render `docs/guide/`, `docs/reference/` and `docs/errors/` from the repository at build time, and the landing page quotes the compile-fail snapshots in `crates/estado-macros/tests/ui/`. Edit those files, not the site.

```bash
pnpm install
pnpm dev          # http://localhost:5173
pnpm check        # svelte-check
pnpm lint         # prettier
pnpm test         # node --test: slugs, link rewriting, build guards
BASE_PATH=/estado pnpm build   # what CI publishes, into build/
```

- `src/lib/server/` reads and renders the Markdown (marked, Shiki with the `estado-ink` theme), rewrites links between docs to site routes and links to other repository files to GitHub.
- `src/lib/components/landing/` holds the landing sections; `player.ts` models the README's media player for the interactive statechart.

The build fails on a relative link to a missing file, on raw HTML other than inline tags, and when the landing's code excerpt no longer matches `README.md`.

Publishing needs the repository's Pages source set to "GitHub Actions" (Settings → Pages).
