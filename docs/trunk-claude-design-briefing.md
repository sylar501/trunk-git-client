**TRUNK**

Claude Design Briefing --- Component Library

*June 2026 · v1.0*

**1. What Trunk Is**

Trunk is a native cross-platform Git GUI client built in Rust + Tauri. It is free and open-source. It targets individual developers, small teams, and large engineering organisations. The primary differentiator is raw performance --- especially on large repositories.

The UI follows a two-panel layout: a narrow left sidebar (workspace/repo/branch navigation) and a full-width graph canvas (the commit graph). All other views --- staging, conflict resolver, interactive rebase, stash manager, tag manager, preferences --- are full-screen takeovers or overlays of this canvas.

**2. What You Are Building**

A complete component library in dark theme. This is not a screen mockup --- it is a design system document showing every reusable UI element in all its states, with exact measurements, spacing, and colour values annotated.

The component library will be the single visual reference for Claude Code when it implements the Tauri frontend. Every component here must be implementable as a CSS custom-property stylesheet.

**2.1 Components to cover (in priority order)**

-   Buttons --- all 5 semantic variants (blue / green / red / amber / neutral) in: default, hover, disabled, and active/pressed states. Also: full-width button variant used in dialogs.

-   Icon buttons --- small square icon-only buttons (used in toolbars and sidebar headers).

-   Text inputs --- default, focused (blue border), error (red border + hint), disabled. Monospaced variant (used for branch names, SHAs, paths).

-   Dropdowns / select --- default, open, focused. Same sizing as text inputs.

-   Toggles --- off state, on state (blue). Transition implied.

-   Checkboxes --- unchecked, checked (blue), partial/indeterminate (dash, used in staging file list).

-   Radio buttons --- unselected, selected.

-   Search / filter inputs --- with leading search icon, clear button on right when non-empty.

-   Sliders --- track, thumb, value label. Used for zoom scale and graph row height.

-   Commit row --- the atomic unit of the graph: graph lane column (SVG lines + dot), branch pills, commit message, author, timestamp, SHA. All in: default, hover, selected states.

-   Branch pills / badges --- all variants: main (blue), feature (green), fix (amber), remote (gray), tag (purple), local-only (amber), pushed (green).

-   Sidebar items --- section header, nav item (default, hover, active with left accent bar), repo row, branch row with colour dot.

-   Diff lines --- add (green bg), delete (red bg), context (neutral). Line number gutter. Gutter staging indicator (filled ● staged, hollow ○ unstaged). Hunk header bar.

-   Info / warning boxes --- blue (informational), amber (caution), green (success), red (destructive). Each with icon + body text.

-   Dialog chrome --- header (icon + title + subtitle), body area, footer (with left-aligned meta text + right-aligned buttons). Sizes: small (400--440px wide) and standard (520px wide).

-   Tabs --- active, inactive, hover. Used in scope selectors (command palette, clone dialog steps).

-   Context menus --- item (default, hover, danger variant), separator.

-   Toasts / transient notifications --- success, warning, info. Short-lived, appear bottom-centre.

-   Empty state card --- centred card with icon, title, description, action buttons, drag hint.

-   Progress / terminal log --- dark terminal-style log area used in clone progress and rebase execution.

**3. Dark Theme Palette**

Use these exact hex values. Do not deviate. Every component must use tokens from this palette --- no arbitrary colours.

**3.1 Backgrounds --- 5 depth levels**

