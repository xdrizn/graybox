# Game layer — status

**Current milestone:** G7 — Share (and G0.3, G2.3 leftovers)

## Done
- G9 owner's batch (2026-11-13): nine reports from the owner.
  - **Pen**: with Shift held, the Pen never curves. Every segment is straight and snapped to 45°, both
    when drawing and in vector edit mode. The 8 px click wobble is now measured from where the press
    landed, not from the snapped anchor, so a horizontal Shift-click no longer counts as a drag.
    Shift is read live during the drag, as in Figma. Letting go of it mid-drag pulls the curve's
    handles out of the point Shift placed (vector edit mode measured them from the press before).
    Pressing it again straightens the segment again. A Shift press keeps the previous point's
    handle off.
    Tests: `crates/engine/src/pen_tests.rs`.
  - **Pencil strokes with spikes** (the "broken brush strokes"): fixed by taking upstream's
    #1741 fix (`figcraft_geom::simplify_path`, upstream 0aba3f76 and 61389cdd) on its own,
    without a full merge. A stroke that turns back on itself, or that runs almost straight, no
    longer gets a curve that shoots off it. Upstream's tests came with it. The Brush and Width
    profile menus do open (an earlier probe had injected the press and the release in the same frame).
  - **Organic brushes and Taper**: four new stretch brushes made in code: Brush pen (ink with
    varying width), Dry brush (strands with gaps), Chalk (rough edges with grain), and Marker
    (chisel ends). They live in `crates/pathops/src/organic.rs`. The taper, ink, bristle and
    organic brushes are now built per subpath from a unit outline. `Brush` gained
    `taper` (on by default), `taperStart` and `taperEnd` (0..1 of the length; unset means the
    preset's own), with the controls Taper the ends, Taper start and Taper end under Brush. Taper
    also shrinks a scatter brush's ends. A closed path's brush starts and ends in the middle of its
    longest side, so a rectangle no longer gets a thin corner.
  - **Boolean corner radius**: upstream #2222 taken on its own (c48d844d, 4c91f908, 529134b8,
    bae366ba, ec105d0f). A boolean group has a Corner radius field that rounds the corners of its
    result. Layers inside keep their own radius.
  - **Eraser in design files**: the Eraser works on any page, not only on boards. On a board it
    still erases ink. In a design file it removes open, stroked, unfilled vectors under it, such
    as Pencil strokes; frames, shapes and filled paths are left alone. `tool.set {tool: eraser}`
    and `tool.eraser` no longer ask for a board. The Pen tool group now holds Pen, Pencil, Eraser
    and Shape builder. Draw mode is lit for both the Pencil and the Eraser.
    Logic: `crates/engine/src/design_eraser.rs`.
  - **Bottom bar**: the mode switcher shows only Draw · Pixel · Design. Prototype is a tab of the
    right panel. Inspect is a new small button left of Play at the top of the right panel
    (`crates/ui-egui/src/game/inspect_button.rs`), and Shift+D still works.
  - **No spacing handles on the canvas**: no gap handles either (the padding ones were already
    gone). Gaps and padding are set in the right panel.
  - **9-slice crossings**: a small square marks each of the four places where the cuts cross.
    Dragging one moves both of its cuts. With Shift, the opposite cuts follow, so the slice stays
    symmetric; Shift does the same when dragging a single cut. Code:
    `crates/ui-egui/src/game/nine_slice_nodes.rs`.
  - **Claude chat**: the guide now covers nested pages, Documents, 9-slice, the game kinds, the
    third-person game's variables and events, and Unity export. It also names what is planned
    (closing a document version with a signature, signing through a link, client links), so
    Claude says these are not there yet. A test checks that every command the guide names exists.
    The panel's title, close box, input and Send button now line up (Send moved to the input's
    right edge).
  - Checked in the real app on a scratch file: the brushes, Taper on and off on rectangles, the
    eraser, the Union radius, the absence of gap handles, the 9-slice crossings (the Shift case is
    covered by tests; the control channel does not hold Shift during a drag), and the chat panel.
    Limits: the design-file Eraser hit-tests top-level layers, so a stroke inside a frame is not
    erased; a boolean group's radius always rounds, whatever its Graybox corner style is.
- G17.1 3D rotation of layers (2026-11-13, owner's request; spec
  [`specs/2026-11-13-frames-3d-design.md`](specs/2026-11-13-frames-3d-design.md)): a frame, component,
  instance or group gets **X, Y and Perspective** in a "3D" section of the Design panel (under Position;
  `+` turns it on, `−` off). Unity's numbers: +X tips the top away, +Y brings the right side near (the
  owner's "left side nearer" health bar is Y negative); ±80°, perspective 50–100 000 (default 1000,
  raised so no corner of the box comes closer than a fifth of it). Stored in plugin data
  `graybox.rotation3d` = `x,y,perspective` (copies, undo and old builds keep or ignore it). Math in the
  new pure crate `crates/tilt` (`figcraft-tilt`, L0): a 3×3 homography about the layer's centre in its
  parent's space, after its own Z rotation (Unity's Z, X, Y order), with the near-plane cut for what
  overflows; property-tested (corners finite and in front at any angle, inverse round-trips, bounds
  hold the corners, junk strings). `crates/doc/src/tilt.rs`: reading it, the hit-test point mapping and
  the "edit flat" set. Rendering (`crates/render/src/tilt.rs`): the layer is drawn flat into its own
  pixmap (cached while its subtree's `Arc`s, scale and flags are unchanged: moving or panning only
  warps again), then each covered device pixel maps back through the inverse homography and samples
  bilinearly; bounds, culling, repaint of a change inside a tilted frame and PNG/JPG export follow the
  warp. The canvas, Prototype and Play all show it (one renderer). **Editable**: its layers stay real
  layers; a click hits what is drawn under the pointer; while a layer inside it is selected the frame
  shows flat (canvas picture and hit test) so handles, text editing and dragging line up, and it tilts
  back when the selection leaves. A selected tilted layer is outlined where it is drawn (its flat box
  keeps its handles). Smart animate animates X, Y and perspective. Command `node.rotation3d.set {ids?,
  x?, y?, perspective?, enabled?}` (undoable; in the junk sweep). Unity export writes
  `rotation3d: {x, y, perspective}` and renders the sprite flat; the Live Link sets
  `localRotation = Euler(x, y, 0)` about the rect's centre (pivot already 0.5), README notes that
  perspective needs a Screen Space – Camera or World Space canvas (compiled with Roslyn against Unity
  2022.3.39 dlls, not run in Unity). Checked in the real app on a scratch file: a 320 × 56 health bar
  at Y −40°, perspective 600 (left side near), a click above its flat box selects it, selecting its
  label shows it flat, fill width and text changed, deselected it tilts back with the changes, PNG
  export takes in the near end, Prototype shows it tilted. Not checked: Play over the game scene, the
  browser build beyond `cargo xtask wasm`, the Live Link inside Unity. Code review (high) fixed: the
  flat drawing's effect scale inside scaled parents, the cache keyed on the presenter's clock (it
  missed every frame in Play; now keyed on GIF/pixel frames), the cache ignoring layers left out or
  outlined, "Select layer" not following edit flat. Left for G17.2: the flat selection box and hover
  outlines inside an unselected tilted frame, nested tilted outlines, the root frame's tilt in smart
  animate. Limits: SVG and PDF export draw it flat; background blur / glass
  inside a tilted frame sees only the frame's content; a blend mode on the tilted layer itself
  composites as normal; no canvas handle to drag the tilt yet (G17.2).
- G17.2 the tilt on the canvas (2026-11-13): a selected tilted layer (alone, Move tool) gets a **round
  handle** 18 pt above the middle of its drawn top edge; dragging it turns the layer like a trackball
  (right: Y−, down: X−, 0.5° a point, Shift snaps to 5°), shows "X …° Y …°" while held, and is one
  "3D rotation" undo step; it keeps the pointer until the release, even off the canvas. **Selection
  shows the tilted shape**: the selection box, its handles, hover and selection outlines and the frame
  name label follow the warp (`tilt_ui::warp`/`drawn`/`drawn_box`, through `doc::tilt::page_warp`:
  the page-space warp of a layer and its tilted ancestors, inner first, edit-flat ones left out), and
  the corner radius / arc handles hide on a tilted layer. **Handles through the tilt**: a press on a
  drawn handle maps back to the flat one (`cmd::tilt::handle_point` in `handle_at`) and the drag runs
  in the layer's flat frame (`cmd::tilt::pointer` in `Session::pointer`, inside `with_flat`); a resize
  solves, from the press, the flat corner whose new box is drawn with that corner under the pointer
  (`figcraft_tilt::Resize::solve`, damped Newton, property-tested), so the corner follows the pointer
  exactly with no drift; where no solution exists the last one stays (no jump). **SVG and PDF export**
  embed a tilted layer as its picture (rasterised whole over its drawn bounds, like a blurred layer).
  Checked in the real app on a scratch file: a 320 × 56 bar at Y −40°: the outline and handles on the
  drawn corners, the round handle turning it to Y −60° X −2° in one undo step, a corner drag landing
  under the pointer and holding its size, edit flat on its fill, PDF and SVG exports tilted. Code
  review (high) fixed: the inside-flag reset however the tools return, the held drag tied to its
  document, the knob drag ending off the canvas, no jump when the solver fails. Limits: the resize
  model is a plain corner/side resize (Alt from the centre, Shift keeping the ratio and the Scale tool
  drift from the pointer on a tilted layer, as does dragging a corner past the opposite one); an
  outline point that falls behind the eye is drawn flat; the hover outline was checked by a unit test,
  not in the app.
- G9 working inside a frame (2026-11-12, owner's report): with a top-level **frame** (not a component,
  instance, widget, board object or empty frame) that has layers in it **selected**, a press on its empty
  background draws a marquee of the layers inside (no Ctrl; it used to drag the frame), a press on a layer in
  it selects that layer directly (already so), and the frame moves by its **name label** (and an empty frame
  by its body). The frame's hover outline is hidden over its background while it is worked in: selected, a
  layer in it selected, or the view zoomed onto it (the frame covers 70 % of the viewport along an axis; an
  unselected frame still marquees and a click selects it, as before). The resize handles on its edges and
  corners work as before. Several selected layers are still dragged together from a gap; Shift-click on the
  background of a selected frame takes it out. Logic:
  `crates/engine/src/canvas/tools/frame_work.rs` (ours); tests `frame_work_tests.rs`; checked in the real
  app at 100 % and zoomed on a scratch frame (marquee, click, label drag, Shift).
- G9 nested pages (2026-11-12): the **Pages list is the same tree as the Docs tab**, and pages can sit
  inside other pages. Every page keeps its own canvas; nesting only organises the list. Data in
  `crates/doc/src/page_tree.rs` (ours): `Document::page_tree` = each nested page's parent + the collapsed
  pages, saved under `page_tree` only when something is nested or collapsed (old files open flat, upstream
  ignores the key); `Document::pages` stays a flat list but is **always the tree walked top to bottom**, so
  next/previous page, the page menus, the Actions palette, the prototype and the Unity export follow the tree
  with no change. `settle_pages` (run on load through `validate`/`repair`, and after every page edit) drops a
  parent that is not a page, cuts cycles and puts the list back in tree order; a page whose parent is missing
  is top-level. Commands in `crates/engine/src/cmd/page_tree.rs` (ours): `page.nest {id?, parent?, index?}`,
  `page.unnest {id?}` (right after its parent), `page.collapse {id?, collapsed?}` and `page.reveal {id?}`
  (not undo steps, but they make the file dirty), read-only `page.tree`; and the page commands learned
  `page.add {parent}` (one undo step), `page.delete {subpages}` (by default the pages inside move up a
  level), `page.duplicate` (the copy sits in the page its source is in), `page.move` (settles). Going to a
  page (`page.select`, next/previous page, the list seeing the open page hidden after undo or opening a
  file) opens the pages above it. UI: `crates/ui-egui/src/game/tree_list.rs` (ours) is the tree widget the Docs
  tab and the Pages list both use (row 28 high, chevron, indent 16, hover, `+` and `…` at the end, drag
  indicator, top/bottom quarter = before/after, middle = inside); `game/page_list.rs` is the Pages list
  (dividers stay lines and hold nothing; search lists matches flat, also inside closed pages; Dev Mode opens
  and collapses but nests nothing; layers dragged onto a page row still move there; half a row of slack above
  the first and below the last row; Escape cancels a drag). Delete (menu or key) on a page with pages inside
  asks: Cancel / Keep the subpages / Delete with subpages. Strings in `en.toml` and `pt-BR.toml`. Tests:
  `doc/src/page_tree_tests.rs`, `engine/src/page_tree_tests.rs` (also the junk sweep), `game/page_list/tests.rs`.
  Not synced in live collaboration (like `docs` and `game`): a collaborator's page edits keep the list
  valid but may flatten the nesting. Found in the real app and fixed for both lists: a row's `+` and `…`
  buttons vanished as soon as the pointer moved onto them (the row stops counting as hovered under its own
  buttons), so the `…` menu could not be opened; `tree_list::hovered` asks the row's rectangle instead.
- G13.1 (2026-11-12): documents, the model, the commands and the Documents tab (spec
  [`specs/2026-11-12-documents-design.md`](specs/2026-11-12-documents-design.md), decisions P1-P10 there).
  Model in `crates/doc/src/docs.rs` (ours): `Documents` is a tree of `Doc`s (id, title, icon, children),
  each with versions (`number`, `state` draft or closed, room for a `fingerprint` and `closed_at`, the
  `blocks`). A `Block` has an id, a kind (paragraph, heading 1-4, bulleted, numbered, checklist, toggle,
  callout, quote, divider, table, link, image, shape, swatch, embed by node id with live or pinned, decision,
  auto-table, signature), text `Run`s (bold, italic, underline, strike, code, colour, highlight, link,
  mention), a style (font, size, colour, background) and child blocks. It sits in `Document::docs` and is
  saved under `docs` only when there is one, so older files are byte for byte what they were and upstream
  ignores it. Loading repairs a hand-edited file (cycles, duplicate ids, deep nesting, out-of-range data) and
  caps everything. Closed versions refuse every block edit (the store already honours it; closing itself is
  G13.4). Commands in `crates/engine/src/cmd/docs.rs` (ours): `doc.create`, `doc.rename`, `doc.move`,
  `doc.delete`, `doc.set_icon`, `doc.block.insert|set|turn_into|move|delete`, all undoable, plus the
  read-only `doc.get` (the tree, or one document with its blocks). UI in `crates/ui-egui/src/game/docs/`
  (ours): a **Documents** tab in the left rail (File, Assets, Documents, Variables) with the tree (New
  document, click to open, double-click or the row menu to rename, + to nest, drag to reorder or nest, icon
  picker, delete with a question) and, for an open document, an editor in place of the canvas (editable
  title; text and heading blocks; Enter adds a block, Backspace on an empty one removes it; a handle on the
  left turns a block into Text or Heading 1-3, or deletes it; "Back to canvas"). Other kinds show as a dim
  chip until G13.2. Typing is sent as one command after a pause, on losing focus, or before any other
  command (hook in `FigApp::run`), so a burst of typing is one undo step and a save never misses the last
  words. Tests: `docs_tests.rs` (doc), `docs_cmd_tests.rs` (engine, including save and reload and junk
  parameters), `game/docs/tests.rs` (UI).
  Not synced in live collaboration (like `game` settings). Typing in a block with several styled runs keeps
  the styles of what did not change (only the changed characters are cut and put in; words after a mention
  start a plain run). A block kind this build does not know loads as a dim "unknown" chip instead of
  refusing the project, but saving from this build drops that block's data (a file from a newer Graybox).
  Checked in the real app on a scratch file: tab, new document, typing, heading, nesting by drag, save,
  close and reopen. Not checked: emoji icons in the picker (they come from egui's own font and may show as
  boxes), controller, the browser build beyond `cargo xtask wasm`. Not virtualised: a document with
  thousands of blocks lays out every block each frame (G13.2).
  Code review (high, twice) caught and fixed: Enter in a nested block added the new block at the wrong
  place (it used the flat position as the top-level index); typing in a block with bold words or links
  flattened them; dropping a document while the pointer was over the panel header could hit a row scrolled
  out of view; one unknown block kind, or a document with no id, made the whole project fail to open;
  `doc.block.set` hid why a style or data was refused; deep blocks were indented wrongly (an `i8` cast);
  words typed after a mention joined the mention; `doc.get` used `blocks` for a count in the list and for
  the blocks of one document (the list says `blockCount` now); editor state of closed files was never freed;
  dead code, and no tests for Enter, drag-and-drop and the delete question (added).
- G13.3 (2026-11-13): a document version as A4 pages, a canvas card that follows the document, PDF and
  Markdown export. One layout for everything: `crates/engine/src/docs_page.rs` (ours) lays a version out
  as A4 pages (595 x 842 pt, 56 pt margins, title block with icon, "Version N - Draft/Closed", a footer on
  every page with the title, the version, the fingerprint when there is one and "Page i of N") and the
  same frames feed the PDF, the canvas card and later the web view. Text is measured with the real text
  layout, split between lines at page breaks (never a heading alone at the bottom, never one lone line of
  a short paragraph, no blank first line on the next page), capped at 300 pages and 12 levels of nesting.
  Every block kind has a look (lists with bullets, right-aligned numbers and checkboxes, toggles open,
  callouts, quotes, dividers, tables that never split a row, decisions with who/when/why, signature lines;
  images, embeds and auto-tables are placeholders until G13.2). A closed version gets a seal on page 1.
  `crates/engine/src/docs_render.rs` (ours) turns the layout into real nodes and into a PDF through the
  existing PDF export (a scratch document, so the PDF matches the card). Commands: `doc.export` (`pdf` or
  `md`, to a path or back as data), `doc.card.create` and `doc.card.refresh`. The card is a transparent
  frame with the locked pages side by side, marked in `plugin_data` (`graybox.doc`, `graybox.doc.version`,
  `graybox.doc.hash`); every document edit rebuilds the cards of that document in the same step, so one
  undo takes back the words and the card together, and a card stays as a record when its document is
  deleted. Markdown: `crates/doc/src/docs_md.rs` (ours). UI: Export pill at the top right of the editor
  (PDF, Markdown, Put on the canvas), the same three items in a tree row's menu, and dragging a document
  from the tree onto the canvas puts its card there. Tests: `docs_page_tests.rs`, `docs_export_tests.rs`
  (engine), `docs_md_tests.rs` (doc), `game/docs/tests.rs` (UI).
  Checked in the real app on a scratch file: a 4-page card on the canvas, footers, the Export menu, a card
  by the menu and by dragging from the tree; the PDF was opened and sent to Adriel. Not checked: emoji
  icons inside the card and the PDF (they need a font with the glyph), the browser build beyond
  `cargo xtask wasm`. Page furniture ("Version", "Page 1 of 4", "Draft") is in English for now; G13.4
  decides the language. The card is rebuilt whole after every typing pause (fine for small documents;
  G13.2 can rebuild only changed pages).
  Code review (high, three passes) caught and fixed: text dropped at a page break; a drop on the panel
  over the canvas made a card; card pages could be edited by hand and then lost (now locked); the next
  page started with a blank line; the canvas rect could be zero on first use (zooms to the card); pages
  were measured after the 300-page cap; every card was rebuilt on any document's edit; an untitled
  document gave a card with no name; `doc.card.refresh` could not repair a card whose page was deleted by
  hand; the widow fix could leave an orphan; the PDF was laid out twice; list numbers of 10 and more
  wrapped onto two lines; the Markdown result reported `pages: null`. Skipped: a card pasted into another
  file follows a document of the same id there.
