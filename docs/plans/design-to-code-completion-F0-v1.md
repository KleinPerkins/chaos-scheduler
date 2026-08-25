# Design-to-Code Completion — F0 Foundation Primitives

**Version:** v1
**Status:** DRAFT
**Date:** 2026-08-24
**Plan-type:** agentic-execution
**Parent plan:** `docs/plans/design-to-code-completion-v2.md` (screen-per-session roadmap, ACCEPTED 2026-08-24)
**GitHub:** epic #329 / child issue #334

**Goal:** Deliver the four foundation fixes that every S1–S7 screen session otherwise re-inherits:

1. Tokenize `ui/Notice` (hardcoded Google-palette RGB → `cs.*` semantic token vars)
2. Fix the systemic off-scale `.page-title` (20px literal → `var(--font-size-2xl)` + tight line-height); create co-located `PageHeader.css`
3. Lift `Modal` + `PageHeader` to composed `cs.*`-bound masters (token-bound chrome; `Modal.css` shared shell)
4. Add a CI lint gate blocking raw hex in `src/**/*.{css,tsx,ts}` wired into the existing `ci-required` fan-in

No visual redesign. No new tokens invented. Every change maps to an existing `cs.*` token.

---

## Pre-acceptance self-review

- [x] Spec coverage: all four F0 requirements (from v2 plan §5/F0) map to work items below
- [x] Placeholder scan: no TBD or empty code blocks
- [x] Type consistency: no new types introduced; all file paths exact
- [x] Self-containment: each work item gives runnable commands with expected output

---

## 5. Work items

---

### T1 — Tokenize `ui/Notice.css`

**Files:**

- `src/components/ui/Notice.css` (edit)
- `src/components/ui/Notice.test.tsx` (create — fails-first)

**Interfaces:**

- Consumes: `--accent-rgb`, `--success-rgb`, `--error-rgb`, `--warning-rgb`, `--success`, `--error`, `--warning` (all defined in `src/styles/tokens.css`)
- Produces: `src/components/ui/Notice.css` using only `var(--*)` color references; `Notice.test.tsx` passing

**TDD steps:**

- [ ] **RED** — Write `src/components/ui/Notice.test.tsx` that reads `Notice.css` and asserts it contains no raw hex or `rgba(` calls with literal RGB triplets (the file must not contain the pattern `/rgba\(\s*\d+,\s*\d+,\s*\d+/`). This test fails before the edit.

```typescript
// src/components/ui/Notice.test.tsx
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join, dirname } from "node:path";
import { describe, expect, it } from "vitest";

const css = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "Notice.css"),
  "utf8",
);

describe("Notice.css tokenization", () => {
  it("contains no raw RGB triplets in rgba() — all colors bind to cs.* token vars", () => {
    // Matches rgba( with a literal numeric first arg (i.e. not var(--…))
    expect(css).not.toMatch(/rgba\(\s*\d+\s*,\s*\d+/);
  });
});
```

Run to confirm failure:

```bash
cd /tmp/cs-f0 && npx vitest run src/components/ui/Notice.test.tsx 2>&1 | tail -15
```

- [ ] **GREEN** — Edit `src/components/ui/Notice.css`:

```css
/* was: rgba(66, 133, 244, 0.1) / rgba(66, 133, 244, 0.35) — Google Blue, not in cs.* token system */
.ui-notice--info {
  background: rgba(var(--accent-rgb), 0.1);
  border-color: rgba(var(--accent-rgb), 0.35);
  color: var(--text-primary);
}

.ui-notice--success {
  background: rgba(var(--success-rgb), 0.12);
  border-color: rgba(var(--success-rgb), 0.35);
  color: var(--success);
}

.ui-notice--error {
  background: rgba(var(--error-rgb), 0.12);
  border-color: rgba(var(--error-rgb), 0.4);
  color: var(--error);
}

.ui-notice--warning {
  background: rgba(var(--warning-rgb), 0.12);
  border-color: rgba(var(--warning-rgb), 0.4);
  color: var(--warning);
}
```

Run to confirm pass:

```bash
cd /tmp/cs-f0 && npx vitest run src/components/ui/Notice.test.tsx 2>&1 | tail -10
```

**Design note:** The `info` variant used Google Blue (`#4285F4`), which has no matching `cs.*` token. Mapped to `--accent-rgb` (brand purple, `#6355e8`). This IS a visual drift from Google Blue → brand accent; it is the correct MC alignment but the operator should confirm if a dedicated `--info` token is preferred in a future token PR. The amber/red/green variants are exact rgb-triplet matches (`--error-rgb` = `234, 67, 53` matches the old literal exactly in dark theme).

