---
name: marp-presentation-generator
description: Create, edit, polish, and validate Marp Markdown slide decks. Use when Codex needs to generate a presentation, turn an outline or source material into slides, improve an existing Marp deck, choose default, gaia, or uncover themes, add Marp directives, or prepare Markdown for Marp for VS Code or Marp CLI export.
---

# Marp Presentation Generator

## Workflow

1. Determine the deliverable:
   - If the user asks for a file, create or edit a `.md` deck at the requested path, otherwise use the current workspace.
   - If the user asks for inline output, return the complete deck in a `markdown` code block only.
   - If the user asks for PDF, PPTX, HTML, or image export, first produce valid Marp Markdown; then use Marp tooling only when available or explicitly requested.
2. Gather missing content only when required. If the topic, outline, or source material is absent, ask for it. If theme, slide count, audience, or tone is absent, choose reasonable defaults and proceed.
3. Start from `assets/template-default.md`, `assets/template-gaia.md`, or `assets/template-uncover.md` when a bundled template matches the requested style.
4. Write the deck with valid front matter, clear slide boundaries, concise slide content, and relative asset paths.
5. Review the finished deck for directive validity, slide flow, content density, spelling, and broken references.

## Theme Selection

- Use `default` for neutral technical, business, or documentation-style decks.
- Use `gaia` for brighter presentation decks. Use `class: lead` or `_class: lead` on title and section-divider slides when centering helps.
- Use `uncover` for sparse, modern decks with minimal text and strong visual hierarchy.
- Honor an explicit user theme preference. If the user requests a custom theme, verify the theme CSS path or existing workspace configuration before referencing it.

## Marp Structure

Use YAML front matter at the top of every new deck:

```markdown
---
marp: true
theme: default
paginate: true
size: 16:9
---
```

- Include `marp: true` for Marp for VS Code compatibility.
- Use `---` on its own line after front matter to separate slides.
- Put global directives such as `theme`, `paginate`, `size`, `header`, and `footer` in front matter.
- Put local directives in HTML comments immediately before the affected slide.
- Prefix a local directive with `_` when it should apply only to the current slide, such as `<!-- _paginate: false -->` or `<!-- _class: lead -->`.

## Slide Writing Rules

- Use one main idea per slide.
- Prefer 3-5 short bullets, a compact table, or a single visual over dense paragraphs.
- Start content slides with a descriptive `##` heading.
- Use title slides and section dividers sparingly; keep them short.
- Preserve the user's terminology and factual claims. Do not invent data, citations, logos, screenshots, or source names.
- For image-heavy slides, use Marp image syntax such as `![bg right:40%](path/to/image.png)` or `![w:640](path/to/image.png)`.
- Keep speaker notes, if requested, in HTML comments so they do not render as slide body text.

## Quality Check

Before finishing:

1. Confirm the file starts with valid YAML front matter.
2. Confirm slide separators are `---` on their own line.
3. Confirm front matter is not mistaken for the first slide separator.
4. Confirm local directives are placed before the slide they modify.
5. Confirm image and asset paths are relative to the deck location unless the user requested absolute paths.
6. Confirm the deck does not overflow obvious slide bounds with long paragraphs, large tables, or too many bullets.
7. If Marp tooling is available and the user requested rendered output, run the smallest useful render or preview check and fix reported errors.

## Inline Output Format

When returning a deck inline, output only the Markdown code fence:

````markdown
```markdown
---
marp: true
theme: default
paginate: true
size: 16:9
---

<!-- _class: lead -->
# Presentation Title

Subtitle or presenter name

---

## Slide Heading

- Key point one
- Key point two
- Key point three

---

## Conclusion

- Summarize the main takeaway
- State the next action
```
````