+--------------------------------------------------------------------------+--------------------------------------------------------------------------+
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
| |        | **\--bg-base**                                              | | |        | **\--bg-primary**                                           | |
| |        |                                                             | | |        |                                                             | |
| |        | #0D1117                                                     | | |        | #161B22                                                     | |
| |        |                                                             | | |        |                                                             | |
| |        | Titlebar, window chrome, terminal bg                        | | |        | Graph canvas, main content area, sidebar                    | |
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
+--------------------------------------------------------------------------+--------------------------------------------------------------------------+
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
| |        | **\--bg-secondary**                                         | | |        | **\--bg-tertiary**                                          | |
| |        |                                                             | | |        |                                                             | |
| |        | #1C2128                                                     | | |        | #22272E                                                     | |
| |        |                                                             | | |        |                                                             | |
| |        | Cards, setting groups, overlay panels                       | | |        | Inputs, dropdowns, toolbar buttons                          | |
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
+--------------------------------------------------------------------------+--------------------------------------------------------------------------+
| +--------+-------------------------------------------------------------+ |                                                                          |
| |        | **\--bg-raised**                                            | |                                                                          |
| |        |                                                             | |                                                                          |
| |        | #2D333B                                                     | |                                                                          |
| |        |                                                             | |                                                                          |
| |        | Hover states, toggle tracks, kbd badges                     | |                                                                          |
| +--------+-------------------------------------------------------------+ |                                                                          |
+--------------------------------------------------------------------------+--------------------------------------------------------------------------+

**3.2 Borders**

+--------------------------------------------------------------------------+--------------------------------------------------------------------------+
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
| |        | **\--border-subtle**                                        | | |        | **\--border-default**                                       | |
| |        |                                                             | | |        |                                                             | |
| |        | #30363D                                                     | | |        | #444C56                                                     | |
| |        |                                                             | | |        |                                                             | |
| |        | Default dividers, card edges, section separators            | | |        | Inputs, controls, interactive element borders               | |
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
+--------------------------------------------------------------------------+--------------------------------------------------------------------------+

**3.3 Text --- 4 levels**

+--------------------------------------------------------------------------+--------------------------------------------------------------------------+
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
| |        | **\--text-primary**                                         | | |        | **\--text-secondary**                                       | |
| |        |                                                             | | |        |                                                             | |
| |        | #CDD9E5                                                     | | |        | #768390                                                     | |
| |        |                                                             | | |        |                                                             | |
| |        | Main labels, commit messages, values                        | | |        | Descriptions, metadata, timestamps                          | |
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
+--------------------------------------------------------------------------+--------------------------------------------------------------------------+
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
| |        | **\--text-tertiary**                                        | | |        | **\--text-dim**                                             | |
| |        |                                                             | | |        |                                                             | |
| |        | #444C56                                                     | | |        | #2D333B                                                     | |
| |        |                                                             | | |        |                                                             | |
| |        | Section labels, placeholders, hints                         | | |        | Disabled states, ghost text                                 | |
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
+--------------------------------------------------------------------------+--------------------------------------------------------------------------+

**3.4 Semantic accent colours (foreground + background pairs)**

Each accent has a foreground token (used for text, icons, borders) and a background token (deeply tinted, used for info boxes, selected rows, button fills).

+--------------------------------------------------------------------------+--------------------------------------------------------------------------+
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
| |        | **\--blue**                                                 | | |        | **\--blue-bg**                                              | |
| |        |                                                             | | |        |                                                             | |
| |        | #539BF5                                                     | | |        | #1C2A3A                                                     | |
| |        |                                                             | | |        |                                                             | |
| |        | Primary / constructive actions, selection, HEAD lane        | | |        | Blue-tinted card backgrounds, selected rows                 | |
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
+--------------------------------------------------------------------------+--------------------------------------------------------------------------+
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
| |        | **\--green**                                                | | |        | **\--green-bg**                                             | |
| |        |                                                             | | |        |                                                             | |
| |        | #57AB5A                                                     | | |        | #1B2A1F                                                     | |
| |        |                                                             | | |        |                                                             | |
| |        | Additive / stage actions, diff additions, success           | | |        | Green-tinted card backgrounds                               | |
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
+--------------------------------------------------------------------------+--------------------------------------------------------------------------+
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
| |        | **\--red**                                                  | | |        | **\--red-bg**                                               | |
| |        |                                                             | | |        |                                                             | |
| |        | #E5534B                                                     | | |        | #2D1B1B                                                     | |
| |        |                                                             | | |        |                                                             | |
| |        | Destructive actions, diff deletions, danger state           | | |        | Red-tinted card backgrounds                                 | |
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
+--------------------------------------------------------------------------+--------------------------------------------------------------------------+
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
| |        | **\--amber**                                                | | |        | **\--amber-bg**                                             | |
| |        |                                                             | | |        |                                                             | |
| |        | #C69026                                                     | | |        | #2D2415                                                     | |
| |        |                                                             | | |        |                                                             | |
| |        | Caution / reversible actions, local-only badges, warnings   | | |        | Amber-tinted card backgrounds                               | |
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
+--------------------------------------------------------------------------+--------------------------------------------------------------------------+
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
| |        | **\--purple**                                               | | |        | **\--purple-bg**                                            | |
| |        |                                                             | | |        |                                                             | |
| |        | #986EE2                                                     | | |        | #261E3A                                                     | |
| |        |                                                             | | |        |                                                             | |
| |        | Annotated tags, workspaces, branch lane 4                   | | |        | Purple-tinted card backgrounds                              | |
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
+--------------------------------------------------------------------------+--------------------------------------------------------------------------+