---

### T2 — Fix off-scale `.page-title`; create co-located `PageHeader.css`

**Files:**

- `src/components/PageHeader.css` (create)
- `src/components/PageHeader.tsx` (edit — add import)
- `src/components/Dashboard.css` (edit — remove 3 rules now in PageHeader.css)
- `src/components/WorkflowDetail.tsx` (edit — add PageHeader.css import)
- `src/components/PageHeader.test.tsx` (edit — add token-bound assertion)

**Interfaces:**

- Consumes: `--font-size-2xl`, `--font-size-md`, `--line-height-tight`, `--text-secondary`, `--space-6` (all from `src/styles/tokens.css`)
- Produces: `PageHeader.css` (canonical home of `.page-header`/`.page-title`/`.page-subtitle`); Dashboard.css without those 3 rule blocks

**TDD steps:**

- [ ] **RED** — Add to `src/components/PageHeader.test.tsx` a test that reads `PageHeader.css` and asserts `.page-title` uses `var(--font-size-2xl)`, not a raw pixel value:

```typescript
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join, dirname } from "node:path";

const css = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "PageHeader.css"),
  "utf8",
);

it("PageHeader.css defines .page-title with var(--font-size-2xl), not a raw px literal", () => {
  // Must contain var(--font-size-2xl) in the .page-title block
  expect(css).toMatch(/\.page-title\s*\{[^}]*var\(--font-size-2xl\)/s);
  // Must NOT use a raw 20px font-size
  expect(css).not.toMatch(/\.page-title\s*\{[^}]*font-size\s*:\s*20px/s);
});
```

This test fails because `PageHeader.css` does not exist yet.

Run to confirm failure:

```bash
cd /tmp/cs-f0 && npx vitest run src/components/PageHeader.test.tsx 2>&1 | tail -10
```

- [ ] **GREEN** — Create `src/components/PageHeader.css`:

```css
.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-6);
}

.page-title {
  font-size: var(--font-size-2xl);
  font-weight: 600;
  line-height: var(--line-height-tight);
}

.page-subtitle {
  font-size: var(--font-size-md);
  color: var(--text-secondary);
  margin-top: 2px;
}
```

Edit `src/components/PageHeader.tsx` — add at the top of imports:

```typescript
import "./PageHeader.css";
```

Edit `src/components/Dashboard.css` — remove the three rule blocks:

```
.page-header { … }
.page-title { … }
.page-subtitle { … }
```

Edit `src/components/WorkflowDetail.tsx` — add after the existing CSS import line:

```typescript
import "./PageHeader.css";
```

(WorkflowDetail uses `.page-title` and `.page-header` directly; since it does not import `<PageHeader>` it needs the CSS directly. RunDetail already imports `PageHeader` → gets PageHeader.css transitively.)

Run to confirm pass:

```bash
cd /tmp/cs-f0 && npx vitest run src/components/PageHeader.test.tsx 2>&1 | tail -10
```

---

### T3 — Lift `Modal` to cs.*-bound composed master

**Files:**

- `src/components/Modal.css` (create)
- `src/components/Modal.tsx` (edit — import css + merge base classes)
- `src/components/RerunModal.css` (edit — drop chrome rules now in Modal.css)
- `src/components/Modal.test.tsx` (edit — add token-bound base-class assertion)

**Interfaces:**

- Consumes: `--scrim-rgb`, `--bg-secondary`, `--border`, `--shadow`, `--radius-lg`, `--space-5`, `--space-6` (all from `src/styles/tokens.css`)
- Produces: `Modal.css` (structural chrome with token vars); `Modal.tsx` applying `.modal-backdrop/.modal-scrim/.modal-dialog` by default; `RerunModal.css` dropping duplicate chrome rules

**TDD steps:**

- [ ] **RED** — Add to `src/components/Modal.test.tsx` a test asserting that `Modal` applies base `modal-backdrop` / `modal-scrim` / `modal-dialog` classes even without consumer `*ClassName` props:

```typescript
it("applies base modal-backdrop / modal-scrim / modal-dialog classes when no *ClassName props are passed", () => {
  const { container } = render(
    <Modal onClose={vi.fn()}>
      <p>Body</p>
    </Modal>,
  );
  const backdrop = container.firstChild as HTMLElement;
  expect(backdrop.classList).toContain("modal-backdrop");
  const scrim = backdrop.children[0] as HTMLElement;
  expect(scrim.classList).toContain("modal-scrim");
  const dialog = backdrop.children[1] as HTMLElement;
  expect(dialog.classList).toContain("modal-dialog");
});
```

