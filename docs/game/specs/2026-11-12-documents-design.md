# Documents, Graybox online and signatures (G13–G15) — design

Owner's request (2026-11-12): a **Documents** part of Graybox, half Figma and half Notion, for
written documentation in general (decisions, specs, meeting notes) with a **signing mode**: a
version signed by Adriel and the client is closed for good; a change means a new version that is
signed again. The client signs through a **link**, from anywhere in the world. The same kind of link
lets the client play the prototype, comment, read the documents and download them.

## Decisions taken (owner, 2026-11-12)
| # | Decision |
|---|---|
| P1 | The client signs **in the browser through a link**, drawing with a brush or typing their name in a handwriting font. The signature records the server's date and time, an id, the approximate place and a fingerprint of the version. |
| P2 | Documents live **inside the project file** (`.figcraft`), like components. Templates are available in every file. |
| P3 | A document dragged onto the canvas is a **card that follows the original**, like an instance. A signed version shows frozen, with a seal. |
| P4 | A **Documents** tab next to File and Assets lists the documents as a tree (Notion-like). Opening one puts the editor where the canvas is. Each block has a **handle on its left**: clicking it turns the block into anything (title, list, checklist, link, image, shape, variant, colour, any Graybox thing that can be linked); `/` inserts blocks; drag the handle to move. |
| P5 | All extras: (a) decision block and decision log, (b) diff between versions, (c) templates, (d) per-section approval, (e) automatic tables from components and variables, (f) Claude writes documents, (g) PDF and Markdown export. |
| P6 | **Open**: where Graybox is hosted online (rented server and domain). The owner decides later. Everything before the deploy is built and tested on a local server. |
| P7 | The client confirms their identity with a **6-digit code sent to their email**. No CPF or other ID numbers are stored. |
| P8 | Date and time come from the server. The place is city and country from the IP address, with no prompt. The IP is kept only in the audit trail. |
| P9 | Adriel signs first (closing a version is his signature); then the client's signers sign in any order. The version is "Signed" when everyone has signed. (Chosen by the orchestrator: the owner had no preference.) |
| P10 | Through a link the client can view, play the prototype, comment, read and download the documents, and sign. No editing. Links can have a password and an expiry date and can be revoked. |

Legal note (not legal advice): Brazilian law (Lei 14.063/2020, MP 2.200-2/2001 art. 10 §2) accepts
electronic signatures agreed between private parties. The audit trail makes it stronger. The owner
should have a lawyer look at the contract template once.

## What exists upstream (reuse, don't rebuild)
- `apps/figcraft-server`: a standalone server that keeps files and serves **share links** (edit,
  view, comment), join codes, owner tokens, with hashed secrets, budgets and hardening tests. See
  `docs/server.md` (it includes how to deploy on Linux and `web_url` for the browser build).
- `apps/figcraft-web`: the browser build. The game layer must already run there (wasm rule).
- Comments, the text engine (`crates/text`), PDF export (`crates/pdf`), the SVG and image import.
- The public FigCraft server (`collab.craftapps.dev`) **must not be used**: its terms say not to put
  confidential material there. Graybox gets its own server (P6).

## Architecture
- **Model** (L1, Graybox-owned file): `Document { id, title, icon, parent, children order, blocks,
  versions }`. A block is `{ id, kind, text runs, style, children }`. Kinds: heading 1–4, paragraph,
  bulleted, numbered, checklist, toggle, callout, quote, divider, table, link, image, shape, colour
  swatch, embed (a node id: frame, component, variant, with "live" or a pinned snapshot), decision,
  auto-table, signature slot. Text runs carry bold, italic, underline, strike, code, colour,
  highlight, link, mention (`@layer`, `@document`). Each block can set a font family and size.
- Stored in the `.figcraft` file in a Graybox-owned section with serde defaults, so files without
  documents load unchanged and upstream FigCraft ignores it. Upstream doc-model files are touched only
  at one named hook (`graybox-hooks` skill).
- **Everything is a command** (`doc.create`, `doc.rename`, `doc.move`, `doc.delete`,
  `doc.block.insert`, `doc.block.set`, `doc.block.turn_into`, `doc.block.move`, `doc.block.delete`,
  `doc.version.close`, `doc.version.new`, `doc.export`…), undoable, in the junk-parameter sweep, so the
  menus, Actions palette, CLI, MCP (Claude) and control channel all reach them.
- **One layout for everything**: a function lays a document version out as pages, then reuses that
  layout for the canvas card (frames and text layers made from it), the PDF (through the existing
  PDF export), the Markdown export and the web view. What the client signs is exactly what Adriel
  sees.
- **Closing a version**: the version's blocks, with embedded images and pinned snapshots of live
  embeds, are frozen. A **SHA-256 fingerprint** of their canonical JSON is shown as a short code (for
  example `GBX-7Q4M-2KD9`) on the document and in every PDF page footer. A closed version can never be
  edited: the commands refuse it, and the UI shows it read-only. Changing it means `doc.version.new`
  (v2, a copy of v1 to edit). If the file is edited by hand, the fingerprint no longer matches and
  Graybox shows "This version was changed outside Graybox".