**3.5 Graph branch lane colours**

Seven lanes at similar perceptual brightness. No lane should visually dominate.

+--------------------------------------------------------------------------+--------------------------------------------------------------------------+
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
| |        | **\--lane-1**                                               | | |        | **\--lane-2**                                               | |
| |        |                                                             | | |        |                                                             | |
| |        | #539BF5                                                     | | |        | #57AB5A                                                     | |
| |        |                                                             | | |        |                                                             | |
| |        | Blue (HEAD / main branch)                                   | | |        | Green                                                       | |
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
+--------------------------------------------------------------------------+--------------------------------------------------------------------------+
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
| |        | **\--lane-3**                                               | | |        | **\--lane-4**                                               | |
| |        |                                                             | | |        |                                                             | |
| |        | #C69026                                                     | | |        | #986EE2                                                     | |
| |        |                                                             | | |        |                                                             | |
| |        | Amber                                                       | | |        | Purple                                                      | |
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
+--------------------------------------------------------------------------+--------------------------------------------------------------------------+
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
| |        | **\--lane-5**                                               | | |        | **\--lane-6**                                               | |
| |        |                                                             | | |        |                                                             | |
| |        | #D4537E                                                     | | |        | #39C5CF                                                     | |
| |        |                                                             | | |        |                                                             | |
| |        | Pink                                                        | | |        | Teal                                                        | |
| +--------+-------------------------------------------------------------+ | +--------+-------------------------------------------------------------+ |
+--------------------------------------------------------------------------+--------------------------------------------------------------------------+
| +--------+-------------------------------------------------------------+ |                                                                          |
| |        | **\--lane-7**                                               | |                                                                          |
| |        |                                                             | |                                                                          |
| |        | #D08444                                                     | |                                                                          |
| |        |                                                             | |                                                                          |
| |        | Orange                                                      | |                                                                          |
| +--------+-------------------------------------------------------------+ |                                                                          |
+--------------------------------------------------------------------------+--------------------------------------------------------------------------+

**4. Button Colour Semantics**

This is the most critical system to get right. Colour communicates the nature of an action, not aesthetics. Every button in the application must follow this system.

  ------------------ -------------- ------------ --------------------------------------------------- -------------------------------------------
  **Colour**         **Solid bg**   **Border**   **Semantic meaning**                                **Examples**

  **Blue**           #185FA5        #0C447C      Primary / most important constructive action        *Commit, Begin rebase, Continue merge*

  **Blue outline**   #E6F1FB        #378ADD      Secondary constructive, related to primary          *Amend last commit*

  **Green**          #3B6D11        #27500A      Additive / staging action                           *Stage all, Stage hunk, Create branch*

  **Amber**          #FAEEDA        #EF9F27      Reversible caution --- changes state but undoable   *Stash, Unstage hunk, Rename*

  **Red**            #FCEBEB        #E24B4A      Destructive / irreversible --- data loss possible   *Discard all, Drop commit, Delete branch*

  **Neutral**        #F5F4F0        #C8C6BE      Navigation, settings, secondary actions             *Cancel, Back, Settings, Copy SHA*
  ------------------ -------------- ------------ --------------------------------------------------- -------------------------------------------