This fails before the Modal.tsx edit (currently all classes are undefined without props).

Run to confirm failure:

```bash
cd /tmp/cs-f0 && npx vitest run src/components/Modal.test.tsx 2>&1 | tail -10
```

- [ ] **GREEN** — Create `src/components/Modal.css`:

```css
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
}

.modal-scrim {
  position: absolute;
  inset: 0;
  border: none;
  padding: 0;
  margin: 0;
  width: 100%;
  height: 100%;
  background: rgba(var(--scrim-rgb), 0.55);
  cursor: default;
}

.modal-dialog {
  position: relative;
  z-index: 1;
  border-radius: var(--radius-lg);
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  box-shadow: var(--shadow);
}
```

Edit `src/components/Modal.tsx` — import the CSS and merge base classes:

```typescript
import "./Modal.css";
```

Replace the three class-merging lines at the bottom of the render function:

```typescript
const backdropClasses = ["modal-backdrop", backdropClassName]
  .filter(Boolean)
  .join(" ");
const scrimClasses = ["modal-scrim", scrimClassName].filter(Boolean).join(" ");
const dialogClasses = ["modal-dialog", className].filter(Boolean).join(" ");
```

(Remove the `|| undefined` fallbacks — now always at least the base class.)

Edit `src/components/RerunModal.css` — drop the chrome rules now in `Modal.css`:
Remove from `.rerun-modal-backdrop`: `position`, `inset`, `z-index`, `display`, `align-items`, `justify-content` (keep `padding: var(--space-6)` since it matches the Modal.css default; remove it too since Modal.css provides `padding: var(--space-6)` — **confirm identical value first**).
Remove `.rerun-modal-scrim` rule entirely (Modal.css `.modal-scrim` handles it).
Remove from `.rerun-modal`: `position: relative`, `z-index: 1`, `border-radius`, `background`, `border`, `box-shadow` (keep `width: min(520px, 100%)`, `padding: var(--space-5)` as RerunModal-specific sizing).

Run to confirm pass:

```bash
cd /tmp/cs-f0 && npx vitest run src/components/Modal.test.tsx 2>&1 | tail -10
```

---

### T4 — Hex lint gate

**Files:**

- `scripts/check-no-raw-hex.mjs` (create)
- `scripts/check-no-raw-hex.test.mjs` (create — fails-first)
- `.github/workflows/ci.yml` (edit — add `no-raw-hex` job + fan-in)
- `lefthook.yml` (edit — add `no-raw-hex` pre-commit command)
- `package.json` (edit — add `check:no-raw-hex` + `test:no-raw-hex` scripts)

**Interfaces:**

- Consumes: `src/**/*.css`, `src/**/*.tsx`, `src/**/*.ts` (tracked files)
- Produces: `scripts/check-no-raw-hex.mjs` (exits 1 on raw hex hit); `no-raw-hex` job in `ci-required` fan-in

**TDD steps:**

- [ ] **RED** — Write `scripts/check-no-raw-hex.test.mjs`. It imports the checker, plants a raw hex into a temp file, asserts the checker catches it, then plants a token-var-only file and asserts the checker passes:

```javascript
// scripts/check-no-raw-hex.test.mjs
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { writeFileSync, mkdtempSync, rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { checkFiles } from "./check-no-raw-hex.mjs";

describe("check-no-raw-hex", () => {
  it("fails on a file containing a raw hex color", () => {
    const dir = mkdtempSync(join(tmpdir(), "hex-test-"));
    const bad = join(dir, "bad.css");
    writeFileSync(bad, ".foo { color: #ff0000; }\n");
    try {
      assert.throws(() => checkFiles([bad]), /raw hex/i);
    } finally {
      rmSync(dir, { recursive: true });
    }
  });

  it("passes on a file using only token vars", () => {
    const dir = mkdtempSync(join(tmpdir(), "hex-test-"));
    const good = join(dir, "good.css");
    writeFileSync(
      good,
      ".foo { color: var(--accent); background: rgba(var(--scrim-rgb), 0.5); }\n",
    );
    try {
      assert.doesNotThrow(() => checkFiles([good]));
    } finally {
      rmSync(dir, { recursive: true });
    }
  });
});
```

This test fails because `check-no-raw-hex.mjs` does not exist yet.

Run to confirm failure:

```bash
cd /tmp/cs-f0 && node --test scripts/check-no-raw-hex.test.mjs 2>&1 | tail -10
```