## G13 — Documents in Graybox (local, no server)
One session per phase. Each phase ends committed and checked in the real app.
1. **G13.1 Model and Documents tab**:
   - the model, the commands, save and load, and the junk sweep;
   - the **Documents** tab in the left panel (hook: `LeftTab`, `panels/layers.rs` tab row);
   - the document tree: create, rename, nest, reorder by drag, delete, icon;
   - opening a document shows a basic editor in place of the canvas.
2. **G13.2 Block editor (Notion-like)**:
   - the left handle, with *Turn into*, *Duplicate*, *Delete*, *Colour* and *Font*, dragged to move;
   - the `/` menu with search;
   - Markdown shortcuts (`#`, `-`, `[]`, `>`, `---`);
   - Enter and Backspace behave like Notion;
   - inline formatting with a small floating bar;
   - mentions and links;
   - images (paste and drop), shapes and colour swatches;
   - live embeds of frames, components and variants (pick from the file);
   - tables;
   - multi-block selection, copy and paste;
   - undo per edit.
   Design-heavy: an Opus session, compared with Notion by observation.
3. **G13.3 Pages, canvas card and export**:
   - the page layout;
   - dragging a document from the tree onto the canvas makes a card that follows the document;
     a closed version's card is frozen with its seal;
   - PDF and Markdown export.
4. **G13.4 Versions and decisions**:
   - Close version (fingerprint, read-only) and New version;
   - the version history;
   - **diff** between two versions (added, removed, changed blocks, side by side and inline);
   - the **decision block** (what, when, who, why);
   - the decision log (every decision in the file, linking back).
5. **G13.5 Templates, Claude and automation**:
   - templates (proposal and scope, meeting notes, HUD spec, Unity handoff, change request);
   - MCP tools so Claude reads and writes documents from the Graybox chat ("make the meeting notes",
     "summarise the decisions");
   - **auto-tables** (screens, components with their variants and states, variables);
   - **per-section approval** (a section is marked approved, with who and when).

## G14 — Graybox online (needs P6 for the deploy, not for the build)
1. **G14.1 Links that open in the browser** (local server first): Adriel publishes the file to
   *his* server and gets client links:
   - **View and play**: the prototype opens in the browser build and Play runs the game, keyboard,
     mouse and controller;
   - **Comment**: the existing comment link;
   - **Documents**: read every document, download the PDF.
   Links can have a password and an expiry date, and can be revoked (extend the server's link model,
   upstream hook rules apply). Test end to end against `figcraft-server` on localhost.
2. **G14.2 Deploy**, once the owner has a server and a domain:
   - HTTPS (Caddy or a similar reverse proxy);
   - `figcraft-server` as a service, and the browser build served at `web_url`;
   - backups of the data folder;
   - a short runbook in `docs/game/`;
   - new terms and privacy text for *his* server (confidential client work, LGPD).
   Step by step with the owner (he buys the server and domain; Claude never enters payment details).

## G15 — Signing through a link (needs G13.4 and G14.1)
- **Flow**:
  1. Adriel closes a version.
  2. He adds signers (name and email).
  3. He signs first in Graybox: he draws or types his name.
  4. He publishes, and each signer gets a signing link.
  5. The signer opens the link, reads the whole version (scrolling to the end enables *Sign*),
     types their name and email, receives a 6-digit email code, and enters it.
  6. The signer draws a signature with the brush or types their name and picks one of three
     handwriting fonts.
  7. The signer accepts the statement ("I have read and agree to version N of <document>"), then
     signs.
- **What is recorded per signature** (the audit trail):
  - name, email (verified), the signature image;
  - server UTC time, a signature id;
  - city and country from the IP (from an offline IP database with a licence that allows it, with an
    ATTRIBUTION row), the IP, the browser;
  - the version's fingerprint.
  The server signs the whole trail with its own key (Ed25519), so any later change is detectable.
- **When everyone has signed**, the version is **Signed**:
  - Graybox and the web view show a seal;
  - the PDF gets a **certificate page**: every signer, their time, place, email, the fingerprint
    and the trail's id;
  - Graybox can check a signed PDF or file against the server's record ("Verify").
- **Email**: a transactional email service (for example Resend's free tier). Its key lives only in
  the server's config, never in the repo.
- **Handwriting fonts**: open licence (OFL) families from `storytold/craft-fonts`, never in this repo.
- Signed versions are never deleted by the server's 30-day cleanup.

## Rules (as everywhere in Graybox)
- No panics. Wasm keeps building, because the web view and the signing page are the browser build.
  Rust only.
- Upstream touched only at named hooks, listed in STATUS.md.
- Client documents and signatures are confidential: no real client data in tests, fixtures, docs or
  commits.
