# Effects level (US-094) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the "Reduce motion" checkbox with a Full / Reduced / Off effects level that every animation honours.

**Architecture:** `<body data-effects="full|reduced|off">` is the single switch. A small pure resolver (`effects.js`) decides the level from the stored choice, the legacy key and `prefers-reduced-motion`; `app.js` applies it and reads it through `effectsLevel()`; `styles.css` has explicit per-level rules per animation family.

**Tech Stack:** Vanilla JS and CSS served from `crates/beelot-server/static/`; Node (local only) for the resolver's unit test; the browser tools for visual checks.

**Spec:** `docs/superpowers/specs/2026-10-31-effects-level-design.md`

## Global Constraints

- Levels are exactly `full`, `reduced`, `off`. Storage key `beelot.effects`; legacy key `beelot.reduced-motion` is read once and removed when the player picks a level.
- Unset or invalid stored choice: follow `prefers-reduced-motion` (Reduced if set, else Full), live.
- Reduced: movement becomes a fade of at most 150 ms; loops stop. Off: no animation, no transition.
- Game pacing delays (`setTimeout` in `app.js`) are unchanged at every level.
- Animate only `transform`, `translate`, `scale`, `opacity`.
- New texts are translated into French (`i18n.js`) and Dutch (`i18n-nl.js`).
- No server change, no new dependency. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` stay green.

## Review Focus

- Storage unavailable (private window, `localStorage` throws): the level still resolves and Settings still works for the session.
- `beelot.effects` holds garbage (`"banana"`): treated as unset.
- Both keys present: `beelot.effects` wins over the legacy key.
- System `prefers-reduced-motion` flips while the page is open with no stored choice: the level follows; with a stored choice: it does not.
- Switching to Off mid-animation (a trick being collected): the table ends in a correct static state, with no card left half-moved.

---

### Task 1: Level resolver with a unit test

**Files:**
- Create: `crates/beelot-server/static/effects.js`
- Create: `crates/beelot-server/frontend-tests/effects.test.cjs`
- Modify: `crates/beelot-server/static/index.html:566-569` (load `/effects.js` before `/app.js`)

**Interfaces:**
- Produces: global `resolveEffectsLevel(stored: string|null, legacyReducedMotion: string|null, prefersReduced: boolean): "full"|"reduced"|"off"`; also `module.exports = { resolveEffectsLevel }` when `module` exists.

- [ ] **Step 1: Write the failing test** in `effects.test.cjs` using `node:test` and `node:assert`, requiring `../static/effects.js`. Cases, each an assertion on `resolveEffectsLevel`:
  - `("off", null, false)` → `"off"`; `("reduced", null, false)` → `"reduced"`; `("full", null, true)` → `"full"` (a stored choice beats the system).
  - `(null, null, false)` → `"full"`; `(null, null, true)` → `"reduced"`.
  - `("banana", null, true)` → `"reduced"` and `("banana", null, false)` → `"full"` (garbage is unset).
  - `(null, "true", false)` → `"reduced"` (legacy migration); `(null, "false", false)` → `"full"`.
  - `("full", "true", false)` → `"full"` (new key wins over legacy).
- [ ] **Step 2: Run** `node --test crates/beelot-server/frontend-tests/` — expect FAIL (`Cannot find module`).
- [ ] **Step 3: Implement `resolveEffectsLevel`** in `effects.js` as a plain function declaration (so it is a browser global) with the guarded `module.exports`.
- [ ] **Step 4: Run** the same command — expect all PASS. Add `<script src="/effects.js"></script>` before `/app.js` in `index.html`, then `node --check crates/beelot-server/static/effects.js`.
- [ ] **Step 5: Commit** `feat: resolve the effects level from the stored choice and the system (US-094)`.

### Task 2: Settings control and the `data-effects` switch

**Files:**
- Modify: `crates/beelot-server/static/index.html:533` (replace the `reduced-motion` checkbox)
- Modify: `crates/beelot-server/static/app.js:1362-1365` (`motionReduced`), `app.js:1961-1976` (`applySettings`, the checkbox listener)
- Modify: `crates/beelot-server/static/i18n.js`, `crates/beelot-server/static/i18n-nl.js`

**Interfaces:**
- Consumes: `resolveEffectsLevel` (Task 1); `choiceInputs`, `setChoice` (`app.js:62-73`).
- Produces: `effectsLevel(): "full"|"reduced"|"off"` reading `document.body.dataset.effects`; `motionReduced()` removed and its five callers (`app.js:970`, `1194`, `1580` and any others found by `grep -n motionReduced`) now call `effectsLevel() !== "full"`.

- [ ] **Step 1: Replace the checkbox** in `index.html` with a `setting-choice` row in the Language style: a `choice-group` `role="radiogroup"` `aria-label="Effects"`, three `input name="effects"` radios with values `full`, `reduced`, `off` and labels "Full", "Reduced", "Off", title "Effects", small text "Choose how much the table moves."
- [ ] **Step 2: Implement `effectsLevel()` and rework `applySettings()`**: read both keys inside `try/catch` (storage may throw, then use `null`), call `resolveEffectsLevel`, set `document.body.dataset.effects`, remove the `reduced-motion` class logic, and `setChoice("effects", level)`. Remove the old `#reduced-motion` listener.
- [ ] **Step 3: Add the change handler** on `choiceInputs("effects")`: store the value under `beelot.effects` (try/catch, same pattern as the language setting), remove `beelot.reduced-motion`, call `applySettings()`. Register a `matchMedia("(prefers-reduced-motion: reduce)")` `change` listener that calls `applySettings()`.
- [ ] **Step 4: Texts.** Check existing keys with `grep -n '"Full"\|"Off"\|"Reduced"\|"Effects"' i18n*.js`. Add FR/NL entries for "Effects", "Full", "Reduced", "Off", "Choose how much the table moves."; remove "Reduce motion" and "Disable movement and transition effects." from both files.
- [ ] **Step 5: Verify** `node --check` on `app.js`, `i18n.js`, `i18n-nl.js`; `node --test crates/beelot-server/frontend-tests/`; run the server (`docker compose up -d --wait && cargo run -p beelot-server`) and in the browser check: Settings shows three radios in EN/FR/NL; choosing Off sets `document.body.dataset.effects === "off"`, survives reload and leaves "Sound effects" untouched; with only `localStorage["beelot.reduced-motion"]="true"` the radio shows Reduced; with `localStorage.setItem` blocked (override in the console) the radio still changes the level for the session.
- [ ] **Step 6: Commit** `feat: choose the effects level in Settings (US-094)`.