- [ ] **GREEN** — Create `scripts/check-no-raw-hex.mjs`:

```javascript
#!/usr/bin/env node
// Lint gate: fail if any tracked src/**/*.{css,tsx,ts} file contains a raw hex color.
// Raw hex = a # followed by 3, 4, 6, or 8 hex digits (case-insensitive).
// Exempt: comments, and SVG data-URI strings (which are never in our src/ files).
//
// Invoked from CI (no args — scans git ls-files) or lefthook pre-commit (file paths as args).
import { readFileSync, existsSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join, relative } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// Matches a raw hex color outside of a CSS/JS comment.
// We match #RGB(A) / #RRGGBB(AA) — 3/4/6/8 hex digits.
const HEX_RE = /#(?:[0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})\b/;

// Strip single-line comments before scanning (// and /* … */ and CSS /* … */).
function stripComments(src) {
  // Remove /* ... */ block comments (non-greedy, non-newline-aware is fine for our files)
  src = src.replace(/\/\*[\s\S]*?\*\//g, "");
  // Remove // line comments
  src = src.replace(/\/\/.*/g, "");
  return src;
}

export function checkFiles(files) {
  const hits = [];
  for (const f of files) {
    if (!existsSync(f)) continue;
    const ext = f.replace(/.*\./, "");
    if (!["css", "tsx", "ts"].includes(ext)) continue;
    const src = stripComments(readFileSync(f, "utf8"));
    const lines = src.split("\n");
    for (let i = 0; i < lines.length; i++) {
      if (HEX_RE.test(lines[i])) {
        hits.push({
          file: relative(root, f),
          line: i + 1,
          text: lines[i].trim(),
        });
      }
    }
  }
  if (hits.length > 0) {
    const list = hits.map((h) => `  ${h.file}:${h.line}  ${h.text}`).join("\n");
    throw new Error(
      `raw hex color(s) found — use a cs.* token var instead:\n${list}`,
    );
  }
}

// CLI entry point: no args = scan git ls-files; args = scan those paths.
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  let files;
  if (process.argv.length > 2) {
    files = process.argv.slice(2);
  } else {
    const out = execFileSync(
      "git",
      ["ls-files", "--", "src/**/*.css", "src/**/*.tsx", "src/**/*.ts"],
      {
        cwd: root,
        encoding: "utf8",
      },
    );
    files = out
      .trim()
      .split("\n")
      .filter(Boolean)
      .map((p) => join(root, p));
  }
  try {
    checkFiles(files);
    process.exit(0);
  } catch (err) {
    console.error(`\n✗ check-no-raw-hex: ${err.message}\n`);
    process.exit(1);
  }
}
```

Add to `package.json` scripts:

```json
"check:no-raw-hex": "node scripts/check-no-raw-hex.mjs",
"test:no-raw-hex": "node --test scripts/check-no-raw-hex.test.mjs"
```

Add to `lefthook.yml` pre-commit commands:

```yaml
no-raw-hex:
  glob: "*.{css,tsx,ts}"
  run: node scripts/check-no-raw-hex.mjs {staged_files}
```

Add to `.github/workflows/ci.yml` a new job `no-raw-hex` (after `mcp-config-secret-free`, before `secret-scan`):

```yaml
no-raw-hex:
  name: No raw hex (token-only colors)
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v7
    - uses: actions/setup-node@v7
      with:
        node-version-file: .nvmrc
        cache: npm
    - run: npm ci
    - name: Unit tests (hex lint rules)
      run: npm run test:no-raw-hex
    - name: Scan src/ for raw hex colors
      run: npm run check:no-raw-hex
```

Add `no-raw-hex` to the `ci-required` `needs` array.

Run to confirm pass:

```bash
cd /tmp/cs-f0 && node --test scripts/check-no-raw-hex.test.mjs 2>&1 | tail -10
```

---

## Delivery checklist

- [ ] All four T1–T4 tasks implemented and tests green: `npm test && node --test scripts/check-no-raw-hex.test.mjs`
- [ ] Build passes: `npm run build`
- [ ] Lint clean: `npm run lint && npm run typecheck`
- [ ] Hex gate passes on the whole src/ tree: `npm run check:no-raw-hex`
- [ ] doc-warden conformant: `~/dev/_ops/bin/doc-warden.sh check chaos-scheduler`
- [ ] PR opened as DRAFT, linked to #334 and epic #329
- [ ] Bugbot + security review clean
- [ ] `gh pr ready <n>` — reviewer App squash-merges on green
- [ ] Branch deleted after merge