Dynamic colour flipping: \"Stage hunk\" is green. Once staged, the same button reads \"Unstage hunk\" in amber. The colour always describes what will happen on click, not what already happened.

Disabled state: always gray (#ECEAE3 bg, #C8C6BE border, #888780 text) regardless of semantic colour.

**5. Typography**

  --------------------------- ------------------- --------------- -------------------------------
  **Usage**                   **Font**            **Size**        **Colour token**

  View / dialog title         System sans-serif   14px / 13px     \--text-primary

  Section headings            System sans-serif   9px uppercase   \--text-tertiary (caps label)

  Body / setting labels       System sans-serif   11px            \--text-primary

  Descriptions / meta         System sans-serif   10px            \--text-secondary

  Hints / placeholders        System sans-serif   9--10px         \--text-tertiary

  Branch names, SHAs, paths   System monospace    11px            \--text-primary

  Diff code                   System monospace    11px            see diff tokens

  Terminal / log output       System monospace    10px            #CDD9E5 on #0D1117
  --------------------------- ------------------- --------------- -------------------------------

**6. Spacing & Sizing Conventions**

-   Base spacing unit: 4px. All spacing should be multiples of 4.

-   Sidebar width: 156px fixed.

-   Graph lane column width: 64px.

-   Toolbar height: 32px (titlebar) or 34px (graph toolbar).

-   Commit row height: 30px default (range 22--36px, user-configurable).

-   Dialog widths: small = 400--440px, standard = 520px, wide = 660px (rebase).

-   Button height: 22px (toolbar), 26px (dialog), 28px (primary footer CTA).

-   Input height: 22px (small), 28px (standard form field).

-   Border radius: 4px (small controls), 5px (inputs/buttons), 8px (cards/dialogs), 10px (app window).

-   Icon size in buttons: 12--13px.

-   Section label caps text: 9px, uppercase, letter-spacing 0.07em.

**7. dark.css Token Reference**

The complete dark.css file that ships with Trunk. Custom theme authors override these tokens.

+-----------------------------------------------------------------------+
| /\* trunk --- dark.css \| Official dark theme \*/                     |
|                                                                       |
| /\* Edit this file to customise. Hot-reloads on save. \*/             |
|                                                                       |
| :root {                                                               |
|                                                                       |
| /\* ── backgrounds ──────────────────────────── \*/                   |
|                                                                       |
| \--bg-base: #0D1117; /\* titlebar, terminal \*/                       |
|                                                                       |
| \--bg-primary: #161B22; /\* graph canvas, sidebar \*/                 |
|                                                                       |
| \--bg-secondary: #1C2128; /\* cards, panels \*/                       |
|                                                                       |
| \--bg-tertiary: #22272E; /\* inputs, dropdowns \*/                    |
|                                                                       |
| \--bg-raised: #2D333B; /\* hover, toggle track \*/                    |
|                                                                       |
| /\* ── borders ──────────────────────────────── \*/                   |
|                                                                       |
| \--border-subtle: #30363D;                                            |
|                                                                       |
| \--border-default: #444C56;                                           |
|                                                                       |
| /\* ── text ─────────────────────────────────── \*/                   |
|                                                                       |
| \--text-primary: #CDD9E5;                                             |
|                                                                       |
| \--text-secondary: #768390;                                           |
|                                                                       |
| \--text-tertiary: #444C56;                                            |
|                                                                       |
| \--text-dim: #2D333B;                                                 |
|                                                                       |
| /\* ── semantic accents ──────────────────────── \*/                  |
|                                                                       |
| \--blue: #539BF5; /\* primary / constructive \*/                      |
|                                                                       |
| \--blue-bg: #1C2A3A;                                                  |
|                                                                       |
| \--green: #57AB5A; /\* additive / stage \*/                           |
|                                                                       |
| \--green-bg: #1B2A1F;                                                 |
|                                                                       |
| \--red: #E5534B; /\* destructive \*/                                  |
|                                                                       |
| \--red-bg: #2D1B1B;                                                   |
|                                                                       |
| \--amber: #C69026; /\* caution / reversible \*/                       |
|                                                                       |
| \--amber-bg: #2D2415;                                                 |
|                                                                       |
| \--purple: #986EE2; /\* tags / workspaces \*/                         |
|                                                                       |
| \--purple-bg: #261E3A;                                                |
|                                                                       |
| \--pink: #D4537E;                                                     |
|                                                                       |
| \--teal: #39C5CF;                                                     |
|                                                                       |
| \--orange: #D08444;                                                   |
|                                                                       |
| /\* ── graph branch lanes ────────────────────── \*/                  |
|                                                                       |
| \--lane-1: #539BF5; /\* blue \*/                                      |
|                                                                       |
| \--lane-2: #57AB5A; /\* green \*/                                     |
|                                                                       |
| \--lane-3: #C69026; /\* amber \*/                                     |
|                                                                       |
| \--lane-4: #986EE2; /\* purple \*/                                    |
|                                                                       |
| \--lane-5: #D4537E; /\* pink \*/                                      |
|                                                                       |
| \--lane-6: #39C5CF; /\* teal \*/                                      |
|                                                                       |
| \--lane-7: #D08444; /\* orange \*/                                    |
|                                                                       |
| }                                                                     |
+-----------------------------------------------------------------------+

**8. Optimised Prompt for Claude Design**

Copy and paste this prompt exactly into Claude Design, along with the trunk-requirements.docx file and 2--3 screenshots from the conversation mockups (preferences screen, staging view, main graph view recommended).

+-----------------------------------------------------------------------------------------+
| You are designing a component library for Trunk, a native cross-platform Git GUI client |
|                                                                                         |
| (Rust + Tauri). The attached briefing document contains the complete design system      |
|                                                                                         |
| specification. The attached PRD (trunk-requirements.docx) contains full feature specs.  |
|                                                                                         |
| Your task: produce a single-page dark-theme component library covering every reusable   |
|                                                                                         |
| UI element. This is not a screen mockup --- it is a design system reference sheet.      |
|                                                                                         |
| Use ONLY the colour tokens defined in §3 of the briefing (dark.css palette). Do not     |
|                                                                                         |
| introduce any colours not in that palette.                                              |
|                                                                                         |
| Components to produce (in this order):                                                  |
|                                                                                         |
| 1\. BUTTONS --- all 5 semantic variants (blue/green/red/amber/neutral) × 4 states       |
|                                                                                         |
| (default, hover, disabled, active). Also: full-width dialog button variant.             |
|                                                                                         |
| See §4 of the briefing for exact hex values and semantic meanings.                      |
|                                                                                         |
| 2\. ICON BUTTONS --- small 22px square icon-only buttons used in toolbars.              |
|                                                                                         |
| 3\. INPUTS --- text input (default, focused, error, disabled), monospaced variant,      |
|                                                                                         |
| search input with leading icon and clear button.                                        |
|                                                                                         |
| 4\. FORM CONTROLS --- dropdown/select, toggle (off/on), checkbox (unchecked/checked/    |
|                                                                                         |
| indeterminate), radio button (unselected/selected), slider with value label.            |
|                                                                                         |
| 5\. COMMIT ROW --- the atomic unit of the graph canvas. Show: graph lane column         |
|                                                                                         |
| (SVG vertical line + circle node), branch pills, commit message, author,                |
|                                                                                         |
| timestamp, SHA --- in default, hover, and selected states.                              |
|                                                                                         |
| 6\. BRANCH PILLS / BADGES --- all variants: main (blue), feature (green), fix (amber),  |
|                                                                                         |
| remote (gray), tag (purple), local-only (amber), pushed (green).                        |
|                                                                                         |
| 7\. SIDEBAR ITEMS --- section header with + button, nav item (default/hover/active      |
|                                                                                         |
| with left 2px blue accent bar), repo row, branch row with colour dot.                   |
|                                                                                         |
| 8\. DIFF LINES --- add line (green), delete line (red), context line (neutral).         |
|                                                                                         |
| Show: line number gutter, sign column, staging gutter (● staged / ○ unstaged),          |
|                                                                                         |
| code column. Also: hunk header bar.                                                     |
|                                                                                         |
| 9\. INFO / WARNING BOXES --- blue (info), amber (caution), green (success),             |
|                                                                                         |
| red (danger) --- each with icon + body text.                                            |
|                                                                                         |
| 10\. DIALOG CHROME --- header (icon chip + title + subtitle), body area, footer.        |
|                                                                                         |
| Show small (440px) and standard (520px) width variants.                                 |
|                                                                                         |
| 11\. CONTEXT MENU --- default item, hover item, danger item, separator.                 |
|                                                                                         |
| 12\. TOAST NOTIFICATIONS --- success, warning, info --- centred at bottom of screen.    |
|                                                                                         |
| 13\. EMPTY STATE CARD --- icon wrap, title, description, action buttons, drag hint.     |
|                                                                                         |
| Used when workspace has no repositories.                                                |
|                                                                                         |
| 14\. TERMINAL LOG AREA --- dark (#0D1117) background, monospace text, cursor blink.     |
|                                                                                         |
| Used in clone progress and rebase execution.                                            |
|                                                                                         |
| Spacing rules (§6 of briefing):                                                         |
|                                                                                         |
| \- Base unit: 4px. All spacing multiples of 4.                                          |
|                                                                                         |
| \- Button height: 22px toolbar, 26px dialog, 28px primary CTA.                          |
|                                                                                         |
| \- Input height: 22px small, 28px standard.                                             |
|                                                                                         |
| \- Border radius: 4px controls, 5px inputs/buttons, 8px cards, 10px window.             |
|                                                                                         |
| \- Annotate spacing and sizes on the component sheet.                                   |
|                                                                                         |
| Typography (§5 of briefing):                                                            |
|                                                                                         |
| \- UI labels: system sans-serif, 11px, \--text-primary.                                 |
|                                                                                         |
| \- Descriptions: 10px, \--text-secondary.                                               |
|                                                                                         |
| \- Section caps labels: 9px uppercase, letter-spacing 0.07em, \--text-tertiary.         |
|                                                                                         |
| \- Branch names, SHAs, paths: system monospace, 11px.                                   |
|                                                                                         |
| Output format: a single design canvas showing all components grouped by category,       |
|                                                                                         |
| with labels and state annotations. Dark theme throughout. No light theme version.       |
|                                                                                         |
| Include the colour token name (e.g. \--blue, \--bg-secondary) as annotations            |
|                                                                                         |
| wherever a colour is used, so the sheet doubles as an implementation reference.         |
+-----------------------------------------------------------------------------------------+

**9. Screens Reference (for context)**

The following screens have been designed in this project. Claude Design should be aware of them for consistency, but is NOT being asked to redesign them --- the component library is the deliverable.

  ------------------------- ----------------------------------------------------------------------------------------------------------
  **Screen**                **Key notes**

  **Welcome screen**        Two primary action buttons (Open / Clone), recent elements list, \"Create empty workspace\" text link

  **Main graph view**       2-panel: narrow sidebar + full-width commit graph canvas. Commit detail slides in as overlay from right.

  **Staging view**          3-panel: file list (left) + hunk diff (centre) + commit panel (right). Replaces graph.

  **Conflict resolver**     3-panel: ours / base / theirs + merged output preview below. Full-screen.

  **Interactive rebase**    Full-screen takeover: commit list (left) + after-state preview (right).

  **Command palette**       Overlay on dimmed app. Search input + scope tabs + result list with keyboard shortcuts.

  **Stash manager**         2-panel: stash list + detail (metadata, file list, diff). Full-screen.

  **Tag manager**           2-panel: searchable tag list + detail. Group-by dropdown with configurable regex.

  **Branch dialogs**        Create (green), Switch (blue), Rename (amber), Delete (red). Small dialogs.

  **Push / Fetch / Pull**   Small modal dialogs. Push shows commit preview. Pull shows integration strategy radios.

  **Preferences**           2-panel: left category nav + right settings content. 10 categories.

  **Empty workspace**       Centred card on dot-grid canvas: Add repo + Clone repo + drag hint.
  ------------------------- ----------------------------------------------------------------------------------------------------------

*Trunk Component Library Briefing --- June 2026*
