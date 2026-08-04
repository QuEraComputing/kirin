# Issue tracker: Local Markdown

Issues and specs for this repository live as Markdown files in `.scratch/`.

## Conventions

- One feature per directory: `.scratch/<feature-slug>/`
- The spec is `.scratch/<feature-slug>/spec.md`
- Implementation issues are individual files under
  `.scratch/<feature-slug>/issues/`, numbered from `01`
- A `Status:` line near the top records triage state
- Comments append under a `## Comments` heading

## Publishing and fetching

When a skill publishes a spec or issue, it creates the corresponding Markdown
file under `.scratch/`. When a skill fetches a ticket, it reads the referenced
file directly.