### Task 3: Per-level rules for every animation family

**Files:**
- Modify: `crates/beelot-server/static/styles.css` (remove the `.reduced-motion` block at `:1108-1113`; add the `data-effects` rules; add `@keyframes fade-in`)

**Interfaces:**
- Consumes: `body[data-effects]` from Task 2.
- Produces: custom properties `--effect-fast: 150ms` (Reduced) / `0s` (Off) on `body`, and the documented pattern that later stories follow: a movement keyframe gets a Reduced rule mapping it to `fade-in` and an Off rule with `animation: none`; a loop is declared under `body[data-effects="full"]`.

- [ ] **Step 1: Add `@keyframes fade-in { from { opacity: 0; } }`** and, under `body[data-effects="reduced"]`, map to `animation: fade-in var(--effect-fast) both` (keeping `animation-delay`): `.playing-card.dealt-in` (`deal-in`), `.current-trick .playing-card.card-arriving` (`card-drop`), `.arriving-late` (`fade-up`), `.table-hand .playing-card` (`hand-in`), `.player-call.call-arriving` and `.player-call[data-kind="coinche"]` (`call-pop`, `call-shake`), `.reaction-bubble` (`reaction-pop`), `.trick-collecting .trick-slot` (`trick-collect`, fade only, no translate).
- [ ] **Step 2: Stop loops at Reduced and Off**: `.hero-suits span` (three `float-*`), `.player-station.deciding` (keep the gold border), `.player-thinking i` (dots stay visible at full opacity), `.auction-waiting-view .active-player .player-avatar`, `.trick-winner .winning-card` (keep the border and shadow, drop the pulse), `.live-dot`, `.player-station.call-flash` (`call-ripple`).
- [ ] **Step 3: Off** — `body[data-effects="off"] *, ::before, ::after { animation: none !important; transition: none !important; }`. Reduced — `transition-duration: var(--effect-fast) !important` on the same selector. Delete the old blanket `.reduced-motion` rules.
- [ ] **Step 4: Verify in the browser** (bot game, laptop 1280×800 and phone 390×844 via `resize_page`) at each level: deal, a bid with a call badge, a card played and its trick collected, a hint, the end of a round. Full: as before the change. Reduced: cards fade in, nothing travels, active player still marked. Off: nothing animates and no card is left half-moved, including when switching to Off in Settings while a trick is on view and returning to the table. Also emulate `prefers-reduced-motion: reduce` with no stored choice and confirm Reduced. Check one private and one pass-and-play table. Record a performance trace at 4× CPU throttle at Full and confirm no layout-triggering animation (only transform/opacity).
- [ ] **Step 5: Commit** `feat: honour the effects level in every animation (US-094)`.

### Task 4: Backlog, README and final checks

**Files:**
- Modify: `README.md` (if it lists Settings; otherwise skip), `docs/product-backlog.md` (US-094 note: delivered, and the pattern for later stories)

- [ ] **Step 1: Add to the US-094 entry** one line that later stories follow the patterns in `docs/superpowers/specs/2026-10-31-effects-level-design.md`.
- [ ] **Step 2: Run** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` (Docker running) — expect all green; `node --test crates/beelot-server/frontend-tests/` PASS.
- [ ] **Step 3: Commit** `docs: mark US-094 delivered and point later stories at the effects patterns`; report commands run and anything not run.