- G13.6 (2026-11-13): emoji on page and document icons. A double-click on the small icon at the left
  of a row in the Pages list or the Documents tree (lit under the pointer, with a tip) opens an emoji
  picker; the row menu's "Change the icon…" opens it too. The picked emoji replaces the icon; "Remove
  the icon" brings the built-in one back. One picker for both (`crates/ui-egui/src/game/emoji_picker.rs`,
  ours): 96 emoji and symbols in six groups (Pages, Game, Work, People, Nature, Shapes), drawn with
  egui's monochrome emoji font. Only emoji the app's fonts can draw are offered, and a row whose icon
  the fonts cannot draw (a hand-edited file, Claude) shows the built-in icon, so never a box; a test
  checks every emoji on offer. Nine of the first choices, and the document icon list of G13.1 (its
  🗂), were boxes and were replaced. A page's icon is Graybox data saved in the file under
  `page_tree.icons`, not in the page name (`Document::page_icon`, `set_page_icon`; cleaned and capped at
  16 characters when a file loads, forgotten when its page is deleted, kept by Duplicate page). Command
  `page.set_icon {id?, icon}` (undoable; empty removes); `page.tree` lists `icon`. Tests:
  `page_tree_tests.rs` (doc and engine), `game/page_icon_tests.rs` (UI).
  Checked in the real app on a scratch file: the hover state and tip, the picker from a double-click
  on a page icon and from a document's menu, the emoji in both lists; screenshots sent to Adriel.
  Not checked: colour emoji (egui's font is monochrome), emoji in a document card or PDF title (G13.4).
  Code review (high) caught and fixed: the picker was keyed by the row's place, so a list that changed
  while it was open could send the pick to another page (now keyed by the page or document).
- G13.4 and G13.7 (2026-11-13): closing a version, new versions, compare, the decision log, "For
  signature" and signing on this computer.
  - Close version… (asks first) records the SHA-256 fingerprint of what the version says (its blocks,
    "for signature" and signers: `docs_version::canonical`) and the closing time, and the version can no
    longer change. Its short code `GBX-XXXX-XXXX` (the first 40 bits in Crockford base 32) shows in the
    version bar, on every PDF footer and in the Markdown. New version: a draft copy whose blocks keep
    their ids. The version menu reads an earlier version (read-only, with a note), compares a version
    with the one before (added, removed and changed blocks, a removed block where it was) and opens the
    decision log (every Decision block of the file with who, when and why; a click opens its document).
  - A closed version whose words no longer match its fingerprint says "Changed outside Graybox" and
    cannot be signed (the editor checks again once a second). A test pins the shape of what the
    fingerprint covers: a field added to blocks or signers later must be skipped when it is default, or
    every version closed before would read as changed.
  - For signature (G13.7): a switch in the version bar adds a Signatures section at the end with a slot
    for the Designer and one for the Client (name and role fields; add or remove signers, up to 20). A
    Signature block (Turn into → Signature; the `/` menu comes with G13.2) shows the slot of the role it
    names anywhere in the document. In the PDF each slot is a line with the name, role and a date line;
    a signed slot shows its mark and "Signed <time>".
  - Sign… on a closed version: draw the signature with the mouse or a pen on a 3:1 pad, or type the name
    in one of three handwriting styles. No font ships with Graybox (craft-fonts has no handwriting
    fonts): each style uses the first installed of a short list (Dancing Script, Segoe Script…; Caveat,
    Ink Free…; Great Vibes, Lucida Handwriting, Gabriola…), else Inter italic. A signature records the
    name, role, time, the fingerprint it agreed to and how it was given ("graybox"); the model has room
    for G15 (a signer's email, the server's time, "link").
  - The PDF of a closed version for signature (or signed) ends with a certificate page: the code, the
    SHA-256, the closing time, each signer (signed in Graybox on this computer, or not signed yet) and
    the line for the client's co-signature through gov.br (assinador.iti.br). The Markdown ends with the
    signers. Times are UTC: the engine has no local time zone.
  - Model `crates/doc/src/docs_version.rs` (ours). Commands in `crates/engine/src/cmd/docs/version.rs`
    (ours): `doc.version.close`, `doc.version.new`, `doc.version.for_signature`, `doc.version.signers`,
    `doc.version.sign`, `doc.versions`, `doc.diff`, `doc.decisions`. Page: `docs_page/sign.rs`; a card
    draws a drawn mark as a vector (`Item::Ink`), layout revision 2. UI:
    `crates/ui-egui/src/game/docs/{versions,sign,compare}.rs`. Tests: `docs_version_tests.rs` (doc and
    engine), `game/docs/version_tests.rs`, junk sweep entries.
  - Checked in the real app on a scratch session with its own config folder: the switch, closing,
    signing typed and drawn, the PDF and its certificate page, the card on the canvas, compare.
    Screenshots and the PDF sent to Adriel.
  - Code review (high) caught and fixed: the fingerprint's shape was not pinned; the Sign dialog's name
    changed only on Enter (the previews lagged, the first click on Sign could miss); Compare stayed open
    over another document; the card layout revision was not bumped; a closed version without a
    fingerprint looked intact in the editor but could not be signed; a removed block showed after the
    block added in its place; a slot without a name printed "Name" in bold.
  - Not done: local times, page furniture in Portuguese, the `/` menu (G13.2), signing through a link
    (G15).
- G13.2, first part (2026-11-13): the blocks look and edit like Notion's.
  - Each kind draws its mark on the line of its words: a bullet (dot, ring, square by depth), the number
    of its run (a block of another kind starts the count again), a checkbox that ticks (a done task is
    struck through and dim), a toggle arrow that opens and closes (its content hides), a callout with its
    icon on a tinted card, a quote bar, a decision card with who, when and why, a link with its address.
    An empty block says what it is ("Heading 2", "List", "To-do"...).
  - Markdown marks typed at the start of a block turn it, and the mark goes: `#` to `####`, `-` `*` `+`,
    `1.` `1)`, `[]` `[ ]` `[x]`, `>` `"`, and `---` (a divider; writing goes on in a new line below).
  - Enter splits at the cursor (selected words go first); a list, task, toggle or quote goes on as the
    same kind, a heading is followed by text; Enter on an empty list item ends the list (out one level,
    else text). Shift+Enter breaks the line inside the block. Backspace at the start turns a list item,
    heading, quote or callout into text, then takes a nested block out a level, then joins it to the
    block above (the cursor lands at the joint); an empty decision or link turns into text, a filled one
    stays. Tab nests under the block above (opening it if it is a closed toggle), Shift+Tab takes it out;
    the arrows cross from block to block at the first and last character. Each is one undo step.
  - A handle (six dots) left of the hovered block opens a menu: Duplicate, Delete, Turn into (13
    kinds), the colour of the words and the background (9 colours that read on both themes, or none).
  - Model: `crates/doc/src/docs/edit.rs` (ours): `split_block`, `merge_block`, `duplicate_block`; the
    words that move keep their block's colours, a new empty line starts plain. Commands `doc.block.split`,
    `doc.block.merge`, `doc.block.duplicate`; `doc.block.turn_into` takes `text?`. UI:
    `crates/ui-egui/src/game/docs/blocks.rs` (the editor page in `editor.rs` now only lays them out). The
    editor counts as the canvas for keyboard navigation, so Tab nests a block instead of moving to the
    panels. Tests: `docs/edit/tests.rs`, `docs_cmd_tests.rs`, `game/docs/blocks_tests.rs` (raw keys).
  - Checked in the real app on a scratch session: the kinds, typing `## ` and `[] `, Enter twice to end a
    list, Tab, the handle menu, a background colour. Screenshots sent to Adriel.
  - Code review (high, two rounds) caught and fixed: Enter split where the cursor was a frame earlier;
    an empty heading turned into text on Enter; Backspace emptied a decision or link into text; Tab
    nested under a divider; the new block lost its colour; a split at the deepest level made a block
    too deep; Shift+Enter split instead of breaking the line and Shift+arrows left the block; Enter kept
    selected words; an empty decision could not be removed with Backspace; an empty new line took the
    heading's colour and size.
  - Next parts: the `/` menu, drag to move, the formatting bar, images, embeds, tables, links and
    mentions, selecting several blocks, Font in the handle menu. Delete at the end does not join the next
    block yet.
- G13.2, the `/` menu (2026-11-13): typing `/` at the start of a block or after a space opens a list of
  blocks under the cursor (Text, Heading 1 to 3, the lists, Toggle list, Callout, Quote, Divider,
  Decision, Link, Signature). The letters after the `/` filter it, in the app's language or English and
  without accents ("titulo" finds "Título"); the arrows choose, Enter or a click picks, Escape closes
  and leaves the words. An empty block turns into the kind picked; a block with words keeps them
  (without the `/` and its letters) and gets the new block below. When nothing matches, Enter is an
  ordinary Enter; a space after letters that find nothing, or the cursor going before the `/`, closes
  the menu. The only empty line of a document, and the line being written, say "Type “/” for blocks";
  an empty block's hint uses the block's own font.
  - "Toggle" read "Alternar" in Portuguese (upstream's word for a switch): the block is now "Toggle
    list" ("Lista recolhível").
  - Code: `crates/ui-egui/src/game/docs/slash.rs` (ours), wired in `blocks.rs`; tests in
    `game/docs/slash_tests.rs` and `slash.rs`.
  - Checked in the real app, in Portuguese: the hint, the menu, the filter without accents, the arrows
    and Enter. Screenshots sent to Adriel.
  - Review (high) caught and fixed: the menu was 600 px wide (egui's first-pass width); Enter was
    swallowed when nothing matched; the hint was smaller than the line's text.
- G13.2, moving blocks (2026-11-13): drag a block by its handle (the six dots; a click still opens its
  menu). While it is dragged, an accent line shows where it will land: between two blocks, at the level
  of the block below the line, or one level in (after a nested block) when the pointer is that far
  right. Letting go moves it with what is inside it, in one undo step; it never lands inside its own
  content; Escape cancels; near the top or bottom of the editor the page scrolls. Ctrl+Shift+↑/↓ (⌘ on
  a Mac) moves the block with the keyboard among its neighbours. The handle's tooltip now says "Click
  for the menu, drag to move".
  - Code: `crates/ui-egui/src/game/docs/drag.rs` (ours; where a drop lands is a pure function with
    tests), the handle in `blocks.rs`, `editor.rs` collects each block's row. Tests:
    `game/docs/drag_tests.rs` (raw pointer drags and keys).
  - Checked in the real app: the line while dragging and the block landing. Screenshots sent to Adriel.
  - Review (high) caught and fixed: egui ends a drag on Escape too, which dropped the block instead of
    cancelling; no scrolling while dragging in a long document; the line crossed the tops of letters.
- G13.2, formatting words (2026-11-13): select words in a block and a bar comes up over them: bold,
  italic, underline, strike, code, link and colour (text colour and highlight swatches). The keys do the
  same: Ctrl+B, I, U, E (code), K (link) and Ctrl+Shift+X (strike), ⌘ on a Mac; pressed again they take
  the look off. The words stay selected after each change, so looks stack. Ctrl+K (or the link button)
  asks for an address; Enter links the words, an empty address unlinks them, Escape goes back. Links
  are drawn in the accent colour, underlined; Ctrl+click opens them (web and e-mail addresses only,
  never files or programs). Words typed next to formatted ones take their look. Each change is one
  undo step through the new command `doc.block.format {doc, id, from, to, mark, value?, on?}` (also for
  MCP, the CLI and the control channel).
  - Code: `crates/doc/src/docs/format.rs` (the range formatting, pure, with tests), the command in
    `crates/engine/src/cmd/docs.rs` (in the junk sweep), `crates/ui-egui/src/game/docs/inline.rs` (the
    bar, the keys, drawing the looks, Ctrl+click). Tests: `game/docs/format_tests.rs`.
  - Checked in the real app: the bar, bold, the colours, Ctrl+K and the drawn looks. Screenshots sent
    to Adriel.
  - Review (high) caught and fixed: Ctrl+click would have opened any address (a `file://` link in a
    document from someone else could launch a program); Ctrl+Shift+U/K still deleted words (egui's
    own keys); the bar could carry over to the same-numbered block of another document; coloured words
    in a task done were not dimmed. Found while checking in the app: opening the colours closed the bar
    and lost the selection (a field given the keyboard reports no selection in that frame).
  - Not yet: Ctrl+B with nothing selected does not "type bold"; the per-frame copy of the runs could be
    shared (small cost in long documents).
- G13.2, tables (2026-11-13): `/table` puts a table of three rows by three cells in the document, the
  first row a header (semibold on a tinted row), and writing starts in its first cell. Tab and
  Shift+Tab go to the next and previous cell, Enter to the cell below; past the last cell they add a
  row. With the pointer near the table, a + bar under it adds a row and one on its right adds a column.
  Right-click a cell: insert a row above or below, a column left or right, delete the row or the
  column (never the last one), switch the header row. Words in cells keep their looks when edited.
  Each change is one undo step through the new commands `doc.table.cell {doc, id, row, col, text}` and
  `doc.table.change {doc, id, op, at, on?}` (also for MCP, the CLI and the control channel; a delete
  must name its row or column).
  - Code: `crates/doc/src/docs/table.rs` (the table edits, pure, with tests), the commands in
    `crates/engine/src/cmd/docs.rs` (in the junk sweep), `crates/ui-egui/src/game/docs/table.rs` (the
    grid, the cells, the + bars, the cell menu). Tests: `game/docs/table_tests.rs`.
  - Checked in the real app: the table, padding in the cells, typing, Tab from the last cell adding a
    row, the cell menu. Screenshots sent to Adriel.
  - Review (high) caught and fixed: Ctrl+U and Ctrl+K in a cell deleted its words (egui's own keys);
    Tab past a full table sent the keyboard to a row that was never added; `doc.table.change` with no
    `at` deleted the first row. Found while checking in the app: the cells had no room around their
    words (a field without its own frame ignores its margin).
  - Not yet: bold and the other looks cannot be set inside a cell (the bar is for blocks only); arrow
    keys do not move between cells; columns all share the width; the table is copied every frame
    (small cost in big tables).
- G13.2, images (2026-11-13): `/image` puts an empty image block in the document and asks for a
  picture file ("Choose image" on the block does it again); image files dragged onto an open document
  go in where they are dropped, one after the other, each named after its file. A picture shows as
  wide as the column allows but never wider than itself, and at most 560 points tall. The picture goes
  into the file's images (as on the canvas), through the new command `doc.block.image {doc, data |
  path, alt?, id? (an empty image block), parent?, index?}` (one undo step; also for MCP, the CLI and
  the control channel).
  - Code: the command in `crates/engine/src/cmd/docs.rs` (in the junk sweep; a refused block leaves no
    picture behind), `crates/ui-egui/src/game/docs/media.rs` (drawing, the empty block, drops).
    Tests: `game/docs/media_tests.rs`.
  - Hook: `crates/ui-egui/src/pending_open.rs` (the "Adding to…" card also names `doc.block.image`).
  - Checked in the real app: a 1200×600 picture and an empty image block. Screenshot sent to Adriel.
    Not clicked in the app: "Choose image" (it opens the system's file window); it uses the same
    picker as the colour picker's "Upload from computer".
  - Found by the tests and fixed: a picture wider than 2048 pixels crashed the window (the graphics
    card refuses such textures); pictures are now made no bigger than they show.
  - Review (high) caught and fixed: `/image` left a cursor position behind for the next field.
  - Not yet: images in the canvas card, PDF and Markdown exports are still placeholders; a caption
    and resizing by hand; files dropped on a closed version are ignored without a message; the same
    picture shown at two very different widths is decoded again each frame.
- G13.2, frames and components in documents (2026-11-13): `/` "Frame or component" puts an embed
  block in the document and opens a searchable list of the file's frames, components, variants and
  instances (top layers of every page, and those inside sections and component sets). The layer shows
  fitted to the column (never larger than itself), with "Live" or "Pinned" and its name under it.
  Live follows the layer as it changes (redrawn only when its layers or the components its instances
  show change); the handle menu's "Follow the layer" switches to Pinned, a picture of it as it is now,
  and "Choose another layer" changes it. Closing a version pins every live layer in it, so what was
  signed keeps showing what it showed. A deleted layer says so and offers the list again. One new
  command, `doc.block.embed {doc, node, live?, id?, parent?, index?}` (one undo step; also for MCP,
  the CLI and the control channel).
  - Code: `crates/engine/src/cmd/docs.rs` (the command, `snapshot`, `pin_live_embeds`, called by
    `doc.version.close` in `cmd/docs/version.rs`), `crates/ui-egui/src/game/docs/embed.rs` (drawing,
    the list). Tests: `game/docs/embed_tests.rs`, `docs_cmd_tests.rs`.
  - Checked in the real app: a live HUD frame, the list with search, Enter picking, pinned. Screenshots
    sent to Adriel.
  - Caught by the tests and fixed: a layer picked from the list was pinned instead of live. Review
    (high) caught and fixed: a live layer's picture could pass the graphics card's size limit by one
    pixel.
  - Not yet: embeds in the canvas card, PDF and Markdown exports are still placeholders; a very large
    live layer redraws on the window's own thread; the list does not open upwards near the bottom.
- G13.2, colour swatches, shapes, mentions and several blocks at once (2026-11-13): `/` "Colour
  swatch" adds a colour chip with its code and a name; clicking the chip opens the palette and a
  code field. `/` "Shape" adds a rectangle, ellipse or line with a fill, changed from its menu. Typing
  `@` opens a list of the file's other documents and frames (search as you type, Enter picks); the
  pick becomes a blue pill in the words, and Ctrl+click on it opens that document or goes to that
  frame on the canvas (selected and zoomed). Escape in a block selects it; Shift+Up/Down and
  Shift+click grow the selection (shown as one blue band), Delete removes the selected blocks in one
  undo step, Ctrl+C copies them as Markdown and Ctrl+V pastes them back as blocks (in an empty line,
  in its place). Two new commands: `doc.block.mention {doc, id, from, to, layer | document}` and
  `doc.block.paste {doc, blocks, parent?, index?}`; `doc.block.delete` also takes `ids` (all in one
  step).
  - Code: `crates/doc/src/docs/{mention,clip}.rs`, `crates/engine/src/cmd/docs.rs`,
    `crates/ui-egui/src/game/docs/{select,slash,inline,blocks}.rs`. Tests: `select_tests.rs`,
    `mention_tests.rs`, `swatch_tests.rs`, `docs_cmd_tests.rs`.
  - Checked in the real app: swatches, an ellipse, the `@` list and a frame mention, Ctrl+click going
    to the frame, Escape + Shift+Down + Delete (one undo). Screenshots sent to Adriel.
  - Review (high) caught and fixed: copying took the blocks of the last version while an older one
    was shown; Enter on a selection could give the keyboard to an image.
  - Not yet: pasting blocks into an empty line is two undo steps; an image pasted while blocks are
    selected lands on the canvas; the swatch code field keeps an invalid code until reopened.
- G13.2, fonts, pasted pictures, Delete that joins, and the canvas kept out (2026-11-13): the block's
  handle menu now starts with Duplicate and Delete, then Font (Default or Mono), Colour and
  Background; "Turn into" comes last as a short list that scrolls, so the menu fits a small window.
  A Mono block is drawn in the app's monospaced face (exports use JetBrains Mono when installed). A
  picture copied in another app and pasted (Ctrl+V) goes in as an image block after the block written
  in, or after the selected blocks. Delete at the end of a block joins the next block's words to it.
  While a document is open, the canvas hidden behind it takes no keys and no pastes (Delete, tool
  keys, copy, paste used to act on the hidden layers); undo, redo, save, open, Actions and tabs still
  work.
  - Code: `crates/ui-egui/src/game/docs/{blocks,media,mod}.rs`; the canvas hook in `shortcuts.rs`.
    Tests: `blocks_tests.rs` (Mono, Delete), `media_tests.rs` (pasted picture),
    `select_tests.rs` (keys with a document open; fails without the hook).
  - Checked in the real app: a Mono paragraph and the new menu. Screenshots sent to Adriel.
  - Review caught and fixed: "Monotype Corsiva" counted as monospaced; "Default" showed checked for
    a block with another font of its own.
  - Not yet: in the browser an image-only clipboard is not pasted into a document (the browser sends
    only the key); the handle menu does not open upwards near the bottom.
- G13.8, Docs polish, first pass (2026-11-13): headings stand apart from what is above them (more room
  over a heading 1 than a heading 3), and callouts, quotes and decisions keep a little away from their
  neighbours. The canvas toolbar no longer floats over an open document. A callout without an icon of
  its own shows a drawn "i" in a ring (the light bulb emoji drew as an outline). An empty document
  offers "Start with" chips under its hint: Text, Heading 1, To-do list, Table, Image, Frame or
  component. The `/` and `@` lists and the list of layers open above their line when there is no room
  below.
  - Code: `crates/ui-egui/src/game/docs/{blocks,editor,slash,embed}.rs`; the toolbar in the existing
    `app.rs` hook. Tests: `blocks_tests.rs` (empty document), `slash.rs` (where a list opens).
  - Checked in the real app: the briefing with headings, the empty document, `/` near the bottom.
    Screenshots sent to Adriel.
  - Not yet: the right panel still shows the canvas' page properties while a document is open (that
    panel is another session's area); Up/Down step over tables, images and embeds.
- G11.6, the bow (2026-11-13, Adriel's changes): a click shoots from the hip, holding aim goes over the
  shoulder with a crosshair in the middle of the screen, and the shoulder button swaps sides.
  - Hip shot: a left click alone with the bow turns the hero to where the camera looks and shoots after
    a quick 0.18 s draw (no aim mode, no crosshair).
  - Aim mode (`tps/combat.rs`, new action `aim`: right click, controller LT): only with the bow chosen;
    the camera eases in over the right shoulder (1.9 m behind, 1.5 m up, 0.65 m to the side: the hero on
    the left of the screen), the hero walks at aim pace facing the camera, the bow up with an arrow
    nocked (new clip `bowAim`). A click there draws from the nocked pose (0.35 s) and shoots; a click
    during the follow-through shoots next. Letting go of aim mid-draw drops the shot; a roll, a climb or
    a fall ends aim mode and it comes back after.
  - Shoulder swap (new action `shoulder`: mouse 4 and 5, controller RS): the camera eases across to the
    other shoulder (`tps/camera.rs`, interpolated between steps like the hero). Both are in the controls
    panel; files made before them play the default keys, less any key the file already uses.
  - Where arrows go (`tps/aim.rs`): a ray from the camera through the middle of the screen, starting past
    the hero, to the first foe, solid or ground (80 m); the arrow leaves the hand aimed a little high so
    its drop brings it onto that point.
  - The crosshair: a small light dot with a dark ring, drawn by the game under the screens, only in aim
    mode, fading with the camera. `mirando` is true in aim mode. "Crosshair when aiming" under the Game
    picker (command `game.crosshair.set {visible}`, undoable, saved in the file) hides it for a UI that
    draws its own.
  - Code review (high) caught and fixed: aiming right after a hip click lost that shot; old files fired
    the default aim keys over their own inputs; the crosshair ray ran every step; the controls panel
    rebuilt the default settings every frame. Each has a test or a guard.
  - Tests: `tps/bow_tests.rs` (hip shot, aim mode, crosshair hits near and far, easing in and out,
    shoulder swap, no aim without the bow, roll, junk camera), `engine/src/game_kind_tests.rs` (the
    crosshair command, the new keys), `game/tps.rs` (crosshair and `mirando`), `game/input.rs` (default
    keys).
  - Checked in the real app on a scratch file: aim mode over the right shoulder, the crosshair on a
    dummy and the arrow hitting it (45 → 20), the swap to the left shoulder (by the controller's RS:
    the control channel cannot send mouse 4/5), the camera easing back, a hip shot.
  - Still to check: in the browser, that mouse 4/5 do not also move the page back or forward (eframe
    already stops the right-click menu). Over the right shoulder the raised bow is mostly hidden by the
    hero's head (seen clearly from the left shoulder): a pose tweak for the feel pass.
- G11.6, the rest (2026-11-13): the camera eases back behind the hero, tree crowns fade out of its way,
  the feel pass and the browser check.
  - Camera (`tps/camera.rs`): after 2.5 s with no camera input, no steering and no aim mode it eases
    back behind the hero and to its usual height on a critically damped spring (about 2 s, starting
    gently, the short way round, drawn between steps). Any camera input restarts the wait; it never
    turns while the player steers.
  - Something between the camera and the hero still pulls it in at once (G11.4); it now eases back out
    (5 per second) instead of jumping.
  - Jumps and falls: the view's height trails the hero's on a spring (at most 1 m), so a jump moves the
    hero up the screen instead of bobbing the view; on the ground, a ladder or a wall it springs back,
    so hills keep the hero in the middle of the screen.
  - Crowns (`Prop::crown_in_the_way` in `tps/props.rs`, `fade_crowns` in the renderer, `prop_fragment`
    in `tps.wgsl`): a pine or oak crown on the line from the camera to the hero thins to a fine
    screen-door pattern (at most 85% of its pixels gone) and the hero shows through; its shadow stays.
    It fades smoothly with the distance from that line; only tree batches that changed are uploaded.
  - Bow, seen in play: over the right shoulder the hood hid the bow. The ready and draw poses hold it a
    little to the right and the aim camera now sits 1.0 m to the side (was 0.65) and 1.6 m up (was
    1.5), so it shows on both shoulders; the string hand waits behind the grip instead of sticking out.
  - Crossfades: G11.3's are kept (strikes fade in under 0.1 s on purpose, so a hit lands when pressed).
  - Browser (`game/browser_buttons.rs`): while a game plays (and half a second after) the page cancels
    the browser's own action for mouse 4 and 5 (press, release and auxclick), so they swap the shoulder
    instead of going back or forward; the editor keeps the browser's behaviour.
  - Checked in Edge (1256x708): the third-person game runs at the display's 200 fps, every frame under
    5.2 ms; every mouse 4/5 event is cancelled while playing and none in the editor. Budgets: 116,896
    triangles drawn plus 32,374 in the shadow map (limit 250k), 33 draw calls (limit 150), 18.9 MB of
    GPU memory on the desktop and 6.9 MB in the browser (limit 40 MB). The release wasm is 65 MB before
    wasm-opt and compression.
  - Not yet: a real mouse 4/5 click in the browser (automation cannot press them, and some mice send
    back and forward as keys through their driver, which no page can stop); the string hand dips a
    little mid-draw (keys blend angle by angle); in aim mode the crown test uses the line to the
    crosshair point, beside the hero. The installed copy comes with the batch install of this round.
  - Tests: `tps/camera_tests.rs` (10), `bow_tests::over_either_shoulder_the_head_never_hides_the_bow`,
    and the renderer's `tree_crowns_are_flagged_…` and `the_crowns_in_the_way_fade_…`.
- G11.5, rest (2026-11-13, "continue todos os trabalhos"): foes, damage, arrows, and the variables
  and events the designed UI uses.
  - Foes (`tps/foes.rs`): three training dummies in a row 8 m in front of the start and five husks
    around the valley (placed on dry, flat, open ground). A husk sees the hero at 16 m, walks over
    (1.5 m/s, never into the river, at most 26 m from home), winds up 0.7 s with its arms raised, then
    hits for 15 with a push if the hero is still in reach in front of it; a hit staggers it and loses
    the wind-up. Dummies wobble. Broken foes fly apart and come back (dummies 4 s, husks 14 s); husks
    walk home while the hero is dead. The hero is pushed out of foes (no walking through them).
  - Strikes land once each in the middle of the swing (sword 18/18/28, spear 26 reaching 2.9 m, club 36
    with a big push). The bow, drawn 0.35 s or more, shoots an arrow (`tps/arrows.rs`) where the camera
    looks: 32 m/s with a little drop, 25 damage, stuck in solids and the ground for 6 s, at most 16.
  - Health (`tps/damage.rs`): 100; a fall over 4 m without a roll hurts 6 per metre; a hit knocks back
    and flinches (not over a weapon draw or the drawn bow); at 0 the hero falls and lies there (the fall
    plays even when a death screen pauses the game); Enter respawns at the start, weapon kept.
  - Events and variables: `vida-mudou`, `tomou-dano`, `vida-zero`, `renasceu`, `atirou`,
    `inimigo-morreu`. A third-person file's Game collection adds `stamina`, `staminaMax`, `escalando` and
    `mirando` (`arma` starts as "Sword"); Play publishes vida, vidaMax, ultimoDano, stamina, staminaMax,
    agachado, escalando, mirando, atacante and arma (`game/tps.rs` `variables`). Game bars can follow
    `stamina`.
  - No third-person starter HUD (Adriel's choice: his own UI reads the variables and events).
  - Drawn (`renderer/tps/foes.rs`): faceted husks with two orange slit eyes (their only emissive), straw
    dummies on posts, a white flash and a lean on a hit, pieces when broken, arrows with a fletch.
  - Code review (high) caught and fixed: the hero walked through dummies; a hit during a weapon draw left
    the weapon on the back (and dropped the drawn bow's pose); a first-person stamina bar followed the
    reserve ammo; broken arm pieces sank into the ground; a death screen that pauses froze the hero
    standing; publishing the variables allocated every frame. Each has a test.
  - Tests: `tps/combat_tests.rs`, `renderer/tps/tests.rs` (foes and arrows), `game/tps.rs` (the bridge),
    `engine/src/game_kind_tests.rs` (the variables, a stamina bar).
  - Checked in the real app on a scratch file: a HUD bound to the variables, a dummy broken by the sword, a husk hitting
    (100 → 85), the death screen with the hero lying down, Enter respawning, an arrow hitting a dummy.
- G11.5, first part (2026-11-12, with G11.4, same request from Adriel): weapons and strikes.
  - Weapons (`tps/combat.rs`): 1–4 pick sword, spear, club, bow (drawn from the back; the same key again
    puts it away; also in the air; a key pressed mid-strike counts when the strike ends); click strikes
    (drawn at once if on the back): three sword strikes chain when clicked in time, the spear thrusts, the
    club swings; a strike slows the hero, lunges a little and keeps the facing; the bow draws while the
    button is held (walk pace, facing where the camera looks) and shoots on release. Nothing is hit yet.
  - Input: `Input::weapon` (`game/input.rs`, 1–4); the attack is the existing "shoot" binding. The Play
    help line lists both. `inspect` shows `strike` and `aiming`.
  - Code review (high, twice) caught and fixed: an attack during the sheathe moved the sword to the back
    mid-swing; a weapon key during a strike or a jump was lost; a strike cut short by walking off an edge
    kept playing its swing in the air. Each has a test.
  - Tests (`tps/movement_tests.rs`): the sword combo, weapon keys and the bow; the three fixes above.
  - Checked in the real app: sword combo, 2 spear, 3 club, 4 bow (held: aiming, released: shot); sheathe
    then click keeps the sword in hand; 4 during a club swing switches when it ends; 2 in a jump draws.
- G11.4 (2026-11-12, asked by Adriel after trying G11.3: "the hero walks through things, does not
  climb, does not attack"): movement and camera.
  - Collision (`tps/collide.rs`): the hero is a standing cylinder; the world's solid boxes push it out
    and cancel the part of its speed that runs into them; their tops are ground (it stands on crates,
    the tower's base, the wall's top), also after a long fall; a step of 0.3 m is walked up; downhill it
    keeps its feet on the ground instead of hopping.
  - Controller (`tps/controller.rs`, rewritten): jump with 0.12 s of coyote time and a 0.12 s buffered
    press; Ctrl rolls (a dodge, 15 stamina), and Ctrl just before landing turns the landing into a roll;
    a fall of more than 7 m without it knocks the hero down (knockdown, then get-up; damage comes with
    G11.5); walking into a ladder grabs it (W/S up and down, Space lets go with a small push, off the top
    onto what is behind when there is room, off the bottom onto the ground); walking into a ledge, crate
    or block 0.45–2 m high mantles onto it; walking into the rock wall climbs it on both axes (W/S, A/D;
    what stands in front of it, like the broken pillar, stops the hero) and costs stamina (it lets go when
    spent); running costs stamina (back after 0.8 s; spent out, it jogs until 25).
  - Camera: stops short of solids and hills between it and the hero.
  - Input: `Input::roll` (`game/input.rs`): "roll" reads the file's binding when it has one, else Ctrl or
    the controller's X. `inspect` shows `action` and `stamina`.
  - Code review (high, twice) caught and fixed: Space on a ladder or the wall also fired a full jump
    (coyote time from before the grab); a fall faster than 18 m/s went through crate tops; climbing the
    wall sideways went through the broken pillar; the top of a ladder or wall could pull the hero into a
    box behind it. Not fixed (small): the default Ctrl roll also fires when Shift changes while Ctrl is
    held, because egui gives no "Ctrl went down" event; fixing it needs last frame's modifiers through
    the upstream Play hook.
  - Tests (`tps/movement_tests.rs`): never inside a solid while walking every way from the start; a wall
    stops the hero; every ladder of the valley climbs to the top; down and off a ladder, jump lets go with
    only a small push; the rock wall climbs to the top, costs stamina, and sideways stops at the pillar;
    every ledge, crate and block with room on top is mantled onto, and landed on from up to 30 m; roll;
    hard fall knocks down unless rolled; buffered jump; the camera stops in front of a wall.
  - Checked in the real app: walked to the crate by the start and climbed it, jumped off; trunks and
    walls stop the hero; Ctrl roll; the tower ladder (Space off it rises 0.17 m); on the wall, sideways
    stops at the broken pillar. Seen: the camera goes into tree crowns (only trunks are solid) — G11.6.
- G11.3 (2026-11-11): the third-person hero and its animation. An 18-bone rigid-part skeleton
  (`crates/game/src/tps/rig.rs`: pelvis, spine, chest, head, two cloak bones, arms, legs; `Pose` holds
  a rotation per bone and a root offset), an original faceted hooded adventurer with a cloak, belt, strap
  and boots (no emblems; `renderer/tps/hero.rs`, under 3000 triangles) and four weapons (sword, spear,
  club, bow) held in the hand or worn on the back. A clip format (`tps/clip.rs`: keyed bone angles and
  root offsets, smooth Catmull-Rom or eased keys, loop / once / hold, full-body or upper-body layers) and
  all 26 clips of the spec authored in code (`tps/clips.rs`): idle, walk, jog, run, crouch idle and walk,
  jump up, fall, land, land roll, ladder and wall climb, mantle, three sword strikes, spear thrust, club
  swing, bow draw and shoot, hit, knockdown, get up, die, weapon draw and sheathe. The animator
  (`tps/animator.rs`) blends the six locomotion cycles by speed and crouch on one shared phase, goes to
  jump / fall / land from the controller's grounded state and vertical speed, crossfades every change from
  the pose on screen (no pops), and adds lean into turns, a pitch on speed changes, head yaw and nod, and
  a cloak that flares with speed and flutters. It runs in the fixed 60 Hz step and is drawn interpolated.
  `HeroPose` carries the skeleton, tilt, weapon, bow draw and clip. Debug builds preview clips in Play:
  F6 / Shift+F6 next / previous clip (name top left, camera closer), F7 ends it.
  Tests (`tps/animation_tests.rs`, `tps/rig.rs`, `renderer/tps/hero.rs`): blend weights sum to 1 at every
  speed and crouch; crossfades are continuous step by step; each clip loops or ends as declared; bone
  count and mesh budget; feet stay on the ground in the cycles; junk input.
  Checked in the real app on a scratch Third person file: every clip previewed from the front and the
  side, weapons along the forearm in the strikes, and held W (jog 5 m/s), Shift+W (run 8.5), release
  (settles to idle), C and C+W (crouch idle and crouched walk 2.4), Space (jump up, fall, land) with
  bursts of screenshots: legs alternate, the cloak swings, no jumps between frames. Not checked: walk
  (light stick only), a controller, the browser beyond `cargo xtask wasm`.
- G11.2 (2026-11-11): the third-person world. `crates/game/src/tps/` gets `terrain.rs` (an 80 x 80 cell
  heightfield, 2 m cells, made from integer-hash noise and smoothstep only, so it is bit-identical on
  wasm: a hill start in the south, a cliff drop, a river valley, plateaus with ruins, a waterfall at x=-44),
  `props.rs` (prop kinds with their solids and climb tags), `layout.rs` (everything placed on it: trees,
  rocks, ruined arches and pillars, the watchtower with a beacon, mountain and keep cards outside the
  map) and the rewritten `world.rs`. `World::ground_height` reads the very triangles that are drawn;
  `spawn`, `solids`, `climbables` (ladders, mantle blocks, a climb wall, for G11.4), `beacon`, `river`,
  `in_water`, `grass_cover`, `walk_limit` and `checksum` are the boundary. The renderer
  (`renderer/tps/`: `world`, `props`, `grass`, `sky`, `scene`, `gpu`, `shapes`, `tps.wgsl`) draws the
  ground and one-off pieces in one buffer, props instanced (one draw per kind, shadow too), grass as
  instanced tufts of 3 blades with the wind, the distance fade and the bend away from the hero done in the
  vertex shader (`grass_vertex`), the river and waterfall as emissive strips with a moving glow
  (`water_fragment`), and a thin eclipse ring with a halo in the sky; the beacon is the scene's one point
  light. Measured by the tests: 88k triangles drawn, 32k into the shadow, 32 draw calls, 19.6 MB of GPU
  memory on desktop and 7.0 MB with the 1024 shadow on web, 52 KB of world data in RAM.
  Checked in the real app on a scratch Third person file: Play shows the valley, ring, tower, ruins and
  trees; W walked over the ground at its height; the grass sways, bends away from the hero, fades with
  distance; trees, rocks and the hero cast shadows; the first-person arena looks exactly as before. Not
  checked: the waterfall and river up close, a controller, the browser beyond `cargo xtask wasm` (it
  compiles; its size growth was not measured), a slow GPU.
  The camera can still end up inside a tree or a pillar (G11.4 owns the camera).
  Failing test that is G11.3's: `renderer::tps::hero::tests::the_hero_is_drawn_where_it_stands_with_finite_vertices`
  assumes the ground is at y = 0 and says "feet on the ground: 8.78" now that the valley has hills. It
  should compare with `World::ground_height`.
- G11 clean look (2026-11-11, Adriel's change to the spec: the third-person game must look clean, not
  like the grainy first-person arena). The whole look is one struct, `TpsLook`
  (`renderer/tps/look.rs`), applied by `paint`; the first-person look is untouched and could take
  another `TpsLook` later. What it sets and does: no pixel-noise textures (every surface is its tone's
  flat colour, the project's greys stay the palette; the hero takes its tones from the same theme), no
  random facet or tuft shades (the ground tone depends on the slope only), a smooth sky gradient
  (a dome shaded by view direction: `sky_fragment`) that is also the fog colour, the picture stored
  sRGB-encoded so dark gradients have no steps, soft shadows (a 4 x 4 filter of a bilinear comparison
  sampler, 1.5 texels wide, no acne or dither), bloom still only from the emission buffer, and edge
  smoothing by a cheap luma-edge filter (FXAA's idea) in the composite pass. Multisampling was not
  used: the scene has three targets, so 4x would add well over 100 MB at 1440p and a resolve per target;
  the filter costs no memory and works on WebGL2. Neither shader had grain or dither already (checked
  `scene.wgsl` and `post.wgsl`). Hooks (our renderer files, small): `Uniform.look` (x shadow softness,
  y edge smoothing; 0 keeps the arena's behaviour) in `renderer/mod.rs`, `scene.wgsl` (the soft
  filter behind `look.x > 0`), `post.wgsl` (`smooth_edges` behind `look.y`) and the frame colour format
  in `renderer/gpu.rs`. Checked in the real app: the valley, shadows and sky are smooth, the arena looks
  as before. Not checked: the browser build beyond `cargo xtask wasm`; a slow GPU (the shadow filter
  is 16 taps a pixel).
- G11.1 (2026-11-11): which game a file plays. `GameSettings::kind` (`figcraft_doc::game::GameKind`: First
  person, the default, or Third person; older files read as first person and are saved byte for byte as
  before); the undoable command `game.kind.set {kind: fps|thirdPerson}` (`cmd/game_kind.rs`); the "Game"
  dropdown first in the Prototype tab (nothing selected); a new design file (File ▸ New, the + tab, Home)
  shows a "Choose the game" card with two choices (Esc or the X keeps first person; programmatic
  `file.new` never shows it); Play runs the chosen game. Third person is a **placeholder** that proves
  the path: `crates/game/src/tps/` (fixed 60 Hz step, flat ground with scattered blocks, a faceted
  capsule hero moving 360° relative to the camera with acceleration and smoothed facing, jump, an orbit
  camera turned by mouse and right stick on its own, hero centred, drawn between two steps), drawn by the
  same renderer, theme, fog, key light and shadow (`renderer/tps/`). Input is the file's own bindings
  (WASD, mouse, Space, Shift, C, left/right stick, A). The first-person game is unchanged.
  Checked in the real app on a scratch file: the + tab showed the card, "Third person" set the Game
  dropdown, Play drew the hero and the ground with the same shadow and fog, W walked 5 m/s, the mouse
  turned the camera with the hero staying put and centred, W then went the way the camera looked and the
  hero turned to face it, Space jumped and landed; switching the dropdown back to First person and
  pressing Play gave the usual arena. Not checked: a controller (the same input path), the browser build
  beyond `cargo xtask wasm`, 60 fps on an iGPU.
  File ownership for the next phases (so G11.2 and G11.3 can run side by side without touching each other):
  - G11.2 World owns `crates/game/src/tps/world.rs` (+ new files for terrain, props, climb tags) and
    `crates/ui-egui/src/game/renderer/tps/world.rs` (+ new files). It keeps `World::ground_height`,
    `World::objects` (or replaces it by a mesh the renderer batches) and `World::spawn` as the boundary.
    It may add grass/terrain pipelines in `renderer/gpu.rs` (the `statics(third_person)` switch).
  - G11.3 Hero and animation owns `crates/game/src/tps/hero.rs` (`HeroPose`: the input of the rig; add
    fields it needs, such as lean or a clip time) plus new rig/animation files in `crates/game/src/tps/`,
    and `crates/ui-egui/src/game/renderer/tps/hero.rs` (+ new files). It draws from `HeroPose` only.
  - G11.4 Movement and camera owns `tps/controller.rs` and `tps/camera.rs`; G11.5 adds its own files.
  - Shared and small: `tps/mod.rs` (the `Tps` struct, `Frame`) and `renderer/tps/mod.rs`: change them
    only to add a field or a call.
  - Not done yet in G11.1, on purpose: the game's own variables, events and bindings (third person
    still reads the first-person bindings and leaves `vida`, `municao`… at their starting values), the
    cards' still renders (they are small drawings made of lines), collision with the blocks, sound.
- G9 (2026-11-11): an opaque inside stroke the colour of what lies behind a shape (a dark ring on a dark
  frame) no longer shows a thin light line along its outer edge: the fill stops a pixel inside the
  stroke instead of on the same curve (rectangles, rounded rectangles, ellipses; `render/covered.rs`
  `fill_under_stroke`, tests in `render/stroke_edge_tests.rs`). Centre and outside strokes never had it.
  Dashed, per-side, translucent, blended, gradient strokes and other shapes draw as before.
- G0.1 (2026-11-03): `crates/game` (`figcraft-game`, pure simulation, L2) with the game's fixed
  60 Hz clock (`Game::advance`, `FixedClock`); `crates/ui-egui/src/game/` with `Play`, which
  drives the game from egui's frame time. Not shown in the app yet (Play mode is G1).
- Branding (2026-11-03): the app shows "Graybox by INDIRECT®" (window title and Windows file
  properties; "Graybox" in the Home wordmark and Help ▸ About Graybox). About credits FigCraft.
  Code in `crates/ui-egui/src/game/brand.rs`.
- Audit (2026-11-03): the big "feat: add graybox" commit (32b0bc22, another agent) brought most of
  G1–G6; checked in the app (Play scene, HUD bound to variables) and ticked in PLAN.md.
- G4 rework (2026-11-03): no screen roles. Any frame is a game screen; opening, pausing and
  freeing the cursor are ordinary prototype interactions. Pause game / cursor actions run with an
  overlay are undone when it closes (`game::Play::settle`). Game settings (controls, events,
  sounds, share, example screens) moved from a floating window to the Prototype tab with nothing
  selected. The starter kit's HUD, Inventory, Pause, Map and Death are examples built from plain
  interactions (`cmd::game::example_interactions`); `examples/game-starter.figcraft` regenerated.
- G5 (2026-11-03): sound checked end to end on desktop (import, Play sound from a key trigger in
  Play, no errors). Added the player's own Volume slider in Play, bottom right while the mouse is
  free; it scales every sound on top of the file's master, group and sound volumes and is kept
  for the app run. `cargo xtask wasm` passes.
- G6 (2026-11-03): components carry their behaviour. Instances already played their main's
  interactions (sounds, game events); "Play animation" now aims at the instance's own copy of
  the layer (`cmd::game::aim_animations_at_instances`, via each layer's `source`), so a button
  that pulses its glow pulses the hovered button, not the main component.

- G4 navigable menus (2026-11-03): any auto layout frame (list, row, grid) can be switched on as
  "Navigable menu" in the Prototype tab (`game.menu.set`, stored in `GameSettings::menus`). In
  Play, focus moves with arrows/WASD/D-pad/left stick and the mouse, holding repeats (0.4 s, then
  every 0.12 s), Confirm (Enter/Space/A) runs the item's On click (or On press), Back (Escape/B) closes the
  overlay. A wrapping vertical layout navigates like a grid (↑↓ in a column, ←→ across).
  Items that are instances of a set with a `State` property show `Focused`/`Pressed`;
  others get an orange outline. Navigation math in `figcraft_game::menu`, focus in
  `ui-egui/src/game/menu.rs`, applied by `present/game.rs`. Starter kit: Button, Tab and Grid
  cell are State sets; the Inventory grid, Pause buttons and library List/Grid are menus. Checked
  in the app (Pause list with ↓, Inventory grid with the D-pad). Spec and plan in `docs/game/specs/`.
  Fix (2026-11-04): the mouse finds menu items through layers drawn over the menu (a monitor
  bezel image covered the whole screen and hid the items from the pointer).
- G9 CRT effect (2026-11-06): new effect type `crt` (`figcraft_doc::Crt`, our `doc/src/crt.rs`),
  a backdrop effect drawn like glass by our `render/src/crt.rs`: bulge, red/blue split, glow,
  box-filtered scanlines with bleed, phosphor columns, vignette, brightness, all in the layer's own
  coordinates. A render with a visible CRT in view on its page is drawn in one piece (no bands),
  canvas patches redraw CRT layers whole, and its backdrop reads clamp at a cut view's edge; the CRT clears its shape on the surface below as far as its picture
  will not cover it, so in Play (transparent frame) the unbent layers never show. In Play it bends the UI layers only, not the 3D scene.
  Figma and Vector Drawable exports leave it out with a warning. Checked in the app: canvas,
  zoomed to 30 % and 300 %, Play. Spec and plan in `docs/game/specs/2026-11-06-crt-effect-*`.
- Play never waits for a heavy screen (2026-11-06): a full-screen CRT took ~460 ms a redraw and
  froze the game on every HUD change (a shot with ammo, a hotbar slot). The CRT now shades rows
  in parallel, blurs its glow at half/quarter size and reads one channel per split (~140 ms), and
  Play redraws a changed screen off the UI thread (`game/frames.rs`): the last image shows until
  the new one is ready, newest request wins. The browser draws at once, as before.
- G9 Unity Live Link run in a real Unity (2022.3.39f1, Personal licence, 2026-11-10): a synthetic
  screen (9-sliced panel with stroke and radius, gradient shape, texts left/centre/right/middle,
  constraints left/right/centre/stretch/scale, three levels of nested frames, one translucent
  layer) exported by `figcraft-cli run`, imported in batch mode with the new
  `GrayboxLiveLink.ImportBatch` (`-executeMethod`, `-grayboxFolder`, optional `-grayboxScene`), and
  the Canvas rendered to PNG through a camera: it matches Graybox's render (only the font differs:
  Unity has no Inter) both at the design size and with the screen made larger (anchors act as
  Graybox's constraints). Both text paths (TextMeshPro, legacy Text) import clean; a second export
  after edits (move, recolour, text, rename, delete, add, fill removed) updates in place with no
  duplicates. Found and fixed: Watch never fired (Mono's `FileSystemWatcher` filter `layout.json`
  does not match files in sub-folders: now watches all files and picks `layout.json` by name);
  opacity was applied twice (sprite and Image colour: now the sprite is exported opaque and the
  layout's opacity becomes a `CanvasGroup`, which the layers inside inherit); auto-width texts
  wrapped in Unity's wider font (the export now carries `text.autoResize`; auto width stays on one
  line); a layer that lost its fill kept its old Image; the package had no `.meta` files (a
  `GrayboxNode` in a scene is referenced by its script GUID, so those are committed). Not tried:
  the window's buttons by hand (the editor opens the window and its Watch runs, headless).
- G9 Live Link prepared for the Unity 6 (6000.3) project of a collaborator, and for pixel art: a
  "Pixel art (no smoothing)" switch in the window (`GrayboxLiveLink.PixelArt`: Point filter, no
  compression); `FindFirstObjectByType` under `UNITY_2023_1_OR_NEWER`. Compiled against 2022.3.39f1
  (TextMeshPro, legacy Text, and the 2023+ branch); NOT run in Unity 6 (not installed here), and
  `enableWordWrapping` may warn there (see the package README).
- G10.1 (2026-11-11): chat with Claude. A speech-bubble button right after Play (`Icon::Chat`, drawn
  in code) toggles a floating Claude panel (bottom right of the canvas, left of the Design panel;
  resize by dragging its top-left corner): messages with small Markdown, a one-line trace of what
  Claude does, Send (Enter; Shift+Enter is a new line), Stop, New chat, per-file conversation kept in
  memory. Code in `crates/ui-egui/src/game/claude/` (`events` stream-json parser, `runner` spawns
  `claude -p … --output-format stream-json`, `discover` finds `claude` and `figcraft-cli`, `state`,
  `panel`, `markdown`, `guide` = the Graybox system prompt, `control` = the port). Command id
  `game.claude.toggle`. The browser build shows the button and says the chat needs the desktop app.
  Checked in the real app on a scratch file with a **stand-in `claude.exe`** that speaks stream-json
  and drives the real `figcraft-cli mcp --attach`: "create a 400×300 frame named Menu with a centred
  title" made the frame and title (the agent avatar showed), "make the title orange" (a second
  message, `--resume`) changed that same title to #FF8800, Stop ended the process tree and said
  "Stopped.", and the real `claude` printed the sign-in message in the panel (his login had
  expired). After he signed in again, the same three steps were run with the **real Claude** (2026-11-11,
  scratch file): it made the Menu frame (auto layout, dark navy, bold white "Menu" title centred) and
  said what it did; "make the title orange" changed that same Title to #FF8A00; Stop ended
  `claude.exe` and its tool server (no `figcraft-cli` left) and showed "Stopped.". The first reply
  takes about 15 s to start ("Starting…").
  Part 2 (G10.2 icons) is not built; the runner takes extra image paths (`Turn::images`) and returns
  `Event`s, which is what it will use.

- G16.1 (2026-11-12): pixel art core and drawing. Plan: pure crate first (canvas, raster, brush and
  fill, sprite model, storage in plugin data), then the engine commands, the sharp render hook, and
  last the Pixel mode editor on the canvas. Built:
  - `crates/pixel` (L2, pure): `Canvas` (RGBA, max 1024 a side), raster lines/rects/ellipses
    (Bresenham, Zingl), pixel-perfect strokes, square/round brushes 1–64, mirror axes on half
    pixels, tile wrap, bucket fill (contiguous or global, tolerance), the `Sprite` model (layers,
    frames, tags, palette) and its store: sprite JSON under `graybox.pixel`, one PNG cel per
    `graybox.pixel.cel.<layer>.<frame>`. The layer's image fill is the flattened frame 0, so exports,
    Figma files and older Graybox builds still show the art.
  - Commands `pixel.create`, `pixel.stroke`, `pixel.shape`, `pixel.fill`, `pixel.clear`, `pixel.pick`,
    `pixel.get`, `pixel.set`, `pixel.mode` (`crates/engine/src/cmd/pixel.rs`, tests in
    `pixel_tests.rs`, all in the junk sweep). Each stroke is one undo step.
  - Render: a pixel layer is drawn with nearest-neighbour sampling (`crates/render/src/pixel.rs`),
    sharp at any zoom, in the canvas, exports and Play.
  - Pixel mode (`crates/ui-egui/src/game/pixel/`): "Pixel" in the mode switcher, double-click a pixel
    layer, or Edit pixels in the new Pixel art section of the Design panel. A tool pill replaces the
    toolbar: Pencil B, Eraser E, Bucket G, Eyedropper I (Alt holds it), Line L, Rectangle U, Ellipse
    Shift+U; two colours (right button draws the second, X swaps), brush size [ ] and shape, Pixel
    perfect, Filled, Contiguous, Mirror X/Y (pink axis handles to drag), Tile, Grid (shows from 6
    screen px per art pixel), Done/Esc/Enter. Shift-click draws a line from the last point. Over the
    sprite: checkerboard for transparency, live preview of the stroke, brush outline with its mirror
    images. With no pixel layer selected, Pixel mode makes a new sprite by dragging (or a click / the
    Create button uses the size in the pill). A stroke released off the canvas still lands.
  - Checked in the real app on an Untitled document: sharp at 100 % and 293 %, grid and checker,
    Pixel art section, the pill, a mouse stroke with one "Draw pixels" undo step, handles hidden.

- G16.2 (2026-11-12): selection, transform and colour. Plan: pure pieces first (selection mask,
  transforms, palette files, sprite layers), then the commands, then Pixel mode and the panel.
  Built:
  - `crates/pixel`: `select::Mask` (rect, lasso polygon, magic wand contiguous or global with
    tolerance, add/subtract/intersect, invert, move, outline edges merged into runs, stored packed
    under `graybox.pixel.selection`), `transform` (move, flip, quarter turns, outline, replace colour,
    copy/paste, clip to the selection), `palette` (`.gpl` and `.hex` read and write), sprite layers
    (add, remove, move). Every drawing command keeps to the selection when there is one.
  - Commands `pixel.select`, `pixel.transform`, `pixel.outline`, `pixel.replaceColor` (one sprite or
    `ids`, every layer and frame), `pixel.copy`, `pixel.paste` (a PNG or any layer drawn at the
    sprite's scale, at most 1024 art pixels a side), `pixel.layer.add/delete/set/move/merge`,
    `pixel.palette.set/export` (`crates/engine/src/cmd/pixel/edit.rs`); in the junk sweep.
  - Pixel mode: Replace colour R (paints only over the second colour), Marquee M, Lasso Q, Magic
    wand W, Move V. The pill groups Line/Rectangle/Ellipse and Marquee/Lasso/Wand in one button each
    (a corner mark; a click on the group in hand takes the next tool). Shift adds to the selection,
    Alt takes away, both keep the overlap. Marching ants; the Move tool and the arrows (Shift: 10)
    move the selected pixels; Flip H, Flip V, Rotate and Outline buttons with the selection tools.
    Ctrl+A / Ctrl+D / Ctrl+Shift+I select all, none, invert; Ctrl+C / X / V copy, cut and paste
    pixels (also as a picture on the system clipboard); Delete clears; Esc lets go of the
    selection first, then leaves.
  - Design panel: while a sprite is edited, the Pixel art section lists its sprite layers (eye,
    pick the one drawn on, opacity, up, down, merge down, delete, +) and the palette (the sprite's
    colours, Load/Save `.gpl`/`.hex`, then the file's colour styles and colour variables: click
    takes the first colour, right click the second). New section "Pixel colors" under Selection
    colours: the colours of the selected pixel layers (also inside selected frames); a click opens a
    picker beside the panel and Replace swaps that colour everywhere in those sprites (one undo).
  - Checked in the real app on an Untitled document: marquee and wand selection with marching ants,
    a Move tool drag, the sprite layers and palette in the panel, Pixel colors replacing a colour.
- G16.3 (2026-11-12): pixel art for game UI. Plan: the pure smart resize first (worked examples),
  then the renderer, then the commands (9-slice in art pixels, Pixel text, Pixel button, palette
  swap), then the panel. Built:
  - `crates/pixel/src/resize.rs`: `fit` regrows a frame to any art size by whole pixels: without
    9-slice the middle row and column repeat (Middle, default), the last ones repeat outward
    (Edges) or the sprite repeats (Tile); with 9-slice the corners stay and the edges and middle
    repeat or stretch by whole pixels. Sprites keep `resize`, `sliceFill`, `slicePadding`.
    `transform::swap_colors` swaps several colours at once.
  - Renderer: a pixel layer at another size than its sprite is regrown, never scaled; an instance
    of a pixel component draws its main's pixels (`pixel::source`), so editing the main updates
    every instance. Pixel text (`graybox.pixelText` = art scale): a text layer drawn at one pixel
    per art pixel on its parent's art grid, each pixel ink or not, then scaled up sharp; its
    effects come out pixel-sharp too, its opacity applies to the result.
  - Commands: `game.nineSlice.set` on a pixel layer snaps the cuts to whole art pixels (a new
    slice cuts a quarter of the shorter side), works with auto layout, and gives the cuts as the
    frame's padding; `pixel.set` takes `resize`, `sliceFill`, `slicePadding` and keeps the cuts in
    art pixels when the scale changes; `game.image.toBackground` moves a sprite onto its frame;
    `pixel.text`; `pixel.button` (a component set State=Default/Hover/Pressed: frame with auto
    layout, 9-sliced original art, its cuts the padding, a Pixel text label, hugging);
    `pixel.paletteVariant` (a new variant, Palette=<value>, with colours swapped in every pixel
    layer in it). Both starters are one undo step. Pixel commands refuse an instance.
  - Panel: Pixel art shows "When resized: Middle / Edges / Tile", or with 9-slice "Repeat /
    Stretch" and "Cuts as padding"; the 9-slice section reads a pixel layer's cuts in art pixels;
    a Pixel text section for text layers (on/off, its pixel scale); a "Pixel button" button in the
    New sprite pill; "As variant" beside Replace in the Pixel colors picker when a component is
    selected.
  - Checked in the real app: Pixel button from the pill (three sharp states, a longer label grows
    the button), a sprite resized with Middle, 9-slice and Tile, the Pixel text section, and a
    palette swap made into a variant from the Pixel colors picker.
- G16.4 (2026-11-12): animation. Plan: the pure playback first (`crates/pixel/src/anim.rs`: which
  frame shows at a moment, per tag and direction, worked examples), frame and tag edits on the
  sprite (tags follow inserted and deleted frames), then the commands (`pixel.frame.*`,
  `pixel.tag.*`, `pixel.set {play}`), then the renderer (a pixel layer shows its frame at the
  presenter's time, and the animation clock wakes Prototype and Play), then the timeline strip in
  Pixel mode (frames × layers, durations, drag, onion skin, play) and a "Plays" row in the panel.
  Built:
  - `crates/pixel/src/anim.rs`: `sequence` (a tag's frames in its direction; ping-pong shows each
    end once a turn), `frame_in` / `frame_at` (the frame at a moment from each frame's duration,
    looping, or stopping on the last frame after `repeat` runs). Sprites keep `play` (the tag
    played in Prototype and Play) and tags keep `repeat`; `insert_frame`, `remove_frame`,
    `move_frame` keep tags in step.
  - Commands: `pixel.frame.add {index?, copy?, duration?}` (new or duplicate), `pixel.frame.delete
    {frame | frames}` (one undo step), `pixel.frame.move`, `pixel.frame.set {frame | frames,
    duration}`, `pixel.tag.add / set / delete`, `pixel.set {play}`. The image fill shows the
    played tag's first frame.
  - Renderer: a pixel layer shows the frame its tag is at at the presenter's time; the animation
    clock counts pixel layers, so Prototype and Play redraw when a frame is due (same path as
    animated GIFs).
  - Pixel mode: a timeline strip over the pill (frames across with number and duration, layers
    down with a dot per cel that has pixels, tags as coloured bands), play/stop, previous/next
    (`,` and `.`), new, duplicate and delete frames, one duration for the picked frames, drag a
    frame header to move it, Shift+click picks a run of frames, New tag, a tag row (name,
    Forward/Reverse/Ping-pong, Repeat, Plays in Prototype, Delete tag), and onion skin (the frame
    before in red, after in blue, under the one drawn on). The panel has a "Plays" menu.
  - Checked in the real app: a 4-frame bouncing ball with a ping-pong tag, onion skin, playing in
    the editor and in Prototype (the presented screen redraws as the frames change).
- G16.5 (2026-11-12): export and import. Plan: the pure exporters first
  (`crates/pixel/src/export.rs`: a `PixelExporter` trait over frames already drawn, with PNG, PNG
  per frame, sprite sheet + JSON, GIF and Unity; scale, tag, layers, trim, padding, extrude,
  power of two, layout), then sheet import (`crates/pixel/src/import.rs`: a PNG cut by a grid into
  frames), then the commands (`pixel.export` writes files or returns them, `pixel.import` makes a
  pixel layer), then the Unity screen export marking pixel layers and writing a sheet for animated
  ones, then the Live Link side (C#, through the `unity-livelink` agent, batchmode check), then
  the Export and Import section in the Pixel art panel with presets.
  Built:
  - `crates/pixel/src/export.rs`: `PixelExporter` (frames already drawn + options → files, no
    disk), with `Png` (first frame), `Frames` (`name_0.png`…), `Sheet` (PNG + JSON in the common
    sprite-sheet layout: `frames[]` with `frame`, `spriteSourceSize`, `sourceSize`, `duration`,
    `pivot`, `border`; `meta` with `frameTags`, `slices`), `Gif` (the tag in its direction, its
    repeats, exact colours up to 256) and `Unity` (a sheet whose `meta.unity` says Sprite Multiple,
    Point, no compression, pixels per unit, the tag to play). Options: scale 1–16, one tag, layers
    (hidden ones too when picked), trim (the empty border all frames share, so the pivot holds),
    padding, extrude, power of two, layout (packed: near square and identical frames stored once;
    grid; row). Caps: 8192 a side, 32 M pixels a picture, 64 M pixels a whole export.
  - `crates/pixel/src/import.rs`: a PNG (up to 8192 a side) cut by a frame size into frames, left
    to right then down; empty cells after the last painted one are dropped.
  - Commands: `pixel.export {id, format, dir?, …}` (writes into `dir`, or hands the files back as
    base64 for Claude, the CLI and the browser; in the junk-sweep skip list since it writes) and
    `pixel.import {path | data, frameWidth?, frameHeight?, duration?, scale?, x?, y?, parent?}`
    (a new pixel layer, one undo step). The layer's 9-slice goes into the JSON (slices, borders).
  - Unity screen export (`unity_export.rs`, ours): a pixel layer's entry says `pixel: true`, and an
    animated one also writes `sprites/<id>_anim.png/.json` (`animation`), each art pixel as big as
    on the screen; a sheet too big to make is left out and the still sprite stays.
  - Live Link (C#, through the `unity-livelink` agent): `GrayboxSheet.cs` imports a Graybox sheet
    (Sprite Multiple, one sprite per unique frame, pivot and border, Point, uncompressed, PPU), a
    clip per tag (ping-pong without repeating the ends) and an AnimatorController whose default
    state is the played tag; an `AssetPostprocessor` does it for a PNG + JSON dropped into Assets.
    Screen import: `pixel` layers always import sharp; animated ones get an Animator on their
    Image (the still sprite stays at rest). Checked in Unity 2022.3.39f1 batchmode.
  - Pixel art panel: "Export pixel art" (presets Unity sheet, Sprite sheet + JSON, PNG, PNG per
    frame, GIF; scale, padding, which frames, Packed/Grid/Row, Trim; Export… asks for a folder, the
    browser downloads) and "Import pixel art" (frame size, Import… makes the layer beside it).
  - Checked in the real app: the panel, a GIF and a Unity sheet of the bouncing ball written to a
    folder, a row sheet imported back as a 4-frame layer, a screen with it sent to Unity.
- G16.6 (2026-11-12): polish. Plan: pixel fonts (a pure glyph-sheet → TrueType writer in
  `crates/pixel/src/font.rs`, a text-crate hook so a redrawn font replaces its earlier version, the
  `pixel.font.*` commands, a Font menu in Pixel text), then shade mode (`paint::shade` along the
  sprite palette, `pixel.stroke {shade}`, a pill toggle), then `pixel.paint` and the Claude guide,
  then a measured performance pass, then the guide `docs/game/pixel-art.md`.
  Built:
  - Pixel fonts: any pixel layer can be a glyph sheet (`pixel.font.set {id, name?, chars?,
    cellWidth?, cellHeight?, baseline?, spacing?, off?}`), or `pixel.font.sheet` makes a new one
    (6 × 8 cells, printable ASCII, a faint cell grid and baseline row drawn under half opacity so
    they aren't glyphs). Its first frame becomes a TrueType font (each glyph its painted pixels as
    squares, proportional: painted columns + spacing; an empty cell is half a cell wide) loaded
    under the font's name, so text lays out, wraps and hugs in it like any font. Drawing on the
    sheet reloads the font and re-measures the text that uses it; the app also reloads fonts
    whose sheet changed every frame (open, undo). `pixel.text {font}` sets text layers in it at
    one glyph pixel per art pixel. In Pixel mode a glyph sheet shows each cell's character and
    baseline. Panel: a Pixel font section (name, cell W/H, baseline, gap, characters, "n of m
    drawn"), and a Font menu in Pixel text. The new-sprite pill has a "Pixel font" button.
  - Shade mode (stretch goal of G16.2): a pencil option; the main button moves each pixel it
    covers one colour on along the sprite's palette, the other button one back (once per stroke,
    stopping at the ends, empty pixels and colours not on the ramp untouched). Without a palette
    the ramp is the cel's colours dark to light. `pixel.stroke {shade: ±n}`.
  - `pixel.paint {id, rows, palette, x?, y?, clear?}`: a sprite drawn from rows of characters in
    one undo step (a typo changes nothing), how Claude draws. The in-app Claude guide now explains
    pixel layers, `pixel.paint`, fonts and export; MCP reaches every `pixel.*` command through
    `run_command`, so no new MCP tool (no upstream hook in `crates/mcp`).
  - Performance (ignored timing test `pixel_tests::perf`, 256 × 256, 8 layers, 24 frames, noisy
    art, debug-optimised build): animation clock and sprite parsing 0.01 ms per layer per frame,
    render cache key 0.3–0.5 ms, a frame composed on a cache miss 6–11 ms, font refresh ~0 ms, a
    stroke 19 ms on the frame at rest (was 27–32) and 11–20 ms on another frame: drawing on a frame
    that isn't the one at rest no longer remakes the layer's image fill. No caches added for the
    clock or Pixel text (measured cheap; Pixel text draws at art size, scale² fewer pixels).
  - Checked in the real app: a glyph sheet with P, l, a, y drawn, a text "Play" set in it (sharp,
    hugging 120 × 48), the font loading again after the file was reopened, the cell labels, and a
    shade stroke each way on a 5-colour ramp.
  - Guide: [`pixel-art.md`](pixel-art.md) (pt-BR summary first). Review fixes: one sheet per font
    name (a copied sheet no longer reloads the font every frame; the panel says it is a copy),
    names avoid installed families, `shade` with `erase` is refused, the palette-less ramp keeps
    the most used colours, the glyph guide shows on the first frame only.
- G16 "Done when" check (2026-11-12, real app + Unity 2022.3.39f1 batchmode): a 24 × 24 button
  drawn with the mirror, 9-sliced, hugging a Pixel text label, as a component with Hover and
  Pressed variants each playing a 2-frame tag, its outline recoloured in every state, then sent to
  Unity both as a sheet (`pixel.export` unity: 2 sprites, border 8, Point, a "hover" clip) and on a
  screen (`game.unity.export`). The in-app Claude chat drew a 16 × 16 coin with `pixel.create` +
  `pixel.paint`. Found and fixed: a 9-sliced pixel layer on a screen went to Unity as one sprite
  with its layers baked in (the label cropped into the art); now the sprite is the art alone at
  its own size and the layers follow as children (`unity_export.rs`), checked in Unity (a sliced
  Image with a "Label" Text child, an Animator playing "hover"). Seen, not changed: "Cuts as
  padding" applies when the cuts are set, so turning auto layout on after 9-slice keeps padding 0
  until the cuts are set again or the option toggled.
- G16.8 (2026-11-13): four things Adriel asked for, one commit each.
  - **Pixel button and pixel fonts dropped.** He never asked for either. Gone: the two starters in
    the new-sprite pill, the Pixel font section and the Font menu in the Design panel, the glyph
    sheet itself (`crates/pixel/src/font.rs`), the `pixel.font.*` commands
    (`engine/src/cmd/pixel/font.rs`), the `pixel.button` command and `pixel.text {font}`, the
    per-frame font refresh (`game/fonts.rs`), their tests, the guide entries and the pixel-font
    parts of `pixel-art.md` and the spec. **Pixel text on ordinary text layers stays** — that one he
    wants. An old file that holds a glyph sheet still opens: the key is ignored, the art is an
    ordinary sprite, and a text layer set in the old font falls back to a normal one
    (`pixel_tests.rs::an_old_file_with_a_glyph_sheet_opens_as_an_ordinary_sprite`).
  - **The open sprite is titled above its board** (`game/pixel/title.rs`): while a sprite is open,
    its name sits just above the board's top-left corner in the canvas accent colour, like a frame's
    label. It is pushed onto the engine's own `tools::labels`, so clicking it selects the layer and
    dragging it moves the whole sprite with the canvas's existing rules. Closed, there is no title:
    the layer goes on hugging its pixels, which he likes.
  - **The art no longer jumps when a sprite inside an auto layout is drawn on.** Cause found by
    repro, not by reading: hugging (`board::retrim`) moves the layer on the page to keep the art
    still, and an auto layout puts it straight back, so the shown box changed while the position did
    not — the art shifted on every stroke, and closed it was cut to the wrong box, which read as
    "the edit came back old" or "Esc undid it". Fix: a layer an auto layout places shows the **whole
    board** (no trim, nothing moved); a layer marked absolute hugs as before. Tests:
    `pixel_tests/trim.rs` (two) and `game/pixel/tests.rs` (in a layout across open-draw-leave, an
    edit surviving Esc and reopening, undo taking back only that edit).
  - **The brush shape sits with the brush size** (`game/pixel/pill.rs`): one field reading
    `− n +`, a hairline, then two cells (square, circle) with the chosen one lit on an accent tint,
    tooltips on both halves. `[` / `]` and the shape shortcut still work. He had not found the old
    separate toggle.
  - Checked in the real app on a scratch file (a 16 × 16 "Coin" in a horizontal auto layout next to a
    rectangle): the title over the board, the whole board shown inside the row, a pixel drawn at
    (0, 0) leaving `absoluteBounds` at exactly 188, 116, 96 × 96, the art still there after Esc and
    after reopening, and the new brush field in the pill.
  - Three more the owner asked for the same day, each its own commit:
    - **The sprite's background turns off** (the checkerboard button beside the grid): the empty
      pixels become see-through, so a reference under the sprite can be traced. Only how the board
      is shown while editing — the pixels, the export and the layer in Design are untouched.
      `Editor::background` (remembered like the grid), `draw.rs` leaves the checker quads out, new
      `Icon::Checkerboard` drawn in code.
    - **A vector copied on the canvas pastes in as pixels**: `pixel.paste` grows `clipboard` (the
      copied layers are put on the page, drawn, and taken straight back out inside the one command)
      and `threshold` (default 50 %: alpha under it is left out and the rest comes in solid, so
      there is no feathered fringe; 0 keeps the alpha as drawn). A layer bigger than the board is
      fitted to it instead of refused. Pixel mode's Paste takes the newest copy, pixels or layers,
      by the clipboard's own copy count (`layers_generation`).
    - **Colours in Pixel mode.** The eyedropper reads the whole canvas: a painted art pixel gives
      its own colour, an empty one what is drawn under the sprite, and outside the board whatever
      the page shows (new `pixel.colorAt {x, y}`). The colour picker's own eyedropper works over a
      sprite too (Pixel mode lets a press through while `tool.pick_only` is set), and its colour
      goes to the drawing colour it was opened on, where the file's colour styles and variables were
      already offered. In the panel, while a sprite is open the Fill section is the two drawing
      colours (each opening the standard picker, the style or variable link kept, the header's swap
      button and `X` swapping them) instead of "image 100 %", and "Pixel colors" is now **Selected
      colors**, between Fill and Stroke.

## Decisions
- G16 pixel art, details the spec left open (chosen as a pixel artist would expect):
  - New sprites are 32×32 at scale 4 (one art pixel = 4 page units); default colours a near-black
    `#1b1d29` and an off-white `#f3f1ea` (G16.3; original, no third-party palette). Pixel perfect is on.
  - Pixel mode sits between Draw and Design in the mode switcher. Esc and Enter leave it, like Done.
  - The pixel grid shows only from 6 screen pixels per art pixel, so it never hides small art.
  - Shift-click with the pencil draws a straight line from where the last stroke ended.
  - Mirror axes sit on whole or half pixels (an even-width sprite mirrors around its middle line).
  - Smart resize (G16.3) default: the middle row and column repeat, so corners and borders stay;
    "edges" and "tile" are the other choices.
  - The Design panel title of a pixel layer still reads "Rectangle" (upstream); the Pixel art
    section under Position says what it is.
  - G16.2: a selection tool's Alt subtracts (it no longer holds the eyedropper there). A click
    without a drag deselects. Moving, flipping and Delete act on the current sprite layer; with
    nothing selected they act on the whole layer, and the arrows nudge the layer itself as before.
  - G16.2: Paste puts the pixels back where they were copied from, selected, with the Move tool in
    hand. Until pixels are copied in this session, a paste of a picture from another app still
    makes a new image layer.
  - G16.2: Outline draws outside the art in the first colour by default; Pixel colors lists at most
    32 colours, most used first.
  - G16.3: the default colours are now an original near-black `#1b1d29` and warm off-white
    `#f3f1ea` (the old pair matched a well-known palette's ends).
  - G16.3: a pixel layer's 9-slice is the same `graybox.nineSlice` key as any layer's, in page
    units that are whole art pixels; its base is the sprite's own size. Alt+drag scaling by whole
    multiples is left out: the Scale field does it, and Alt+drag already resizes from the centre.
  - G16.3: the Pixel button's label is 12 art pixels (8 broke up in a smooth font) until pixel
    fonts (G16.6); the set doesn't clip, so a state that grows with its label stays in view. A
    palette-swap variant is named by its new colour (`Palette=#c4833a`); rename it in the panel.
  - G16.3: the tileset preview is the Tile resize (a pixel layer made larger shows its sprite
    repeated); drawing in tile mode already wraps (G16.1).
  - G16.3: Pixel text ignores the text's blend mode, and a layer whose size isn't a whole number
    of art pixels has its art stretched by that fraction (keep pixel layers at multiples of the
    scale; the Pixel button's label line height is whole art pixels for that reason).
  - G16.3 review, left for the G16.6 performance pass: Pixel text is drawn again each frame (no
    cache yet), and its ink can sit up to one art pixel outside the text's own bounds (a redraw
    of that sliver needs an upstream bounds hook).
  - G16.3: new cuts keep any padding beyond the old ones (the Pressed button's lower label);
    turning "Cuts as padding" on sets the padding to the cuts. An instance of a pixel component
    shows (and `pixel.get` reads) its main's pixels; it isn't drawn on, recoloured or sliced as
    pixel art itself.
  - G16.4: switching what plays on hover or press is a variant whose sprite plays another tag
    (the Pixel button's Hover and Pressed states), so every prototype trigger already switches
    tags; no new prototype action. A layer with no tag chosen plays every frame. Design shows the
    first frame of what it plays (still); Pixel mode plays with its own button. The play button
    runs the tag the current frame is in (the played one first), else every frame, always looping.
  - G16.4: a new frame after a tag's last frame joins the tag (as you'd add a frame to `idle`);
    a moved frame leaves the tags where they are. Enter stays Done in Pixel mode (as in vector
    edit), so play has no key; `,` and `.` step frames. Onion skin is one frame each side, drawn
    under the frame drawn on, hidden while playing.
  - G16.4 review, left for the G16.6 performance pass: the animation clock reads each pixel
    layer's sprite description every time it is asked (each presented frame).
  - G16.5: export settings last for the session (not saved in the file). Presets: Unity sheet and
    Sprite sheet + JSON start with 1 px padding (no bleed between frames), GIF at ×4, PNG at ×1.
    Trim is shared by all exported frames (Aseprite's "trim sprite", not per cel), so one pivot
    fits every frame. Pixels per unit default 16 art pixels a unit (× the export scale); screens
    use the Live Link's 100 × layout scale so a sheet matches its still sprite. A GIF tag with
    Repeat 1 plays once; Unity clips loop unless a repeat is set (Unity can't count repeats).
  - G16.5 review, left: the export param parsers repeat the anim ones (small); an animated pixel
    layer resized without 9-slice exports frames at art size, so Unity stretches them (resize the
    art instead, or use 9-slice).
  - G16.6: a pixel font's em is the cell height rounded up to a power of two (8 for 6 × 8 cells),
    because text layout puts advances on a 1/1024-em grid: other ems give glyph pixels a fraction
    off and hugging text rounds up a pixel. Text in a pixel font is `em × scale` points. Glyph
    pixels are those at least half opaque (the sheet's guide tints aren't). The font is read from
    the sheet's first frame. Font names stay unique in a file ("Pixel Font 2"). Shade's ramp is
    the sprite palette in its own order (as Aseprite's shading ink uses a palette range), main
    button forward. A bitmap font export for Unity was left out (time).
  - G16.6 follow-ups Adriel asked for (2026-11-12):
    - **D6 changed: one colour system.** The pill's two colours and a Pixel colors swap open
      Graybox's own colour picker (`PickerTarget::Pixel {which}`: custom colour, then the file's
      colour styles by folder, colour variables, and "In this sprite"); the local pixel palette is
      gone from the panel (Palette: Import… makes colour styles from a `.gpl` / `.hex`,
      `pixel.palette.import`; Save… writes the sprite's colours). A colour picked from a style or
      variable keeps its link: painting records `colour → style:<id> | var:<id>` in the sprite
      (`Sprite::links`), Replace colour moves the link, and when a style changes the Pixel colors
      section offers "Update from styles" (`pixel.styles.follow`). Shade steps along the primary
      colour's style folder (or variable group), dark to light, else the sprite's colours
      (`pixel.stroke {ramp}`). The old `palette` field still reads and `pixel.palette.*` still work.
    - The Pixel art section comes last in the Design panel (after Effects) and folds away.
    - The timeline over the pill is off until turned on: a pill button (lit while on), Alt+T, or
      "Show pixel timeline" in Actions (`pixel.timeline {on?}`, kept in the app's View prefs, not
      the file). Hidden, the sprite shows the frame being edited; Prototype and Play still play.
    - The pill no longer fits a ~900-point canvas with the Timeline button added: when narrow it
      leaves the zoom label out, then folds the two mirror buttons into one that steps through them.
  - G16.7 pixel feedback from Adriel (2026-11-13):
    - **Board and trim (decided by the orchestrator).** In Pixel mode a pixel layer is a board: the
      Design panel's W and H (where a frame's sit) are its size in art pixels, and typing a size or
      dragging the board's sides adds or cuts art pixels there (`pixel.canvas {width, height,
      anchor | offset}`; anchor is the side kept, Alt drags from the centre; the scale stays).
      Outside Pixel mode the layer hugs its used pixels: `show()` calls `board::retrim`, which
      stores the box round the pixels of every frame's visible layers as plugin data
      (`graybox.pixel.trim`, `store::shown_box`; each cel's box is cached by the hash of its stored
      text, so a stroke decodes one cel) and, when the layer is at the size of the box it showed,
      moves and sizes it to the new box so the art never moves on the page. A layer resized in
      Design keeps its size (its art regrows by its grow mode); an empty board keeps the board's
      size. The renderer, the image fill and the Unity animation sheet draw `store::shown_image`
      (the trimmed art); 9-slice cuts and base are measured on it, and `pixel.export` (the whole
      board, as before) moves the cuts out by the margins. Pixel mode draws the whole board round
      the layer (`art_to_page` offsets by the trim). The Pixel art section reads "Board: W × H art
      pixels". A 9-sliced layer or an instance is not a board (no sides to drag, no W/H in art
      pixels).
    - The 9-slice section sits just above Export (it was under Layout).
    - 9-slice with components already worked on the main component, on its pixel layer and on a
      layer sliced before it became a component (instances resized keep the corners); now tested,
      and the section says "Applies to this layer; instances of the component resize with it" on
      a component or a layer inside one.
    - Mirror axes: the pink handles grab from 13 points away and turn the pointer into a resize
      arrow (Pixel mode tells the canvas it set its own pointer, `pixel::has_cursor`); with both
      axes on, a ring where they cross drags both. Handles win over the board's sides.
    - The pixel Text tool (text drawn into pixels) was started and cancelled by Adriel before it was
      committed; Pixel text and pixel fonts stay as they were.
  - G16.8 pixel feedback from Adriel (2026-11-13):
    - Pixel fonts and the Pixel button starter are **dropped, not hidden**: the model, the commands
      and their files are gone, so nothing carries their weight. Old files keep opening (an unknown
      plugin-data key is ignored and a missing font family falls back), but they do not come back.
    - **A sprite an auto layout places is its whole board.** Hugging and a layout cannot both decide
      where a layer sits, and the artist already sized the board, so inside a layout the slot is the
      board. Outside one, hugging is unchanged; `absolute` on the layer brings hugging back.
    - The sprite's title is a canvas label, not a new widget: it joins `ToolState::labels`, so
      select and drag behave exactly as a frame's own name does, with no new command.
    - The brush shape joins the size field rather than keeping its own button (Aseprite reads that
      way, and he looked for it there); the shortcuts stay so nobody loses them.
    - The sprite's background is a **view** toggle, nothing in the file: an exported sprite and the
      layer in Design look the same whether it is on or off.
    - A vector pasted into a sprite is **rasterised, not linked**: it becomes ordinary pixels to be
      edited, and the vector it came from stays on the page untouched. Alpha is cut rather than
      feathered by default, since a pixel artist wants a sharp edge; `threshold: 0` keeps the alpha
      for anyone who wants the soft one. No new command: `pixel.paste` already drew a layer as
      pixels, so it gained `clipboard` and `threshold` instead of a second path.
    - The eyedropper takes what is **rendered** at that point, so over empty canvas it picks the
      page's own colour (the app's own eyedropper does the same). Picking transparency by clicking
      an empty art pixel is gone: the eraser is how a pixel is cleared.
    - "Selected colors" sits where the owner asked (between Fill and Stroke) and keeps everything it
      did. The app's own upstream section is "Selection colors", under Effects: a near name but a
      different thing, far apart in the panel, and he named this one himself.
- G11.4 movement (recommended option each time):
  - No physics engine: one cylinder against the world's axis-aligned boxes (two passes) plus the
    heightfield; a few hundred boxes per step is cheap at 60 Hz and the same on wasm.
  - Roll is Ctrl / controller X by default without adding an action to `GameSettings` (adding one would
    change how every saved file reads back); a file can still bind "roll" itself. The spec's B is crouch
    in the first-person defaults, so X was free.
  - Falls only knock down for now (no health yet); the damage numbers of the spec (4 m / 12 m) come
    with G11.5's health.
- G11.5 strikes (recommended option each time):
  - Attack is the existing "shoot" binding (left click / RT); weapons are the number keys 1–4. D-pad and
    wheel switching wait for G11.5's bindings.
  - The sword combo counts a click after 35% of the current strike; a strike blocks jump, roll and
    climbing until it ends.
- G11.5 foes and the bridge (recommended option each time):
  - Foes are cylinders pushed apart after moving, no pathfinding: husks walk straight at the hero and
    slide along solids. Health lives in the simulation; the Game variables carry it to the UI.
  - Arrows fly parallel to the camera from the shoulder. The aim dot sits on the hero's back until G11.6
    puts the camera over the shoulder; then the arrow aims at the point under the dot.
- G11.6 bow (Adriel's spec; recommended option where it left a choice):
  - Aim mode needs the bow: right click does nothing with a melee weapon.
  - Mouse 4 and mouse 5 both swap (a toggle), so the controller needs one button for it.
  - The crosshair point is computed in the simulation and arrows aim at it, so the dot never lies, even
    with the shoulder offset. The game draws only the dot; the designed UI can replace it.
- G11.6 camera, crowns and browser (the recommended option each time):
  - Easing back waits while the player steers instead of slowing down: turning the camera turns the
    stick's directions, and the hero would run in circles.
  - The view trails the hero only in a jump or a fall; on the ground it follows exactly.
  - Crowns fade with a screen-door pattern (discard), not blending: nothing to sort, the same on WebGL2,
    and the shadow stays whole.
  - Mouse 4/5 are cancelled only while a game plays, from one capture listener on the window (eframe
    already turns them into the Extra1 and Extra2 buttons).
- G11.2 world (Adriel was away; the recommended option each time):
  - The ground is a heightfield with alternating triangle splits, and the walking height is looked up in
    those same triangles, so what you see is what you stand on. Only integer hashing and smoothstep make
    it, so the golden checksums in `world_tests.rs` hold on every machine and in the browser.
  - A tuft is 3 blades in one instance (a third of the instances for the same look). Each chunk row of
    8 m is its own draw, with the vertex-buffer slice moved instead of `first_instance` (WebGL2 has none).
  - Third person has its own shadow map (2048 on desktop, 1024 on web); its texel size goes to the shader
    in `Uniform.bloom.w` (0 means the first-person 2048), so the arena is untouched. Near and far planes
    are 0.1 and 420 here (0.05 and 200 in the arena).
  - `scene.wgsl`'s fragment body moved into `fn shade` (entry points cannot be called) so the water
    fragment can light itself the same way; the arena's output is unchanged.
  - The third-person theme dims the arena's (fog x0.55, key x0.55, hemisphere x0.6, ambient x0.7), turns
    pixel textures off (clean look) and puts the one point light on the watchtower beacon.
  - Mountain and keep cards sit outside the walkable map and are flagged like the first-person gun: no
    fog and no scene lights, so they stay dark silhouettes. Orange is only emissive: the ring, the beacon,
    the tower windows and the waterfall.
- G11.3 hero and animation (Adriel was away; the recommended option each time):
  - Skinning runs on the CPU: each bone has its own rigid mesh, built once; every frame its vertices
    are moved by the bone matrix into the dynamic vertex buffer the hero already used, so the hero stays
    one draw call with its shadow and needs no shader change (no clash with G11.2's pipelines; works on
    wasm). A GPU bone buffer can come in G11.6 if profiling asks for it.
  - 18 bones (limit 24); keys hold Euler angles in degrees, quaternions at run time.
  - Only the equipped weapon is drawn (`Weapon` lives in the animator; G11.5 will set it). The bow is
    held in the left hand, and draw and sheathe are mirrored for it. The weapon moves from back to hand at
    a time written in the clip (`swap_at`). Weapons are angled along the forearm in the hand.
  - Crossfades start from the pose on screen and carry on with its own angular speed, which dies out (a
    light inertialization), so a change mid-swing never pops. The gait speed goes through a spring before
    it picks the blend.
  - The clip preview is debug-only (F6/F7, `cfg!(debug_assertions)`); it freezes movement input and
    brings the camera to 3 m.
  - Clean look (Adriel's change to the spec): the hero, cloak and weapons are solid flat-shaded parts.
    Each part only names a grey tone (0-5) of the scene theme, with no colour, texture or emissive of its
    own, so the third-person theme G11.2 owns (`renderer/tps/mod.rs`: palette, fog, no pixel textures)
    decides how it looks. An orange or white glow, if one is wanted later, would be a small emissive
    detail through the same material, not a second palette.
- G11.1 game kind (Adriel was away; the recommended option each time):
  - Both games take the same `figcraft_game::Input` (filled by `game/input.rs` and the presenter), so
    bindings, gamepad sticks and "the mouse is free" behave the same. The third-person simulation is a
    separate type (`figcraft_game::tps::Tps`), held next to the first-person `Game` in `Play`
    (`Play::tps`); Play picks one with `game::tps::sync` from the file's `kind` and routes the frame step,
    the drawing, the help line and `inspect` through `game/tps.rs`. A third kind later is one enum entry
    plus one more `Option` there.
  - The renderer keeps one scene pipeline: a `Snapshot` carries `focus` (where the key light's shadow map
    centres: the hero in third person, the origin in first person) and an optional hero. Third person
    gets its own static buffer (built the first time it is drawn) and no first-person portal light.
  - Camera yaw and pitch change every display frame (a mouse turn shows at once); the hero moves in fixed
    steps, using the camera yaw at that step. The picture is interpolated between two steps.
  - Jog is the default speed (5 m/s), a light stick walks slower, Shift runs (8.5), crouch (2.4) only
    slows down for now (no lower capsule). Gravity 20, jump 7 (about 1.2 m).
  - The new-file card is skipped in this crate's own tests (`cfg!(test)` in `kind_ui::after_run`): they
    create files all the time. The card is shown by `FigApp::run`, so File ▸ New, the + tab and the Home
    cards ask; the control channel (`engine.execute`) and `main.rs`'s first file never do.
- G10.1 Claude chat:
  - The CLI runs with `--strict-mcp-config` (only the `graybox` server), `--tools Read` (reading the
    images of G10.2), `--allowedTools "mcp__graybox Read"`, `--permission-mode dontAsk`,
    `--setting-sources project` (his user settings, plugins, hooks and skills stay out of it),
    `--disable-slash-commands`, in a scratch folder `%TEMP%\graybox-claude`. The prompt goes through
    stdin. `ANTHROPIC_API_KEY` is removed from its environment so it always uses his plan.
  - Only the native `claude.exe` is used (PATH, then `~/.local/bin`, `~/.claude/local`): a `.cmd`
    wrapper (npm install) cannot take the guide as an argument on Windows.
  - The control channel: with `--control` it is the one already open; otherwise `apps/figcraft`
    opens one on a free 127.0.0.1 port when the first message is sent (`chat_control.rs`). From then
    on that port stays open for the run (anything on this PC can drive the app through it, as with
    `--control`).
  - Chat state is a thread-local in the `claude` module (not in `FigApp`), so upstream state is
    untouched. Edits go to whichever file is active when Claude calls the tools; if he switches tabs
    mid-turn they land in the other file.
  - Stop kills the `claude` process and what it started (`taskkill /T` on Windows) and shows
    "Stopped." at once.
  - Errors are plain: not installed, `figcraft-cli` missing, not signed in (anything that looks like
    a login/401 problem), otherwise the last lines the CLI said.
  - The install now ships `figcraft-cli.exe` beside `graybox.exe` (`graybox-update-stable` skill);
    his installed copy in `C:\Users\Adriel\Apps\Graybox` does not have it until the next update.
- Play redraws its whole screen when a variant changes on hover. Costly unchanged layers (layer blur,
  big image fills) are kept between renders instead (owner's "essa aqui" screen: ~450 to ~120 ms per
  hover, identical pixels). Going further would mean redrawing only the changed area.
- Fonts (owner, 2026-11-04): Google Fonts / Noto families download to this computer only when picked
  (font picker's "Google Fonts" rows), never into the repo or the program. Sharing (`game.share`)
  puts the font store fonts the text uses into the file (`game.fonts`, base64, capped); whoever
  opens it loads them (`game::fonts::load_shared`, desktop and browser). System fonts never travel.
- State looks without special names (owner, 2026-11-04): hotbars and navigable menus show selected /
  focused / pressed / disabled with a variant the designer picks from the items' own variants
  ("When selected: Property 1 = hover"), or Auto (found by names like hover, selected, focused,
  pressed, disabled), or an orange outline. "Create a variant for it" adds one. Warnings say when
  an item is the original component or not a component copy. Older State=Focused files work on Auto.
- Hotbars (owner, 2026-11-04): an auto layout row marked as a hotbar keeps one selected slot in Play:
  keys 1-9 pick, the wheel (unless a map is open) and LB/RB step, without stopping the player. The
  selected slot shows its State=Focused variant.
- Wireframe hotbars (owner, 2026-11-06, experimental): when no slot is a component copy and the
  first slot is drawn differently from the second, the first slot is the selected look and the
  second the plain one. Picking slot 4 gives slot 4 the first one's look and the first the
  second's, at once, no animation: size, scale and rotation, fills, strokes, effects, corners and
  opacity move; what each slot holds and its place stay (auto layout reflows). Play only, the file
  is untouched. Identical slots keep the orange outline (`game::hotbar::base_looks`).
- Any monitor (owner, 2026-11-06): in Play the screen frame as designed is the reference size. It is
  scaled until its width or height meets the monitor's (nothing cut, no bars) and stretched along
  the other side to the monitor's shape; the layers' Constraints (Design tab) anchor them: Right +
  Top keeps a minimap in that corner, Center the crosshair in the middle, Left & Right stretches.
  A 16:9 design on a 16:9 monitor looks as before; 4K is the same layout twice as big. Overlays keep
  their designed size. The presenter's Fit / Fill options don't apply in Play (`game::screen_fit`).
- Named inputs (owner, 2026-11-04): the designer names keys/buttons (Prototype, nothing selected →
  Game inputs); a press fires the Game event of that name on the screen on top only, so one key
  can open a screen and close it from that screen. Every file starts with `pegar-item` on E /
  D-pad right. Keys can't clash with gameplay actions or other inputs.
- Maps (owner, 2026-11-04): any frame can be a map (Prototype tab). In Play, while the mouse is
  free, its content pans (mouse drag, left stick) and zooms (wheel; right stick or R2/R1 as chosen),
  between the chosen limits. The view is Play state only.
- Game behind a screen (owner, 2026-11-04): per top-level frame, Game running (default) / Game
  paused / No game, decided by the screen on top (overlay, else the frame). Paused and No game
  freeze the simulation and free the mouse. Game bars: a layer follows vida, municao or reserva as
  a share of its maximum (health max, magazine, starting reserve) and keeps its drawn width as
  the full mark; the binding of a variable to W stays as upstream has it.
- Component panel like Figma (owner, 2026-11-04): a set lists each property on one compact row
  that opens its editor; a variant shows only "Current variant" (its own values, changing one
  renames the variant). Not yet in the compact panel: drag to reorder properties, instance-swap
  default and preferred components (upstream's matrix had them). Upstream's panel tests run with
  the compact panel off (`game::components::COMPACT`).
- Navigable menus (2026-11-03): menu keys live in `menu_bindings`, separate from gameplay keys
  (WASD may move the player and the menu); a menu owns input only while the open screen has one,
  and the player stops moving meanwhile. A key/controller interaction on the open screen for the
  same input wins over the menu (Escape → Close overlay closes one overlay, not two). The first
  menu in layer order owns focus (tab bars switching menus are phase 2). A focused Disabled item
  keeps its look and gets the outline.
- Game screens are free-form (owner, 2026-11-03): no predefined screen types. The kit's screens
  only show what interactions can do. Files with old `screens` or inventory/pause/map bindings
  still open; those are dropped on load.
- Escape in Play frees the mouse only when no screen is open and the frame has no Escape
  interaction of its own (so a pause menu on Escape works). F10 always leaves Play.
- G9 (2026-11-06): a click on a top-level frame's background selects the frame (its hover outline
  already suggested it); a drag there still draws a marquee. Selection colors folds past three
  colours. Auto layout glides last 0.18 s (cubic ease in and out) and only on the canvas. A frame
  fill hidden by a covering layer isn't drawn, which removes the light line around screenshot
  frames. "Padding 0 stays 10" not reproduced (frame, instance and main all apply 0): tests pin it;
  waiting for the owner's steps.
- G9 anchoring (2026-11-07): a layer in a frame without auto layout takes the constraints of the
  third (each way) holding most of it; two thirds or more, or first third into last, stretches. Only
  when its middle is inside the frame (scroll content keeps its own), not inside instances, and not
  when the frame itself was resized (its layers move by their constraints then). Set by hand →
  `GameSettings::anchored` (not plugin data: `.fig` export warns about plugin data). Older layers
  change only when moved or with "Anchor layers by position".
- G9 corner styles (2026-11-07): `Corners::style` (`figcraft_doc::CornerStyle`, our
  `doc/src/corner_style.rs`): Round (upstream's, the only one smoothing shapes), Chamfer (straight
  cut, CSS `bevel`), Scoop (concave quarter circle around the corner, CSS `scoop`), Notch (square
  step, CSS `notch`), V-notch (a V opening half the size along each side, its tip at the full size
  on the diagonal). Every style fits the same size × size square, clamped like round corners, so
  the radius field grows and shrinks it. Round is not written to files (older files load as they
  were). Strokes offset each cut parallel to itself (mitred); other joins take the generic stroke
  outline. `node.set` `cornerStyle` (aliases bevel, inverse round, inset, inverted chamfer; null =
  round). The picker shows only when the selection has a radius or a style. `.fig` export warns
  (Figma draws round corners); code export (CSS, SwiftUI…) still writes plain border radius.
- Branding stops at what the user sees. Internal names stay upstream's: crates, `figcraft.exe`,
  the `.figcraft` extension, the settings folder (`FigCraft`, so settings are kept) and command ids.
  Messages that mention FigCraft in passing (errors, sharing) still do; renaming them would mean
  editing every language catalog.
- The game advances in fixed 1/60 s steps, at most 5 per frame; a late frame (hidden window,
  stall) drops the backlog instead of fast-forwarding. Same game on 30, 60 and 144 Hz displays.
- `crates/game` has no dependencies yet; `crates/ui-egui/src/game/` is where wgpu, input and
  audio will come in (it is the only place that may know about egui).

## Upstream hooks
Every place where we changed an upstream FigCraft file, so merges stay easy:
- `crates/ui-egui/src/present/layout.rs` `layout`: in Play the frame is sized and scaled by
  `game::screen_fit::fit` (anchoring to any monitor).
- `Cargo.toml` `[workspace.dependencies]`: `figcraft-game` path entry.
- `crates/ui-egui/Cargo.toml`: depends on `figcraft-game`.
- `crates/ui-egui/src/lib.rs`: `pub mod game;`.
- `xtask/src/layers.rs` `TABLE`: `("game", 2)`.
- `crates/ui-egui/src/theme.rs` `paint_wordmark`: calls `game::brand::paint_wordmark`.
- `crates/ui-egui/src/windows.rs` `title_of`: calls `game::brand::window_title`.
- `crates/ui-egui/src/dialogs.rs`: About window title and `about_body` text (`game::brand`).
- `crates/ui-egui/src/toolbar.rs` Help menu: `game::brand::ABOUT`.
- `apps/figcraft/src/main.rs` and `startup.rs`: window / error box title `game::brand::FULL_NAME`.
- `apps/figcraft/build.rs`: Windows `ProductName`, `FileDescription`, `CompanyName`.
- `crates/ui-egui/src/panels/proto_panel.rs` `prototype`: calls `game::prototype_section` after the
  prototype settings.
- `crates/ui-egui/src/app/commands.rs` `game.settings`: selects nothing and shows the Prototype tab.
- `crates/ui-egui/src/present/chrome.rs` `finish`: sounds scaled by `Play::volume`.
- `crates/engine/src/proto_run.rs` `play_interactions`: calls `cmd::game::aim_animations_at_instances`.
- `crates/ui-egui/src/panels/component_panel.rs` `instance_section`: a set or main component goes to
  `game::components::section` (compact Properties / Current variant) before upstream's matrix.
- `crates/ui-egui/src/canvas/proto_canvas.rs` `draw` / `input`: a connection dropped on another variant
  of the source's set becomes Change to (`game::proto_targets::variant_at`).
- `crates/ui-egui/src/canvas/proto_canvas.rs` `noodles`: with a selection, only the selected screens'
  connections (`game::proto_targets::shows_connection`).
- `crates/ui-egui/src/panels/proto_panel.rs` `prototype`: `game::screen_ui::section` (game behind the screen,
  game bar) after the menu section.
- `crates/ui-egui/src/present/navigate.rs` `display`: maps show their content moved and zoomed
  (`game::map::apply`).
- `crates/engine/src/cmd/arrange.rs` `scale_node`: an instance scaled (itself or inside a scaled frame)
  gets its "size" override, so a variant change keeps its size (upstream only marked layers inside
  instances).
- `crates/engine/src/canvas/tools/drag.rs` `move_down` (`board_background`, and the selection-box
  fallback) and `pointer.rs` `hover`: a selected top-level frame with layers marquees instead of being
  dragged, and shows no hover over its background (`tools/frame_work.rs`, new, `mod` in `tools.rs`;
  `frame_work_tests` in `lib.rs`); `draw.rs` `board_click`: Shift-click takes a selected frame out.
- `crates/ui-egui/src/present/actions.rs` Change to: `cmd::game::keep_instance_size` before the swap.
- `apps/figcraft/src/main.rs`: with nothing to open, start in the most recent file that still exists.
- `crates/ui-egui/src/services.rs` `Services::cursor_screen_pixels` (desktop Windows: `mouse_position`
  crate) and `canvas/input.rs` `import_dropped`: dropped files land under the mouse.
- `crates/engine/src/fonts/download.rs` `catalog_families` (+ wasm stub in `fonts.rs`); `typography/picker.rs`: the
  font store's families not installed listed under "Google Fonts: downloads when picked";
  `typography/fields.rs`: a pick runs `game::fonts::fetch`.
- `crates/engine/src/cmd/game_share.rs` `share`: `cmd::game::embed_shared_fonts` before publishing.
- `crates/ui-egui/src/app.rs` `frame`: `game::fonts::load_shared`.
- `crates/ui-egui/src/theme.rs` `install_fonts`: Inter stays a fallback behind Space Grotesk (shortcut glyphs).
- `crates/render/src/effect_cache.rs` (new, `mod` in `lib.rs`) and `draw.rs` `draw_layer`: a layer blur and a
  plain image layer reuse their pixels while unchanged (same node `Arc`, place, size, scale, colour
  profile, image bytes, GIF frame); drop shadow casters only for layers with drop shadows.
- `crates/engine/src/canvas/tools/draw.rs` `up` (+ `board_click`): a click without a drag on the
  background of a top-level frame selects it (upstream: deselects); `selection_behaviour_tests`
  row changed to match.
- `crates/engine/src/selection/selection_colors.rs` `collect`: colours group by how they read
  (8-bit channels, whole percent), so near-equal greys are one row.
- `crates/ui-egui/src/panels/selection_colors.rs` `section`: shows by `game::color_fold::shown` (one
  colour too, when it comes from more than the one selected layer); header from `game::color_fold` (folds
  past three colours to a preview and "+N").
- `crates/render/src/covered.rs` (new, `mod` in `lib.rs`) and `draw.rs` `draw_content`: a frame fill
  a layer inside covers completely (opaque, axis-aligned, whole box) is not drawn (no light seam).
  Same file: `draw.rs` `inset_outline` (the ring builder of `stroke_area`, pulled out as a function) and the
  fill call in `draw_content` use `covered::fill_under_stroke` (fill inset under an opaque inside stroke).
- `crates/ui-egui/src/canvas/render.rs` `request_render` and `canvas.rs`: the drawn document goes
  through `game::layout_ease::view`; the canvas repaints while layers glide.
- `crates/ui-egui/src/panels/typography/spacing.rs` `spacing_input`: Alt and a drag anywhere on a
  spacing field scrubs it (`game::alt_scrub`). Number fields do it upstream since v0.17 (merged
  2026-11-10: our `widgets.rs` Alt-scrub was dropped for theirs).
- `apps/figcraft/src/main.rs` (`set_startup_update_check(false)`), `crates/ui-egui/src/toolbar.rs` (Help
  menu row removed), `menus.rs` (`help.checkForUpdates` does nothing) and `dialogs/updates_ui.rs`
  (`check` kept, unused): no FigCraft update check or prompt; its menu string is kept in `game/brand.rs`.
- `crates/ui-egui/src/lib.rs` (`font_watch` field) and `app.rs` `frame`: `game::font_watch::tick`
  looks at the font folders in the background when the window comes back to the front and walks them
  again only when a font file came or went (no refit: a walk reports most families as changed).
- `crates/engine/src/boolean_box.rs` (new, `mod` in `lib.rs`) and `derived.rs` `normalize` step 3: a
  boolean group's box is its result's bounds (after `fit_to_children`).
- `crates/ui-egui/src/canvas/handles.rs` `al_zones`: no padding bands (no padding handles on the
  canvas; gap handles stay).
- `crates/engine/src/anchors.rs` (new, `pub(crate) mod` in `lib.rs`), `derived.rs` `normalize` (after step
  3) and `cmd/props.rs` `"constraints"`: layers placed or moved in a frame without auto layout anchor
  by position (9-slice thirds) until set by hand (`GameSettings::anchored`); `game.anchors.auto`.
- `crates/ui-egui/src/panels/right.rs` `position_fields`: `game::anchor_ui` (Figma-like constraints
  with a clickable box; "Anchor layers by position" for a frame); `constraints_row`/`pretty` kept unused.
- Upstream tests changed for position anchoring: `history_behaviour_tests` scene and
  `canvas_fixes_tests::a_selected_layer_shows_its_constraint_guides_in_its_frame` anchor by hand.
- `crates/engine/src/shift_select.rs` (new, `mod` in `lib.rs`) and `canvas/tools/drag.rs` press: Shift-click
  adds the layer at the selected layers' depth (same top-level frame), not the plain click's level.
- `crates/engine/src/rect_to_frame.rs` (new, `mod` in `lib.rs`), `cmd/structure.rs` `wrap` and
  `cmd/component.rs` (passes `wrapOnly`): "Frame selection" on one rectangle makes it the frame.
- `crates/ui-egui/src/menus.rs` canvas context menu: "Set as frame background" (`game.image.toBackground`,
  in `cmd/game.rs`) after "Crop image" for one image layer in a frame.
- `crates/engine/src/canvas/tools/drag.rs` `resize_drag`: Ctrl crops images inside auto layout too
  (not a layer with its own auto layout, nor one with a min/max size in an auto layout parent).
- `crates/ui-egui/src/panels/right/paints.rs` `paints`: a Fill/Stroke linked to one paint style is one row
  (`game::style_fill`: swatch, name, detach), not the style's paints.
- `crates/ui-egui/src/widgets/colorpicker.rs` gradient "Rotate 90°": `game::gradient_turn::quarter` (a
  linear gradient turns about the layer's middle; others about their centre).
- Upstream v0.17 merge (2026-11-10): `theme.rs` dark `text_tertiary` raised to 136 (upstream's 3:1
  contrast test); `shortcut_parity_tests` pins F9 (`game.play`); `canvas_fixes_tests` pencil preview
  uses a light page (Graybox pages start dark); `crates/text/build.rs` keeps Space Grotesk and
  JetBrains Mono in web builds next to upstream's `FIGCRAFT_EMBED_SCRIPTS`/`FIGCRAFT_WEB_FONTS`.
- `crates/engine/src/canvas/vectoredit.rs` `ensure_vector`: a whole ellipse edits with four points
  (`ellipse_points.rs`, new, `mod` in `lib.rs`).
- `crates/ui-egui/src/canvas/overlays.rs` selection overlay: hidden while the Pen draws a path.
- `crates/engine/src/canvas/tools/drag.rs` pen drag: 8 screen px pull out no handle (was 3); press
  filter: a Cmd/Ctrl press always starts a marquee (not the selection-box drag either), leaving out
  the layer it began on; `draw.rs` `board_click` turns a Cmd/Ctrl click into the deep select.
  `selection_cache_tests` moves with Control (snapping off) instead of Cmd.
- `crates/engine/src/cmd/structure.rs` `join`: several layers joined have their meeting ends welded
  (`weld.rs`, new, `mod` in `lib.rs`; 2 units).
- `crates/ui-egui/src/toolbar.rs`: no Game settings button beside Play (`main_width` counts 5 cells).
- `crates/ui-egui/src/panels/layout_panel.rs` `dimensions`: an upright line shows its length as H
  (`game::line_dims`).
- 9-slice (`crates/doc/src/nine_slice.rs`, `pub mod` in doc `lib.rs`; stored as plugin data
  `graybox.nineSlice`): `crates/render/src/draw.rs` `draw_layer` (now `pub(crate)`) calls
  `nine_slice::draw` (new, `mod` in render `lib.rs`); `cmd/props.rs` `resize_node` keeps a sliced
  layer's children put; `game.nineSlice.set` in `cmd/game.rs`; UI: `panels/right.rs` (section just
  above Export since G16.7), `canvas.rs` (cuts drawn), `canvas/input.rs` (cuts dragged) →
  `game::nine_slice_ui`.
- Pseudo-localization: `crates/engine/src/pseudo.rs` (new, `pub mod`); `canvas/render.rs` draws
  `game::pseudo_view::view(doc)`; `canvas.rs` banner; switch in `game::prototype_section`.
- Unity export: `crates/engine/src/unity_export.rs` (new), `game.unity.export` in `cmd/game.rs`
  (skipped by the junk sweep in `tests.rs`: it writes files), `GameSettings::{unity_dir,
  unity_frames}`; UI `game::unity_ui` (frame switch via `panels/proto_panel.rs`, settings part, export
  on save from `app.rs`). The Unity side is C# (`integrations/unity/`, a UPM package): the one place
  that isn't Rust, as Unity plugins can't be.
- `crates/ui-egui/src/bidi.rs` `single_line`: control height capped to the row, so rename fields edit
  on the same line as the name (theme interact size 24 > row 20).
- `crates/ui-egui/src/panels/proto_panel.rs` `prototype`: calls `game::menu_section` for one
  selected layer (after Scroll behavior).
- `crates/ui-egui/src/present/player.rs` `player`: calls `game::menu_overlay` right after `draw`.
- `crates/ui-egui/src/present/actions.rs` `texture`: in Play, a changed screen asks
  `game::frames` and shows its last image meanwhile.
- `crates/doc/src/game.rs` and `crates/engine/src/cmd/game.rs` are ours (game settings, game commands).
- CRT effect (`crates/doc/src/crt.rs`, `crates/render/src/crt.rs` are ours):
  - `doc/src/lib.rs`: `pub mod crt; pub use crt::{Crt, CrtDirection};`, `mod crt_tests`.
  - `doc/src/props.rs`: `EffectKind::Crt`, `Effect::TYPES` entry, `crt()`, `of_type`, `label`,
    `backdrop_radius` arm. `doc/src/colors.rs`: no-colour arm.
  - `engine/src/cmd/arrange.rs` (scale: line size), `engine/src/dev/devinspect.rs` (row),
    `engine/src/effects_tests.rs` (type list).
  - `format/src/fig/export/{paint,layer,assets}.rs`: `figma_effects` leaves CRT out.
  - `vectordrawable/src/{walk,warnings}.rs`: `Warn::Crt`.
  - `render/src/lib.rs` (`mod crt`, `band_overlap` none with a CRT, `render_alone` filter),
    `render/src/backdrop.rs` (`has_effects`, `effects` arm), `render/src/draw.rs` (reach arm,
    `crt::clear_under` before the layer's composite), `render/src/effect_cache.rs`,
    `render/src/retained.rs` (`sensitive`).
  - `ui-egui/src/panels/right.rs` (imports), `ui-egui/src/panels/right/effects.rs` (the CRT
    fields arm and `CrtField`).
- Corner styles (`crates/doc/src/corner_style.rs`, `ui-egui/src/game/corner_style_ui.rs` are ours):
  - `doc/src/props.rs` `Corners::style`; `doc/src/lib.rs` (`pub mod corner_style`, `CornerStyle`,
    `mod corner_style_tests`); `doc/src/node.rs` (component set corners use `..Default`);
    `doc/src/geometry.rs` `shape_path` (rect-like outlines through `corner_style::outline`).
  - `render/src/draw.rs` `side_strokes` and `stroke_area` (styled rings), `render/src/glass.rs`
    `RoundedRect::of` (round only), `svg/src/lib.rs` stroke shortcut (round only).
  - `format/src/fig/export/shape.rs` (stroke geometry), `layer.rs` (warning).
  - `engine/src/cmd/props.rs` (`cornerStyle` / `cornerShape`, help, override group "corners"),
    `automation/inspect.rs` (`cornerStyle` when not round), `cmd/edit/clipboard.rs` (copied and
    pasted with the radius), `tests.rs` (junk sweep rows), `lib.rs` (`mod corner_style_tests`).
  - `ui-egui/src/panels/right/paints.rs` `appearance`: `game::corner_style_ui::section` after the
    radius row; Corner smoothing only for round corners. `ui-egui/src/game/mod.rs`: the module.
- Claude chat (G10.1; `crates/ui-egui/src/game/claude/` and `apps/figcraft/src/chat_control.rs` are ours):
  - `crates/ui-egui/src/game/mod.rs`: `pub mod claude;`.
  - `crates/ui-egui/src/app.rs` `frame`: `game::claude::tick` (with the other game ticks) and
    `game::claude::show` after `updates_ui::show` (the panel).
  - `crates/ui-egui/src/app/commands.rs`: `"game.claude.toggle"` arm in the UI requests.
  - `crates/engine/src/cmd/game.rs`: `game.claude.toggle` (asks the UI; no params).
  - `crates/ui-egui/src/toolbar.rs`: the chat button after Play; `main_width` counts 6 cells and 4 gaps.
  - `crates/ui-egui/src/icons.rs` / `icons/draw.rs`: `Icon::Chat`.
  - `apps/figcraft/src/main.rs`: `mod chat_control;`, `claude::control::set_port` when started with
    `--control`, and `chat_control::open_if_asked` in `App::logic`.
- Game kind and third person (G11.1; `crates/ui-egui/src/game/{kind_ui,tps}.rs`, `renderer/tps/`,
  `crates/game/src/tps/`, `crates/engine/src/cmd/game_kind.rs` are ours):
  - `crates/doc/src/game.rs` (ours): `GameKind`, `GameSettings::kind`.
  - `crates/engine/src/cmd.rs`: `pub mod game_kind;` and `list.extend(game_kind::specs())`;
    `crates/engine/src/lib.rs`: `mod game_kind_tests;`.
  - `crates/ui-egui/src/app/commands.rs` `run`: `game::kind_ui::after_run` (a new file asks its game);
    `crates/ui-egui/src/app.rs` `frame`: `game::kind_ui::show` (the card), after the Claude panel.
  - `crates/ui-egui/src/present/game.rs` `show`: `game::tps::sync` (which game runs), `game::tps::paint`
    (draws it), and the help line for third person.
  - G11.3, same `show`: `game::tps::debug_keys` after `sync` (clip preview keys, debug builds) and the
    help line now comes from `game::tps::help(play)`.
  - G11.5, same file: `game::tps::respawn_on_request` after the first-person respawn,
    `game::tps::drain_events` for the events, `game::tps::bar_share` first in `bar_widths` (a
    first-person bar on stamina stays full), `game::tps::variables` in `publish` (the first-person values
    as fixed arrays too), `game::tps::dead` in the help line. `crates/ui-egui/src/game/screen_ui.rs`
    (ours): the Stamina bar choice.
  - G11.6, same file: `game::tps::paint` takes the crosshair flag (`!doc.game.hide_crosshair`).
  - G11.6, `crates/ui-egui/Cargo.toml`: the wasm `web-sys` features gain `Window`, `Performance`,
    `EventTarget`, `Event`, `MouseEvent` and `AddEventListenerOptions` (the mouse 4/5 listener).
  - `crates/ui-egui/src/game/mod.rs` (ours): `Play::tps`, hooks in `frame` and `inspect`, the Game picker
    first in `prototype_section`; `browser_buttons::playing()` in `frame` (G11.6).
- Documents (G13.1; `crates/doc/src/docs.rs`, `crates/engine/src/cmd/docs.rs`, `crates/ui-egui/src/game/docs/` are ours):
  - `crates/doc/src/lib.rs`: `pub mod docs;`, `Document::docs` (serde default, skipped when empty), its
    initialiser in `without_pages`, `mod docs_tests;`.
  - `crates/engine/src/cmd.rs`: `pub mod docs;` and `list.extend(docs::specs())`; `engine/src/lib.rs`:
    `mod docs_cmd_tests;`.
  - `crates/ui-egui/src/state.rs`: `LeftTab::Documents`. `icons.rs` / `icons/draw.rs`: `Icon::Document`.
  - `crates/ui-egui/src/panels/layers.rs`: the rail item, the header title (like Assets) and the match arm
    that calls `game::docs::panel`. `app.rs`: the panel's accessible label, and in the central panel
    `game::docs::show_editor` before `canvas::show`. `app/commands.rs` `run`: `game::docs::before_run`.
    `control.rs`: `leftTab: "documents"`. `game/mod.rs`: `pub(crate) mod docs;`.
  - `crates/ui-egui/locales/en.toml` and `pt-BR.toml`: a "Graybox: Documents" block of strings at the end
    of `[ui]` (the other languages show English until translated).
  - G13.3: `crates/doc/src/lib.rs`: `pub mod docs_md;` and `mod docs_md_tests;`. `crates/engine/src/lib.rs`:
    `mod docs_page; mod docs_render;` and the test mods `docs_export_tests`, `docs_page_tests`.
    `crates/engine/src/tests.rs`: `"doc.export"` in the junk sweep's skip list (it writes files). Locales:
    "Put on the canvas", "Export as PDF…", "Export as Markdown…", "Document put on the canvas".
  - G13.6: `crates/ui-egui/src/game/mod.rs`: `pub(crate) mod emoji_picker;` and `mod page_icon_tests;`.
    `crates/engine/src/tests.rs`: two page-icon junk entries. Locales: "Change the icon…", "Double-click
    to change the icon", "Game", "Work", "People", "Nature" (the unused "Icon" removed).
  - G13.4: `crates/doc/src/lib.rs`: `pub mod docs_version;` and `mod docs_version_tests;`.
    `crates/engine/src/lib.rs`: `pub mod doc_versions` (what the editor needs: `code_of`, `intact`,
    `hand_font`, `when`) and `mod docs_version_tests;`. `crates/engine/src/tests.rs`: two junk entries
    for `doc.version.*`. Locales: the version, signature, compare and decision-log strings
    ("Version {n} · {state}" replaces "Version {n} · draft" and "Version {n} · closed, it can no longer
    change").
  - G13.2: no new hook. Locales: the block editor strings ("Turn into, duplicate, colour, delete"
    replaces "Turn into, delete"; "Heading {n}", "List", "To-do", "Decision", "Who", "When", "Why",
    "Brown", "Purple"...).
  - G13.2 `/` menu: no new hook. Locales: "Type “/” for blocks", "Blocks", "No results", "Toggle list".
  - G13.2 moving blocks: no new hook. Locales: "Click for the menu, drag to move" replaces "Turn into,
    duplicate, colour, delete".
  - G13.2 tables: no new hook. Locales: "Header row", "Insert row above", "Insert row below", "Insert column left", "Insert column right", "Delete row", "Delete column".
  - G13.2 images: `crates/ui-egui/src/pending_open.rs`, `pending_opens`: the card title arm also matches `doc.block.image` (pictures dropped on a document). Locales: "or drop one here", "Drop images to add them here".
  - G13.2 embeds: no new hook. Locales: "Live", "Pinned", "This layer was deleted", "Show a frame or component of this file", "Choose a layer", "No frames or components", "Frame or component", "Follow the layer", "Choose another layer".
  - G13.2 swatches, shapes, mentions, selection: no new hook. Locales: "Colour swatch", "Colour name".
  - G13.2 fonts and pastes: `crates/ui-egui/src/shortcuts.rs`, after `game::pixel::paste_events`:
    `game::docs::canvas_input` drops the keys, clipboard events and image pastes meant for the canvas
    while a document is shown (undo, redo and the app's own keys pass). Locales unchanged.
  - G13.8: `crates/ui-egui/src/app.rs`, the Documents hook in the central panel: `show_editor`'s answer
    also hides the canvas toolbar while a document is shown. Locales: "Start with".
  - G13.2 formatting words: no new hook. Locales: "Bold", "Highlight", "Paste a link".
- Nested pages (G9; `crates/doc/src/page_tree.rs`, `crates/engine/src/cmd/page_tree.rs`,
  `crates/ui-egui/src/game/tree_list.rs` and `page_list.rs` are ours):
  - `crates/doc/src/lib.rs`: `pub mod page_tree;`, `Document::page_tree` (serde default, skipped when
    empty), its initialiser in `without_pages`, `validate` (reports a nesting that is not in order, so
    loading repairs it), `repair` (calls `settle_pages`), `mod page_tree_tests;`.
  - `crates/engine/src/cmd.rs`: `pub mod page_tree;` and `list.extend(page_tree::specs())`;
    `engine/src/lib.rs`: `mod page_tree_tests;`; `engine/src/tests.rs`: three junk entries in the sweep.
  - `crates/engine/src/cmd/page.rs`: `page.add` (`parent`), `page.select` (`reveal`), `page.delete`
    (`subpages`, `before_delete`), `page.duplicate` (`place_copy`), `page.move` (`settle_pages`);
    `cmd/view.rs` `page_step`: `reveal`.
  - `crates/ui-egui/src/panels/layers.rs`: `pages` calls `game::page_list::show` instead of drawing its own
    list (the old loop is gone); `delete_page` asks via `game::page_list::ask_before_delete`;
    `page_rename_start_id`, `layer_drag_id` and `dragged_layers` became `pub(crate)`; the `trf` import went.
  - `crates/ui-egui/src/game/mod.rs`: `pub(crate) mod page_list;` and `tree_list;`.
    `game/docs/tree.rs` now draws through `tree_list` (same look and behaviour).
  - `crates/ui-egui/locales/en.toml` and `pt-BR.toml`: a "Graybox: nested pages" block of strings.
- G16 pixel art (the code is ours: `crates/pixel`, `crates/engine/src/cmd/pixel.rs`,
  `engine/src/pixel_tests.rs`, `crates/render/src/pixel.rs`, `crates/ui-egui/src/game/pixel/`):
  - `Cargo.toml`: `figcraft-pixel` path entry; `xtask/src/layers.rs`: `("pixel", 2)`; engine,
    render and ui-egui `Cargo.toml`: depend on it.
  - `crates/engine/src/cmd.rs`: `pub mod pixel;` and `list.extend(pixel::specs())`;
    `engine/src/lib.rs`: `mod pixel_tests;`; `engine/src/tests.rs`: two junk rows in the sweep.
  - `crates/render/src/lib.rs`: `pub mod pixel;`; `render/src/draw.rs` `draw_content`: fills go through
    `pixel::fill`, which draws other paints through the new `fill_paints` helper.
  - `crates/ui-egui/src/lib.rs`: field `FigApp::pixel`; `app.rs`: its default; `game/mod.rs`:
    `pub(crate) mod pixel;`.
  - `crates/ui-egui/src/canvas.rs`: in Pixel mode `game::pixel::draw` instead of `handles::draw`;
    `canvas/input.rs`: `game::pixel::input` after panning, `game::pixel::off_canvas` when the pointer
    is off the canvas; `canvas/overlays.rs`: no selection box in Pixel mode.
  - `crates/ui-egui/src/shortcuts.rs`: key events go through `game::pixel::key` first.
  - `crates/ui-egui/src/app/commands.rs`: the `pixel.mode` UI request.
  - `crates/ui-egui/src/toolbar.rs`: `show` gives the bar to `game::pixel::toolbar` in Pixel mode;
    the mode switcher has a fifth mode "Pixel" (widths and Play position follow); `pill`, `divider`,
    `tool_cell`, `done_button`, `pretty_shortcut` and the size constants became `pub(crate)`.
  - `crates/ui-egui/src/panels/right.rs`: `game::pixel::section` after Effects (above Layout
    until G16.6).
  - `crates/ui-egui/src/icons.rs` and `icons/draw.rs`: `PixelArt`, `PixelPerfect`, `Tiles` (drawn in code).
  - G16.2: `panels/right.rs`: `game::pixel::colors::section` after Selection colours;
    `shortcuts.rs`: `game::pixel::paste_events` after the key filter (an image paste while a sprite
    is edited pastes its copied pixels); `icons.rs` / `icons/draw.rs`: `Marquee`, `MagicWand`,
    `Outline`, `ReplaceColor`, and the pixel icons moved into `draw_pixel_art` (one arm in `draw`).
  - G16.3: `render/src/draw.rs` `draw_layer`: the 9-slice hook now calls `pixel::layer` (Pixel
    text, then `nine_slice::draw`); `render/src/nine_slice.rs` (ours) leaves pixel layers to
    `pixel`; `engine/src/cmd/game.rs` (ours): `game.nineSlice.set` and `game.image.toBackground`
    call `cmd::pixel::kit` for pixel layers.
  - G16.4: `render/src/images.rs` `animation_clock`: one line, `pixel::tick` adds pixel layers'
    frames to the clock (Prototype and Play redraw when a frame is due).
  - G16.5: `engine/src/tests.rs`: `pixel.export` in the junk-sweep skip list (it writes files);
    `crates/pixel/Cargo.toml` (ours) uses the workspace `image` for GIFs.
  - G16.6: `icons.rs` / `icons/draw.rs`: `Shade` (in `draw_pixel_art`).
    Follow-ups: `widgets/colorpicker.rs`: `PickerTarget::Pixel {which}` (`current_paint`, `write`,
    no paint-type or blend rows, `game::pixel::colors::picker_rows` before "On this page");
    `icons.rs` / `icons/draw.rs`: `Timeline`.
  - G16.7: `panels/layout_panel.rs` `dimensions`: `game::pixel::board::dims_row` (a board's W and H
    in art pixels, in Pixel mode); `panels/right.rs`: the 9-slice section moved above Export;
    `canvas/input.rs`: the Pixel mode hook returns `!game::pixel::has_cursor` (its own pointer
    over a mirror handle or a board side stays).
  - G16.8 (colours and the background): `icons.rs` / `icons/draw.rs`: `Checkerboard` (in
    `draw_pixel_art`); `panels/right.rs`: `game::pixel::colors::fill_section` takes the Fill
    section's place while a sprite is edited, and `colors::section` moved from under Selection
    colours to between Fill and Stroke.
  - G16.8: no other new hook, and one **removed**: pixel fonts are gone, so `crates/text/src/fontdb.rs`
    `replace_font_bytes` / `FontDb::replace_bytes` and their re-export in `crates/text/src/lib.rs`
    were deleted (upstream text is untouched again). The sprite's title reuses the engine's own
    `tools::labels` list from our module, so the canvas needed no change.
- G17 (3D rotation of layers): workspace `Cargo.toml` (`figcraft-tilt`), `xtask/src/layers.rs`
  (`tilt` at L0), `crates/doc/Cargo.toml` (depends on `figcraft-tilt`), `crates/doc/src/lib.rs`
  (`pub mod tilt`); `crates/doc/src/geometry.rs` `hit_rec` and `layers_at` (the point goes back
  through a tilted layer's warp, `tilt::flat_point`); `crates/render/src/lib.rs` (`mod tilt`;
  `export_bounds`: a tilted layer exports its `paint_bounds`); `crates/render/src/draw.rs`
  `draw_layer` (`tilt::draw` before `pixel::layer`), `bounds_within` (`tilt::grow`),
  `paint_bounds` (`tilt::inside_tilted`); `crates/engine/src/cmd.rs` (`mod tilt`, its specs);
  `crates/engine/src/canvas/tools/keys.rs` `Session::hit` (runs the hit test as `hit_shown` inside
  `tilt::with_flat`, edit flat); `crates/engine/src/tests.rs` (two junk entries);
  `crates/ui-egui/src/canvas/render.rs` `request_render` (`tilt_ui::view`); `canvas.rs`
  (`tilt_ui::draw`); `panels/right.rs` (`tilt_ui::section` under Position);
  `present/transition.rs` `smart_interpolate` (`tilt_ui::lerp`); `crates/engine/src/cmd/edit/select.rs`
  `layers_at` and `crates/ui-egui/src/menus/select_layer.rs` (inside `tilt::with_flat`, edit flat);
  `xtask/src/layers.rs` `same_layer_ok` (`figcraft-tilt` → `figcraft-geom` within L0).
  G17.2: `crates/render/src/lib.rs` `raster_bounds` (a tilted layer: `paint_bounds`);
  `crates/svg/src/lib.rs` `raster_whole` and `crates/pdf/src/draw.rs` (a tilted layer is embedded as
  its picture); `crates/engine/src/canvas/tools/pointer.rs` `Session::pointer` (`cmd::tilt::pointer`);
  `tools/handles.rs` `handle_at` (`cmd::tilt::handle_point`); `tools/keys.rs` `arc_handles_shown`
  (`!cmd::tilt::selected_tilted`); `crates/ui-egui/src/canvas/outline.rs` `node_quad` and
  `outline_shape`, `canvas/frame_labels.rs` `draw` (`tilt_ui::warp`/`drawn`); `canvas/overlays.rs`
  `selection_geometry` (`tilt_ui::drawn_box`); `canvas/input.rs` (`tilt_ui::input`, the round handle).

- G9 owner's batch (2026-11-13): `crates/engine/src/canvas/tools.rs` (`Gesture::PenDrag.press`,
  `pen_wobble`), `tools/pen.rs` `pen_down` (Shift: no out handle, 45°), `tools/drag.rs` (pen drag:
  Shift or wobble makes no curve), `vectoredit/pointer.rs` (the same in vector edit; `pen_tests`);
  `crates/pathops/src/brush.rs` (four presets; stretch shapes from `organic::stretch_unit` per
  subpath after `seam_on_a_side`; scatter size × `organic::envelope`), `pathops/src/lib.rs`
  (`pub mod organic`); `crates/doc/src/props.rs` `Brush` (`taper`, `taper_start`, `taper_end`,
  sanitized); `crates/render/src/draw.rs` `cached_stroke_outline` (hashes the taper settings);
  `crates/engine/src/cmd/props.rs` `BRUSH_HELP` and `cmd/edit.rs` `stroke.brush` doc (new presets,
  taper keys); `crates/ui-egui/src/panels/right/stroke.rs` `brush_row` (`game::brush_taper::rows`);
  locales (Brush pen, Dry brush, Chalk); `crates/engine/src/lib.rs` (`mod design_eraser`, tests),
  `cmd/view.rs` `tool.set` and `boards/board.rs` `tool.eraser` (the Eraser needs no board),
  `boards/board/tools.rs` `board_down` (Eraser in design files) and `erase_at`
  (`design_eraser::erasable`, no document clone); `crates/ui-egui/src/toolbar.rs` (`GROUPS` pen group
  with the Eraser, `MODES` = Draw · Pixel · Design, `mode_switcher`, `pick_mode`, widths),
  `pen_mode_tests.rs` (Draw instead of Prototype/Inspect, no-Prototype test),
  `panels/right.rs` `right_panel` (`game::inspect_button::show` left of Play);
  `crates/ui-egui/src/canvas/handles.rs` `al_zones` (`gaps.clear()`), `gap_handle_tests.rs` (both
  ignored). Upstream fixes taken on their own, no merge: #1741 (0aba3f76, 61389cdd) and #2222
  (c48d844d, 4c91f908, 529134b8, bae366ba, ec105d0f; plus `..Default::default()` for
  `Corners::style` in `format/src/fig/export_tests.rs`).
- The G1–G6 commit 32b0bc22 changed more upstream files (present/*, toolbar, theme, services,
  prefs, proto model…) without listing them here; to list.

## Known issues (not ours)
- With craft-fonts in the build, `components_scale_tests::editing_deep_mains_keeps_every_override_and_matches_a_full_rebuild`
  fails when the whole engine suite runs in parallel and passes alone (also with the scale fix
  reverted); `board_tables_tests::a_double_click…` passes now.
- This machine now builds with `../craft-fonts` (plus the Graybox faces, see FONTS.md); the same three
  engine tests still fail (board tables row growth, .fig font names, example .fig export).
- Fail on this Windows machine before the G4 rework too: `fig_export_assets_tests::every_example_keeps_its_variables_and_styles`
  (.fig export drops the starter kit's number→text binding) and `fig_font_names_tests`
  (depends on installed fonts).
- `figcraft-engine` test `board_tables_tests::a_double_click_types_in_the_cell_and_its_row_grows_with_the_text`
  fails on this machine at upstream commit `db3cb317` too, before any game change (checked on a
  clean worktree). Probably text measurement without `../craft-fonts`. Everything else in
  `cargo xtask ci` passes. `collab_live_tests::edits_sync_and_undo_takes_back_only_your_own`
  failed once and passed on the rerun (flaky).
- Dev machine (Arch): the web target comes from `pacman -S rust-wasm` (no rustup).

## Next
- G11 (third-person game; spec [`specs/2026-11-11-third-person-design.md`](specs/2026-11-11-third-person-design.md)).
  Milestone G11 is done (`PLAN.md`); the installed copy comes with the batch install. Left from
  G11.6: one real mouse 4/5 click in the browser, the string hand's dip mid-draw.
- Navigable menus phase 2: tab bars (LB/RB) switching the menu below; spatial focus for layers
  without auto layout. To check: Tab sent through the control channel did not open the starter
  Inventory in Play (the controller's View did).
- G0.3: checkpoint and recovery copy every 2 minutes.
- G2.3: try controllers on Linux/macOS (gilrs code is in).
- G7: share link opening in Play (code is in, untested).
