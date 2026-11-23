# DevTrack — design system (round 4: precision design system)

Rust + eframe/egui. This round supersedes earlier design notes where they conflict. Functional
rules and calculations are unchanged.

## Tokens (`src/ui/widgets.rs`, `pal()`)

| Role | Dark | Light |
|---|---|---|
| Canvas | `#131312` | `#FFFFFF` |
| Navigation / secondary surface | `#181817` | `#FAFAFA` |
| Raised surface | `#1C1C1B` | `#FFFFFF` |
| Field | `#161615` | `#FFFFFF` |
| Disabled fill | `#161616` | `#F5F5F5` |
| Divider / control border | `#2E2E2B` / `#464642` | `#E5E5E5` |
| Main / secondary text | `#EEEDE9` / `#A6A59E` | `#171717` / `#666666` |
| Primary action | `#EEEDE9` fill, canvas text | `#171717` fill, white text |
| Focus ring | `#52A8FF` | `#005FCC` |
| Active / missing / error | `#62C073` / `#F5A524` / `#FF6166` | `#1A7F37` / `#A35200` / `#C62A2F` |

Colour is never the only signal: states carry words ("✔ Reported", "! Needs code", "Needs a project").

## Control family (A)

One geometry for inputs, selector triggers and buttons: **36** outer height (border inside),
radius 6, padding 12, Sans 14 or Mono 14; icon buttons 36 × 36; menu rows ≥ 36; segmented
switches 36. Labels Sans 12 above the control (gap 6); helper or error Sans 12 below (gap 6); 8
between rows, 16 between groups. Focus: 2-unit ring outside the control (no layout change).
Invalid: red border plus a specific message. Disabled: grey fill and grey text. Read-only: nav fill,
selectable text. Placeholders are hints, never labels. Validation happens on submit (and on blur
where a field is left), never on a freshly added row; after a failed submit the first invalid field
gets focus. Drafts survive validation errors. Implemented as `input`, `readonly`, `select_trigger`,
`select`, `menu_row`, `segmented`, `button_w`/`primary`/`secondary`/`ghost`/`danger`, `icon_button`,
`field`, `project_picker`, `task_picker`. The component gallery (Settings › Display › Open component
gallery) shows each state in either theme and measures the real start row and time row.

## Type (B)

Geist Sans (Regular, Medium, SemiBold) and Geist Mono (Regular, Medium), bundled under the SIL OFL. Every text renders at 95% of the size written in the code (`TEXT_SCALE` in `widgets.rs`, applied to the glyphs when the fonts are installed); the logo keeps its size, controls and spacing do not change.
Every number is Mono: durations, timers, clock times, dates, counts, percentages, differences, IDs
(#4821), codes and destination values. Mixed text ("3 sessions need details") is drawn with Mono
digit runs (`mixed`, `mixed_job`).

| Use | Font |
|---|---|
| Page heading | Sans SemiBold 28 |
| Section heading | Sans Medium 17 |
| Summary value | Mono Medium 24 |
| Live timer | Mono Medium 34 |
| Ledger interval / duration | Mono 13 / Mono 14 |
| Destination value | Mono Medium 20 |
| Supporting numbers | Mono 12 |

Numeric comparison columns (durations, Actual, destination values) are right-aligned.

## Shell (C)

Sidebar 216; top bar 56; content inset 32 × 24. Today, Activities, Report Time, Statistics at the
top; Settings alone at the bottom (16 inset) with a small amber mark when something needs attention.
No sidebar footer: connections, saving and devices live in Settings. The top bar's right edge holds
the 36 × 36 **Compact window** button (tooltip, accessible name, ⌘/Ctrl+Shift+M); on pages other
than Today the running session (title, Mono timer, Pause, Stop) sits to its left. No global search.

## Today (D)

Session panel (156 active, ~120 idle, padding 20) → summary band (88) → ledger with a 36 header and
64-unit rows. Toolbar: title and count, ‹ date › with Day/Week next to it, List/Timeline at the right.
A 320 context column with a 24 gap appears only while the ledger keeps ≥ 600; otherwise the same
content is an expandable section under the ledger. One attention summary ("2 sessions need details",
Review, Show only these); the filter shows a Mono counter line with "Show all". Review opens the
editor with Save only and returns to the same context. Idle metrics: tracked, needs
details, readiness; active: tracked, completed time, active session, needs details.

## Destination setup (E)

New: Format (name, three format cards with a live example, rounding inside a closed disclosure,
optional System) → Project codes (projects used in the period, full-width code and activity, "Add
task exception", "Add another project"), then Create destination or "Set up codes later". Existing:
the same sections plus Destination settings (lines, fallback code, date format, copy line, a
plain summary and a sample line). One dialog ≤ 680 wide and ≤ viewport − 48 high, header and footer
fixed, body scrolls. "Needs code" opens a one-project code editor (or the fallback code for
sessions without a project) over the same row and date. Unsaved changes ask "Discard your changes?"
on Cancel, Escape or a click outside.

## Workflows (F)

Activities → Connect Azure DevOps goes to Settings › Connections and, once a test succeeds, offers
"Return to assigned tasks" for the same project. The finder has one toolbar (connection, Assigned /
Search, DevOps project, Refresh, "···" with "Refresh imported tasks"); the last connection is
remembered and DevOps projects arrive with the first query. Move task: project, "Also move N
existing sessions", Move task. Sync: choose a folder, review it, Save; status separates "Saving on
this device" from "Delivery to other devices"; technical details collapsed. No dialog opens another.

## Compact window (G)

Header (DevTrack, Expand), current session (Mono 34 timer, Pause/Stop or Start/Resume), today total,
needs-details banner with Review, Recent sessions with "+ Add time", history by day for seven days
and "Load earlier", Settings in the footer (opens the full window on Settings). Under 620 units of
height the session area scrolls with the history. Not always on top.

## Motion (H)

Content-only 200 ms fade, ease-out cubic, selection updates at once, only the destination page is
drawn, rapid navigation continues from the current opacity, Settings sections fade too, and Reduce
motion turns it off.

## Review pictures (`shots/`, logical sizes; rendered at 2 pixels per point)

Regenerate with `devtrack --screenshot <png> --state first|active|incomplete|ready --page <name>
--size WxH [--light] [--compact] [--reduce-motion]`. Pages: today, activities, assigned, report,
stats, settings, connections, sync, gallery, edit, edit-invalid, past, move, dest-new, dest-codes,
dest-error, dest-dirty, dest-fix, dest-fix-project, dest-settings. Evidence files:
`--transition-log <csv>` and `--geometry-report <json>`.

## Round 5 additions

- **Activities:** Projects and Clients tabs (segmented, counts in Mono). Projects: one toolbar
  (search, `Client:` filter with All clients / No client / each client, `Status:` Active / Archived /
  All, + New project), a count with Clear filters, rows 52 high (colour, name, client, tracked Mono
  right-aligned, "···"). A project opens its task workspace with a read-only header (name, client) and
  one **Edit project** button; "‹ All projects" returns with filters kept. Clients: search, + New
  client, rows with project counts, a client page with its projects and + Add project (prefilled).
  Create / rename / delete-confirm panels are inline, max 480 wide.
- **Edit project dialog:** ≤ 480 content; Project name, a searchable Client selector (No client,
  + Create client), a compact colour trigger opening a 7 × 2 palette popup (named, focusable
  swatches), Billable by default, Save / Cancel with discard confirmation. Create client replaces the
  form body (‹ Back) and returns with the new client selected.
- **Period toolbar (`ui/period.rs`):** shared by Today, Report Time and Statistics; fixed slots
  and an equal-width segmented variant (`segmented_equal`). The label slot shows
  the ISO date or range in Mono and the weekday / week / month in Sans; full text in the tooltip and
  accessible name. On Today the session panel always stays above it, so browsing never moves it.
- **Session editor:** Start date · Start time · End date · End time · Duration (five 36-unit Mono
  fields), a live "Result" line with full dates, "Ends next day" for overnight results, a second
  confirmation for results over 24 h, and Save only.
- **Report Time:** actual time to the second (Mono, h:mm:ss); each value's difference split into
  "company rounding" and "display precision".
- **Language:** Settings › Display › Language / Lingua (System, English, Italiano). `--lang it` for
  pictures; `--period day|week|month --date YYYY-MM-DD` set the same period on every page.


## Frontend consolidation — 17 November 2026

Retain the Geist Sans/Mono identity, 36-unit controls and semantic status colors. Control borders
use a separate token (#666666 dark, #8C8C8C light); decorative dividers retain their original token.
Content geometry follows available width: Activities filters reflow before overlapping the primary
action, the running session stacks below 640 content units, and narrow period bars put Today on
its own row. Small or short full windows scroll the page. The session editor scrolls its body with
Save/Cancel outside it; long inline errors wrap within their column. Search fields receive initial
focus once per popup opening, rather than on every empty-query frame.

The time readout is the visual anchor, always one line. No additional ornamental motion or cards.


## Record navigation and bounded ledger

Project and client lists are root routes. Record details replace the root heading with the record
name and type, a breadcrumb and an explicit destination on Back. A project opened from a client
returns to that client before the client list; searches and filters survive navigation. Alt+Left
uses the same route logic outside text entry and modals. Narrow windows stack Back above breadcrumbs.
Ledger metadata is one bounded summary, with grouped observations and access to full checks in
the editor. Time, duration and menu occupy independent columns; the row click excludes the menu.


## Dashboard and reporting simplification — 18 November 2026

This section supersedes the Today context-column pattern above. Today keeps the timer, period
toolbar, summary and full-width ledger. There is no attention/readiness sidebar or duplicate
collapsible section below the ledger. Individual rows retain their checks and Review access;
reporting status remains in Report Time.

Report Time uses one width calculation for header, row selection and separators. Activity and
actual-time columns share the remaining width with the destinations; additional destinations
scroll horizontally when their minimum useful widths cannot fit. Destination values and codes
keep a stable numerical column, with copy/status actions grouped vertically beside them.
Source sessions use a title, full date interval, duration and a right-hand Edit session action;
small widths stack the action. Explanations identify project and destination and keep factual
rounding/precision differences, without repeated introductory paragraphs or arrow chains.


## Statistics alignment and reporting copy — 18 November 2026

Projects and Tasks use the same row height, 3-unit footer slot and following gap. The project
share bar and task divider have a common centre line; one cannot accumulate a different row
stride. Under 760 content units the lists stack. The Statistics project filter is searchable
by project or client, includes All projects, preserves the chosen value while typing and uses
a bounded scrollable result list. It filters existing projects; it never offers creation.

Report Time names the two actions Copy value and Confirm entry; confirmation records the user's
manual entry in the destination. Copy full line is distinct from copying only a duration.
No company rounding and format differences are explicitly distinguished. Details show the
formatted value, its exact pre-format duration and the signed difference without changing
original records or calculation rules.


## Inline report inspection — 18 November 2026

This supersedes the separate report-explanation panel. A named Details action expands source
sessions immediately below the selected row. Sources open at first access, with exact date
intervals and labeled durations; a split session shows both the day portion and its total.
Calculation and full-line copy live in an initially closed disclosure, with a numeric definition
list and a visible preview next to Copy full line. Source controls stay in the viewport when
destination columns scroll horizontally. Copy/confirm controls wrap within their own cell;
only the named Details control (or the project title) toggles inspection, never the whole row.


## UX polish from the ui-ux-pro-max report — 18 November 2026

This section supersedes
the Today summary band and the period label width above.

- **Today:** no summary band. One line above the ledger: "N sessions · total tracked" (Mono
  digits) and, only when needed, an amber "! N sessions need details" with "Show only these".
  The status slot of the period toolbar holds only List/Timeline (176), so the toolbar is one row
  at 1024 and above.
- **Period toolbar:** label slot 256 (full group 760). The ISO date or range always fits; the
  Sans weekday/week/month is drawn when it fits and stays in the tooltip and accessible name.
- **Shell:** root pages have no breadcrumb in the top bar; the heading and the selected sidebar
  item name the page. "Start a session" is a secondary button.
- **Row actions:** ledger and compact rows carry a frameless 36 × 36 **Continue now** (play)
  button next to "···"; the row click excludes both. A single observation is shown by name
  ("Overlaps 1 other session") instead of "1 observation".
- **Inline actions:** `link_btn` is Medium 13 in the main text colour with a quiet underline at
  rest, a hover fill and the focus ring. Report status actions ("Confirm entry", "✔ Entered",
  "! Needs code") are framed in the border token and take their status colour on hover.
- **Report Time:** destination minimum 320, column gap 16, actual-time column 96; Copy value is a
  36 × 36 icon button with name and tooltip. Two destinations fit side by side at 1280 without
  horizontal scrolling; the format line truncates with the full text in the tooltip.
- **Durations:** sessions, timers and destination values use the chosen duration format; totals
  and aggregates (Today line, project and task totals, Statistics) read "1 h 05 min"; a project
  with nothing tracked shows "—". The Settings help states this rule.
- **Checkboxes:** unchecked boxes use the control border token so they stay visible.
- **Statistics:** the project filter needs no separate label; it sits in the toolbar status slot.


## Widths and the project page — 18 November 2026

- Charts and tables use the full content width. Only running prose keeps a measure (empty-state
  panels ≤ 520). The Statistics day chart and the Report Time inline detail no longer cap at
  900 / 820; the detail has a single Close action (on the row).
- **Project page:** one navigation row (36 × 36 Back arrow named for its target, then the
  breadcrumb). Header: name, "Client: …" (or "No client"), primary **Start** (a session on the
  project) and Edit project; under it one facts line ("31 h 14 min tracked · 12 sessions" or
  "Nothing tracked yet"). Tabs: "Tasks N" (no count at 0) and "Add from Azure DevOps"
  ("Azure DevOps" when narrow). The composer field takes the free width with Add task beside it
  and Show done at the right edge. Tasks have a 36 header (Task · Tracked). The empty state is a
  panel with "No open tasks", one sentence and Add from Azure DevOps.


## Ledger and report rows — 18 November 2026

- **Today ledger:** no Review link. The row itself opens the editor (with its checks), as do
  "Edit…" in "···"; rows show the pointing-hand cursor on hover.
- **Report Time rows:** a 36 × 36 disclosure chevron (› closed, ⌄ open; named Details / Close
  details) leads each row, before the project dot; the project name also toggles. Destination
  columns are fixed at their content width (320: value, Copy, status) and sit at the right edge;
  the project/activity column takes the remaining width. The header label and the activity line
  align with the project name. Rows highlight on hover.


## Session editor and report values — 18 November 2026

Supersedes the single
"Start date · Start time · End date · End time · Duration" row and the "Ends on another day" link.

- **Session editor:** Description → Project/Task → two parallel rows in three equal columns:
  **Start date · Start time · Duration**, then **End date · End time · consequence** ("Ends next
  day" when the result crosses one midnight). The end date is always shown. Running sessions show
  End date "—", End time "running" and a read-only duration. Under 460 the fields pair up and
  Duration takes its own row. Checks on the saved session ("! Overlaps 1 other session") follow
  the time rows in an amber frame; the explanation is the frame's tooltip.
- **Report rows:** Recorded and the destination value share the first line (centred on the
  36-unit control line); "Confirm entry" uses the main text colour, so it reads as the row's next
  action; status colours stay for "✔ Entered" and "! Needs code".


## Calendar, Recent and toolbar widths — 18 November 2026

- **Date fields (`date_input`):** Mono `YYYY-MM-DD` text with a calendar button at the right
  edge; the button (or a click in an empty field) opens a month view below the field, or above
  it when there is no room: ‹ month year ›, weekday initials in the user's week order, 36 × 32
  day cells, today outlined, the chosen day filled with the primary colour, and Today. Choosing a
  day writes the text and closes; Escape or a click outside closes. Used by every date field.
- **Recent (`resume_chip`):** 32-high buttons with a drawn play icon, the session title
  (Medium 13) and the project's dot and name (Sans 12); one line, truncated, full text in the
  tooltip and the accessible name ("Start again: …").
- **Toolbars:** a search field takes the width its row leaves free (Projects, Clients, Azure
  DevOps search); filters and actions keep their size.


## Compact window and notices — 18 November 2026

- **Compact history rows:** line 1 holds the title, the Mono duration, ▶ and "···"; line 2 uses
  the whole row width for the time range and the project (or "Missing description"), truncated
  so a row never grows wider than the window. Untitled sessions read "Untitled session" in the
  interface language. Today's total appears once, in the "Today" header of the history.
- **Notices (`notice`):** status-coloured boxes (needs details, checks on a session) use a light
  tint fill (≈ 9 % alpha) and a coloured border; the text keeps the status colour and starts
  with "!" or a word, so it keeps its contrast and never relies on colour alone.


## Report calculation in words — 18 November 2026

"Calculation and full report line" opens, per destination, with one line: "0:20:06 recorded →
**0:20** to enter". Below it only the differences that exist, in words: "Company rounding up to
15 min: +9 min 54 s", "Hours:minutes cannot show seconds (−6 s)" (minutes and decimal-hours
variants), or "No difference: the value is the recorded time." The full line is a Mono preview
in a field-coloured box; when the project has no code for that destination an amber notice says
the line would be copied without it, with Add code (the one-project code editor). Copy full line
carries "Copying does not confirm entry" as its tooltip; the format description is not repeated.


## Alignment and type scale — 18 November 2026

- **Type scale:** 12 · 13 · 14 · 16 · 17 · 20 · 22 · 24 · 28 · 34. Nothing below 12 and no half
  sizes (13.5 became 14; 11, 11.5 and 12.5 became 12).
- **Hints:** placeholders are guidance, not data: always Sans 14, even in Mono fields, and short
  ("None"); the rule they stand for goes in the helper text under the field.
- **Mixed rows:** text and inline links on one line are centred on a bounded 28-unit row.
- **Running session:** 4 units between "Tracking now" and the 36-high title field.
- **Long sessions (Settings › Data):** interval, title and note on the left; Review (secondary,
  96) on the right, centred; rows separated by dividers.
- **Compact history:** 14 units on the right are kept free for the scroll bar.

## Base components and P1 fixes (18 Nov 2026, critique round)

- **Rows.** `row(ui, h, …)` and `row_wrapped(ui, h, …)` give a line a fixed height before any
  item is added, so text, links and 36-high controls share one centre line whatever the order.
  `row_wrapped` also keeps labels whole (egui's flowing labels sit on the bottom of the line).
  `link_btn` is 20 high like a text line; its hover fill reaches 2 px past it.
- **Checkbox.** `checkbox()` draws a 16 px square box (radius 3). Never `ui.checkbox` directly.
- **Disclosure.** `disclosure()` is the only expand/collapse: the drawn chevron of the report rows,
  Medium 14 title, 32-high header. No `CollapsingHeader`.
- **Type scale.** Sizes are only 12·13·14·16·17·20·22·24·28·34. Dialog titles SemiBold 20,
  panel sub-heads and empty-state titles Medium 16, brand SemiBold 17. Page titles wrap.
- **Numbers in words** ("1 h 37 min", "2 of 2") use mixed text: digits Mono, words Sans.
- **Language.** Readable dates use `short_date()` (18/11/2026 in Italian, ISO in English); typed
  date fields stay YYYY-MM-DD with the calendar. Weekdays come from `i18n::weekday_short`. One term
  for measured time: "registrato". The second section is "Work" ("Lavoro") everywhere (⌘2), with three tabs: Projects, Tasks (every task, newest work first; a row opens it in its project and Back returns to Tasks), Clients. Filters: Projects by words, client (incl. "No client") and status; Tasks by words, open/done, client (incl. "No client", which also covers tasks without a project), project (incl. "No project"; only the chosen client's) and source (DevTrack or Azure DevOps); Clients by words and status. Each list says "N of M" when filtered and offers one "Clear filters";
  Statistics is ⌘4. The session editor is always "Modifica sessione".
- **Today.** The count and the total cover the same sessions, the running one included; the compact
  day header adds the running time too. The timeline runs from the first to the last session
  (at least four hours; 8–18 when empty).
- **Reporting row.** Each destination cell: "recorded → **value**" on the first line (value Mono
  Medium 20), code and the reason for any difference in a few words under it ("arrotondato +4 min",
  "senza secondi", "uguale al registrato"); Copy and the status action on the right, or under the
  calculation when the cell is narrow. The row's recorded total sits once, next to the activity name.
  Destinations are 400 wide side by side while the activity keeps 300; otherwise they stack under
  the activity (never a sideways scroll).
- **Provisional values** (a session still running) can't be copied: Copy and "Copy full line" are
  disabled and say why. Disabled `icon_button`/`button_w` follow `ui.is_enabled()`.

## Session panel, navigation and pickers (18 Nov 2026, test round)

- **Session panel (Today).** It has one layout for both a stopped and a running session, so pressing Start moves nothing.
  - **Line 1:** a 44-high description field (Medium 17), then the time (Mono 28) and its action on the right. When stopped, the time reads 0:00:00 in text2 next to **Start**. When running, the green dot and timer sit next to **Pause**, **Stop** and **⋯**.
  - **Fixed width:** the space for the running controls is always reserved, so the description keeps the same width in both states.
  - **Line 2:** project and task, each up to 280 wide. While a session runs, «Started at…» follows them.
  - **Below 820:** the description takes the full width. The time sits on the left with the buttons on the right, and the pickers come below (side by side from 480).
  - **Recent chips removed:** the same sessions are already in the list below, with ▶.
- **Description suggestions.** While you type, a list opens under the field with two groups:
  - «Previous sessions»: up to 8, one per distinct description + project + task combination, matched on description, task, project or client.
  - «Projects»: up to 4.
  - Choosing a session fills description, project and task. Choosing a project fills the project only. ↑/↓ highlight a row, Enter chooses it, and Esc closes the list.
  - When stopped and nothing is highlighted, Enter still starts the session.
- **Compact window.** Its description field uses the same suggestions (`today::suggest_under`). Choosing a session fills its task too, even though the compact window has no task picker.
- **Sidebar and ⌘1–4.** A section always opens at its top level, with no project or client detail, inline report detail, filter or open dialog left over.
  - Periods return to today; the Today view mode (List or Timeline) and hidden destinations are kept.
  - If there is unsaved input, «Leave without saving?» asks first, with «Keep editing» and «Leave without saving».
  - Links inside pages that go to a precise place (Review in Today, fix a project) keep using `go()`.
- **Pickers.**
  - The search placeholders say that typing can also create something: «Cerca progetto o cliente, o creane uno» and «Cerca per titolo o #numero, o creane una».
  - The «Recent» section appears only when the list has more than 6 items, so short lists no longer show the same row twice.
- **Report rows.** There is no vertical gap between a row and its divider, so the hover fill reaches the lines.
  - A line grouped into one without a code reads «Included in the line above», no longer «… the same line …» with the word left untranslated.

## Second full pass (18 Nov 2026)

- **Links** (`link_btn`) are exactly as wide as their text, so a link starts on the same edge as
  the text around it. The hover fill and the focus ring reach 6 px past it on each side.
- **Period bar**: one row from 604 px. Between 604 and 760 the date label (160–256) and the
  segments (76–96) shrink together; below 604 it wraps as before. The date label uses the field
  fill, never the navigation fill, so it doesn't read as disabled.
- **Session panel**: one line while the description keeps 260 px (a 1024 window). Project and
  task end where the description ends when it is at least 568 wide, the task taking what the
  project leaves; otherwise they use the row, less the room for the start-time link.
- **Timeline**: a session block is at least 20 high (title readable, easier target). A day with
  no time shows no total. The day number follows the day name at 6 px.
- **Report cell**: line 1 is "recorded → value" with Copy and the status action on the right;
  line 2, on the whole cell width, is the code and the reason for the difference. The status
  action has one width for every status, so Copy forms a column. Company rounding is never amber.
- **Period status** on Report Time is plain 13 text in three colours/marks, not a pill.
- **Statistics summary**: three across from 620 px, stacked below. Durations in words use Mono
  digits and Sans units, at 24 too. Bar labels are in words when a day has 84 px, h:mm in a month.
- **Counts** ("5 projects") are against the chosen status; "x of y" appears only when a search or
  client filter hides something.
- **Keyboard help** is one shortcut per line with the keys in a 104 px column.
- The component gallery is not linked from the app; `--page gallery` still opens it.
- The compact window's rows end on the same 16 px margin as its controls; the scroll bar sits in
  the margin.

## Session dialogs (18 Nov 2026)

- **Add session and Edit session share one form**: 600 wide; Start date · Start time · Duration
  on one row, End date · End time under it. End and duration follow each other: the one typed
  last decides. Moving the start date carries the end date.
- **Time errors never change the dialog's height.** The invalid field turns red; its message
  goes on one fixed 18 px line under the two rows. The same line says "Ends next day" when there
  is no error. With an unreadable start or end the duration shows "—", never a stale value.
- **Long session**: the amber notice states the duration and the primary button becomes
  "Save anyway": the answer to the notice is a labelled button.
- **Start-time popover** opens left-aligned under the control that opened it (the "Started"
  link or the timer). Its end is plain text ("23:10 · now"), not a field; Cancel is a secondary
  button as in dialogs.

## Azure DevOps work items (18 Nov 2026)

- **Selecting is acting.** While work items are selected, a bar pinned to the bottom of the
  project page shows "n selected · Deselect" and the primary "Import n into “Project”". It never
  scrolls away with a long list. There is no import button at the end of the list.
- **Count row**: "71 work items" when all are shown, "x of y shown" with "Load n more" otherwise,
  and "Select the n not imported yet".
- **Rows** mark only what is already in DevTrack ("✔ in Project", green, with the word): an empty
  status means not imported. Below 640 px the mark joins the second line. Start and "···" end on
  the right edge; the menu imports or updates the single item and opens it in Azure DevOps.
- **Toolbar**: one row from 680 px. Below that: mode and "···", then the search field, then the
  DevOps project (flexible) with Refresh.
- **Loading** shows a spinner where the list will be, not also in the toolbar.
- Every label from the source filter and the task list goes through `tr()`.
- Scenes: `--page assigned` and `--page assigned-selected` show a loaded list on demo data.

## Report Time: one destination, one row per value (18 Nov 2026)

Supersedes the side-by-side comparison of destinations and the per-project rows.

- **One destination at a time.** A segmented control (a select when the names don't fit or there
  are more than four) picks the destination; its "···" menu sits on the same row. Under it, one
  line says what the destination asks for: format · rounding · how lines are grouped.
- **Summary**: recorded time · value to enter (with the reason for the difference) · "n of m"
  entered, with the count of values that need a code. Three across from 440 px.
- **One row is one value to enter**, never a project repeated per destination. Columns: what it
  is · Recorded · To enter · Copy · status action. The first column is named after the
  destination's grouping (Job code, Project, Task):
  - by code: the code in Mono, the projects it gathers on the second line;
  - by project: the project with its dot, the code on the second line;
  - by task: the task, then project and code.
- The second line mentions the difference only when a rounding rule applied. Dropped seconds
  and "same as recorded" are explained in the details, not on every row.
- **Details** open under the row (chevron or title): the sessions in the value with Edit, the
  calculation in words, the full line and "Copy full line".
- A value without a code is titled "No code" in amber and its action is "! Add code".
- Below 700 px a row stacks: what · recorded → value · actions.
- Scenes: `--state varied` (mixed sample data), `--page report-2|report-3` for the other
  destinations.

## Project page context (18 Nov 2026)

- **Top bar names the section inside a record.** On a project or client page (titled by the
  record) the left of the top bar shows "Projects and clients" in Medium 14; a click returns to
  the section's first level. Root pages keep an empty left side: their heading names them.
- **Project facts band** under the title, between two dividers: Client (a link to the client) ·
  Tracked · Sessions · Open tasks, each a 12 label over a Medium 16 value. Four across from
  640 px, two below. It replaces the grey subtitle and the one-line facts.
- **Projects list** has a "Tasks" column (open tasks, Mono, "—" for none) between Client and
  Tracked; in compact rows the count joins the client line.
- **The running time in the top bar opens the start-time popover** on every page, the same one
  as on Today. The session's title still opens Today.

## Task rows on a project page (19 Nov 2026)

- Columns: disclosure · task · Last session (date of the most recent session, Mono, from
  720 px) · Tracked · Start · "···".
- The chevron or the title opens the task's sessions under the row, newest first: date · time ·
  description · duration · Edit. Edit opens the same "Edit session" dialog as Today, so a
  finished session can be corrected from the project page. A task with no sessions has a
  disabled chevron.
- Scene: `--page workspace-open`.

## Time fields and the keyboard (19 Nov 2026)

With focus in a time field (HH:MM), ↑/↓ change the part the caret is in:
- hours: one hour, Shift changes nothing;
- minutes: one minute, ten with Shift; minutes carry into the hour (13:59 ↑ is 14:00, 14:00 ↓ is
  13:59), and the day wraps at midnight.
The part that changed stays selected, so repeated keys keep acting on it. The chevrons in the
field step the minutes (Shift: ten). This applies wherever `time_input` is used: Edit session,
Add session and the start-time popover.

## Conversions: the core of the product (19 Nov 2026)

DevTrack exists to show one tracked time the way each platform wants it written.

- **Settings › Conversions** lists the conversions. Each has a name, a form (decimal hours 2.5,
  hours and minutes 2:30, minutes 150), a decimal mark for decimal hours, and its own rounding:
  on by default, to the closest 15 minutes; step and direction can be changed. A live example
  ("2 h 34 min 22 s → 2.5") shows the result. New conversions write decimal hours with a point
  and no trailing zeros.
- **Converted values sit next to the real time, smaller**: under each session's duration on
  Today, under each task's tracked total and beside each session on a project page. Format:
  "Azure DevOps 0.75 · Zucchetti 0:45", 12 px, secondary colour, Mono digits.
- **The real time is never rounded by a conversion.** A separate option, Settings › Display ›
  "Round tracked time to the minute", is off by default: sessions show seconds.
- A conversion is the same record Report Time calls a destination. Codes, grouping and
  "entered" marks stay on the Report Time page, which is developed last.
- Scene: `--page conversions`.

## Azure DevOps work detail, read only (19 Nov 2026)

- Opening an imported task on a project page reads, once, what the source says about the work:
  Remaining, Completed and Estimate in hours, shown beside "Tracked in DevTrack", then the last
  three updates (date · who · "State: New → Active · Remaining: 8 → 5.5"). Only changes to
  state, hours, assignee, title, iteration and comments count as an update.
- It is read on demand and never stored: "Read again" refreshes it. The block is titled
  "Azure DevOps #n · read only". Nothing is ever written to the source.
- An imported task opens even with no sessions; its sessions follow under "Sessions in DevTrack".
- Calls used: GET workitems/{id} (three scheduling fields) and GET workitems/{id}/updates.
- Scene: `--page workspace-devops` (sample data; previews never reach the network).

## Dropdowns, work item filter, task detail from Today (19 Nov 2026)

- **Dropdown (`select`)**: as wide as its longest option, between the trigger's width and 440;
  longer labels are cut with the full text on hover. At most eight rows or the room under the
  trigger: longer lists scroll. From ten options on, a search field at the top narrows the list.
  It closes on a choice, a click outside or Esc.
- **Work item list**: in "Assigned to me" a field filters the loaded list by title or #number
  as you type, without asking Azure DevOps again; the count reads "x of y match". "Search"
  still looks in all of Azure DevOps.
- **Today › row menu › "Open task"** goes to the task on its project page with its row open:
  source data for an imported task, then every session on it.
- An update that only sets a title or an iteration for the first time (the item being created)
  is not listed among the last updates.

## Daily accuracy and speed (19 Nov 2026)

- **Idle time.** While a session runs, a stretch without keyboard or mouse input longer than
  the threshold (Settings › Display, 10 min by default, "Never" to turn it off) is asked about
  when the person is back: "Discard it and continue" (primary), "Discard it and stop", "Keep
  it". Discarding ends the session where the stretch began. Read from the system on macOS; on
  other systems detection stays quiet. Nothing but the idle duration is read.
- **Reminder.** Optional (off by default): every 15, 30 or 60 minutes without a running or
  paused session, Monday to Friday 9–18, a system notification and the in-app message.
- **Favorites.** A session's menu adds it to, or removes it from, the favorites (description,
  project, task, billable; this device only). On Today, with nothing running, they are a row of
  buttons under the session panel: one click starts that session.
- **Duplicate** makes a copy with the same times and opens it for editing. **Split in two**
  divides a session at its midpoint (whole minute); the two parts keep every other detail.
- **Single keys** while nothing is being typed and no menu is open: `S` starts or stops, `C`
  continues the last session.

## Lock and export (19 Nov 2026)

- **Lock.** Settings › Display › "Lock sessions up to" holds a day; "Up to yesterday" sets it,
  "Remove the lock" clears it. A session that starts on or before that day can't be edited,
  deleted, split or duplicated, and no session can be added or moved into the locked days: the
  attempt explains why and where to change the lock. Continuing it, opening its task and
  favorites still work. The lock is part of the synced settings.
- **Export a period to a CSV file** in Settings › Data (this or last week, this or last month): one row per session with
  date, start, end, duration, decimal hours, description, project, client, task, source
  reference, billable, then one column per conversion. Fields are separated by `;` and the file
  starts with a byte-order mark so Excel opens it correctly. XLSX and PDF are not available.

## Estimates and tags (19 Nov 2026)

- **Estimates.** A project and a task can each have an estimate, typed in hours ("8", "1,5")
  or as hours:minutes ("1:30"); empty clears it. The project's field is the fifth cell of its
  facts band; a task's is the first row under its opened row (every task opens now).
- Under the field: "47% used · 4 h 13 min left", amber from 80%, "! Over the estimate by …"
  once passed. A task row shows "tracked / estimate" in the Tracked column, amber when over.
- The first time a running session takes its task past the estimate, one message says so
  (and a system notification on macOS).
- The person's estimate is separate from the source's own (Azure DevOps "Estimate"), which is
  shown beside it and never copied.
- **Tags.** "Edit session" has a Tags field: free words separated by commas. They show on
  Today's second line as "#tag", are a column of the CSV export, and Statistics lists the time
  by tag (a session with two tags counts under both).

## Three views of the timesheet (19 Nov 2026)

Today's view switch has three segments: **List · Calendar · Timesheet**. Above all three, a
**project bar** divides the period's time among projects (one coloured segment each, names over
the widest, the full breakdown on hover); it is hidden when there is one project.

- **Calendar** (day or week). Click an empty part of a day: "Add session" opens at that time
  (15-minute steps, one hour long). Drag a block to move it; drag its lower edge to change its
  end: 5-minute steps, with a blue preview showing the new times. Sessions that overlap sit
  side by side; one that overlaps nothing keeps the day's width. Running sessions, sessions
  crossing midnight and locked days can't be dragged; a session can't end in the future.
- **Timesheet** (always a week). A row per project (with its client), a column per day, a
  total per row and per day. A cell takes hours ("8", "1,5") or hours:minutes ("1:30"); empty
  removes the time. More time lengthens that day's last session on the project, or adds one
  after the day's last session (from 9:00 at the earliest); less time shortens the last
  sessions. Future days are disabled; a running session is not counted until it stops.
  Day columns read down one left edge: the day's header and its total start where the hours
  start inside the cells (the field's 12 padding in); the Total column stays right-aligned.
  Under the grid, two labelled fields side by side (top-aligned, stacking when narrow); what each
  does sits behind an "i" next to its label (`field_info` / `info_tip`, hover; the text is also
  the accessible name). **Add a project to the week**: a project picker; the row appears and the
  cursor goes to its cell for today, or the week's last day that can be filled. A row with no
  hours (one just added, or emptied) has a × in a last column that removes it; a row with hours
  goes away by emptying its cells. **Fill from last week**: the button "Copy last week"; its
  "i" says how many empty cells it fills. With nothing to fill it is disabled and the "i" and its
  own tooltip say why: last week is empty, or those days are already filled or closed. Below 780 px the grid keeps its width and scrolls sideways: the one place that does.
- Scenes: `--page sheet`, `--page week`.

## The period field is a picker (23 Nov 2026)

The date between ‹ and › in the period bar (Today, Report Time, Statistics) looked like an
input but did nothing. It is now one: pointer, hover border, focus ring, a calendar mark at its
right edge, and a click (or Enter / Space) opens a picker under it. At the top **Go to date**, a
field holding the period's date already selected, so typing replaces it; it takes 23/11/2026,
23/11 (this year), 23.11.2026, 23-11-2026, 2026-11-23 and 3/1/27, and Enter jumps. A wrong date
turns the field red with the accepted forms under it. Below, the month (`month_grid`, the same as
the dialogs' date fields) with the current period marked: one day filled, a week or a month
tinted as one block, today outlined. A day jumps there in the same Day/Week/Month; Today, Esc and
a click outside as everywhere. Scenes: `--page period-picker`, `--page period-picker-week`.

## Trying things on sample data (19 Nov 2026)

`devtrack --demo` opens a second, separate copy of the app's data that is kept on disk, in
"DevTrack Demo" next to the real data folder. It is filled once with the mixed sample set
(4 clients, 12 projects, 15 tasks, about three weeks of sessions, two conversions) and then
behaves like the real app: changes are saved. The window title says "DEMO". `--state …` stays
the in-memory preview used for review pictures.

## Vertical space (19 Nov 2026)

Supersedes the 28 px page heading and the two-line session panel in wide windows.

- **Page title and actions live in the top bar** from 700 px: title in SemiBold 17, subtitle
  after it in 13 secondary (cut when short of room), the page's actions right-aligned before
  the session controls. The page itself starts with its content. Inside a project or client
  the bar keeps the section name and the record keeps its own 28 heading. Below 700 px the bar
  already names the page: the subtitle and the actions get a row at the top of the content.
- **Session panel**: 12 px padding and a 36 px description (Medium 16). From 1100 px of panel
  width it is one row: description · project (170) · task (200) · time · actions; the running
  time opens the start-time popover, so the "Started" link is not shown there. Below 1100 it
  keeps two rows; below the old narrow threshold, the stacked layout.
- **"Now" in the calendar**: a red 2 px line with a dot at its start across today's column,
  drawn above the blocks. When today is in the period the hours always reach the present.

- **Segmented controls** keep 4 px between segments, the same as the padding around the group,
  so the selected and the hovered segment never touch.
- **A bordered table's last row has no divider**: the table's border closes it.

## One table, quieter borders, eased hover (19 Nov 2026)

- **Every table looks like Report Time's**: a 1 px rounded border (radius 10) around a header
  band and rows separated by lines; the last row has no line of its own. A hover fill is the
  whole row, edge to edge, and follows the table's corners on the first and last row.
  `table_border` closes a table; `TABLE_TOP` / `TABLE_BOTTOM` are the corner sets. Applied to
  projects, clients, a client's projects, a project's tasks, Today's list and Report Time.
- A control placed by position inside a row (the "···" of the projects list) is drawn in its
  own child, so it can't move where the next row starts.
- **Control borders are quieter**: `border` is #3D3D3D on dark (was #666666) and #C9C9C9 on
  light (was #8C8C8C). This is below the 3:1 that WCAG 1.4.11 asks of a control's boundary: the
  person asked for it; fields still differ from the page by their fill and their label, and
  hover and focus raise the border.
- **Hover fades**: buttons, icon buttons, menu rows, segments and table rows mix between their
  rest and hover colours over about 120 ms (`hover_mix`); with "Reduce motion" it is instant.

- **Rounded boxes clip their content** ("overflow hidden"): `table_border` covers the four
  corners with the colour behind the table (`clip_corners`) before drawing the border, so a
  hover fill, a highlight bar or a divider never shows outside it.
- A table whose header stays fixed while its rows scroll (projects) draws the header's share of
  the border itself: top and sides, joining the border around the rows.

## Session list: day bands, time range, conversions column (19 Nov 2026)

- A day is a band of the table: 36 high, `nav` fill, 14 px inset like the rows, weekday Medium 13, date Mono 12, and the day total right-aligned on the Duration column in the same format as the rows.
- In the compact window a day is a band too (23 Nov 2026): 32 high, 16 above it, a fill halfway between `nav` and `selected` so it reads in both themes (the rows' hover stays lighter). It reaches 12 past the rows on each side, so the weekday (Medium 13, full text colour) and the date (Mono 12) start on the rows' left edge and the day total (Medium 13) ends on their right edge. It opens the group: the last session of a day has no line under it.
- In the compact window a finished session is edited in a small editor, not the full dialog (23 Nov 2026). Where the system lets the app place windows (macOS, Windows, X11) it is a borderless window of its own, always on top, beside the compact window (right, or left when the screen ends there), level with the row and as tall as its content (taller while a menu or the calendar is open); it follows the compact window and losing focus counts as a click elsewhere. The window is see-through (the main window asks for transparency at start, since the graphics context is made once for it; `clear_color` is fully clear; the other windows are covered by their panels), so the editor's frame, raised fill, 1 px line, radius 10, is its shape; its height is the frame's measured height, not the window's. Elsewhere (Wayland, embedded viewports, review pictures) it is a popover by the row: up to 380 wide, under the row when it fits, over it otherwise. Only what a quick fix needs: task and project (pickers, full width), Duration, start → end (time fields), the day (date field, the end's day moving with it), then Delete… and Save. Typing the duration moves the end; changing start or end updates the duration. Notes, tags, billable and the client stay in the full window's editor; a running session still opens it. Esc, ✕ and a click on the page close the popover, asking "Discard your changes?" in its footer when something changed; the pickers' menus and the calendar don't count as outside. The same checks as the dialog (`save_edit`): closed days, end before start, sessions over 24 hours.
- The compact window's running session can change while it runs (23 Nov 2026), as on Today: its task is the same field with suggestions (`today::description`), its project a link opening the project list (`project_picker_in`, Link trigger). A new project drops a task of another project and takes the project's billable default; the timer keeps counting and a notice says so. With no project, "! Choose a project" in amber beside the link. The client follows the project as a second link ("Client portal · Northwind Traders"); choosing a client there takes its only project, or none. No "Tracking now" line: a 4 px green dot beside the time, centred on it, as in the full window.
- Clock intervals read `09:00 → 09:14` everywhere (`range_job`): times in Geist Mono, arrow in Geist.
- From 760 px the conversions are a column of their own ("Conversioni"): one line per conversion, name then value (Mono 13) left-aligned 12 px after the longest name, so each value reads with its name and the values still line up down the list; more than three show two and "+n altre". Time and duration are single lines centred on the row.
- 560–760 px: the interval moves under the duration and the conversions keep their column. Under 560 px: duration, values only ("0.75 · 0:45", names in the hover), interval.
- Conversion text never runs outside its column.

## Session list: one column per platform (19 Nov 2026, replaces the "Conversioni" column)

- Stacked "name value" pairs in one column were not understood. Each conversion is now its own column, placed after the real Duration and headed "Su <piattaforma>" ("On {}"); the cell holds the value alone, left-aligned, Mono 13.
- A value whose format carries no unit gets one in `text2`: decimal hours "0.75 h", minutes "45 min"; H:MM stays "0:45".
- Up to three columns, as many as fit beside a 150 px title. When they do not all fit, the clock interval moves under the duration to give them room. Under 560 px the values fold into one line under the duration (names in the hover).
- Later the same day: the headers are the platform name alone (no "Su"), and every column, Duration included, is left-aligned under its header; the day total follows. The demo conversion is named "DevOps".
- Column order (user's choice): activity · platform columns · clock interval · duration · actions.

## Session list: conversions behind an icon (19 Nov 2026, replaces the platform columns)

- Row order: activity · clock interval · duration · conversions icon · Continue · "···". No conversion columns.
- The icon (`Glyph::Convert`, two opposite arrows) shows on hover a card: "Conversioni", then one line per platform, name in `text2` and value in Mono 13, values aligned.
- Hover alone would hide the product's core values from keyboard users and gives no way to use them, so the icon is also a button: click or Enter keeps the card open and each line copies its value.
- Under 560 px the row keeps the conversions icon and drops the Continue button (still in the "···" menu).
- No conversions set: no icon.

## Session dialogs: the time block (19 Nov 2026)

- Edit session and Add session share `time_block`: one frame (1 px `line`, radius 8, padding 12) holding the whole time of the session.
- Left: two rows, "Inizio" and "Fine", each a row label (13, `text2`), a date field and a time field. Four field labels became two row labels.
- Right, after a vertical divider: "Durata" as the result of the two rows. Geist Mono Medium 22, no field box; an underline (`border`, `text2` on hover, `focus` 2 px when focused, red when unreadable) says it can be typed. Typing it still moves the end.
- Under the duration, in 12 `text2`: the conversions for that duration, following the fields as they change.
- Running session: the End row is read-only ("—", "in corso") and the duration is plain text.
- Under 460 px: row labels go above each row, and the duration follows under a horizontal divider.
- The fixed 18 px line under the block still carries the first time error or "Ends next day".

## Logo and system icons

The mark is a lowercase "d" followed by a dot. All files derive from the two 4096 px masters and live in `assets/icons/`.

- **App icon** (`app-icon.png`, `AppIcon.icns`): white mark on a black rounded square, 824 px in a 1024 px canvas with a 185 px radius (the macOS icon grid). Set at start as the window and Dock icon; the `.icns` is for a packaged `.app`.
- **Menu bar** (`tray-template.png`, 44 px for 22 pt at 2x): a template image, so macOS draws it dark on a light bar and light on a dark one. `tray-light.png` (black) and `tray-dark.png` (white) are the same drawing for systems without template images. The item opens a menu with "Open DevTrack" and "Quit DevTrack". It is not created in screenshot mode.
- **In the app** (`mark.png`, the trimmed mark, white with transparency): `brand_lockup` paints the mark without a tile, tinted with the text colour: the white version on the dark theme, the black one on the light theme. The name follows in Fira Code Bold (`assets/fonts/FiraCode-Bold.ttf`), the face the mark was drawn from, at half the mark's scale and on its baseline. Sidebar: mark 24 high, name 16. Compact window: 21 and 14. Narrow sidebar: the mark alone. Fira Code is used for the name only.

## Window chrome and collapsible sidebar

- **macOS has no title bar.** The window uses a full-size content view with a transparent title bar: the page reaches the top edge and the system paints the three window buttons over the sidebar. The top 32 px of the sidebar (`TITLE_STRIP`) stay free for them; 76 px (`LIGHTS_W`) is the room they take from the left edge. Other systems keep their title bar and both values are zero.
- **Moving the window.** `window_drag` makes the sidebar strip, the top bar and the compact window's header behave like a title bar: drag moves the window, double click zooms it. Controls drawn after it stay on top.
- **Collapsing the sidebar.** One control (`Glyph::Sidebar`, ⌘B) reduces the sidebar to a rail of icons with the mark alone on top, and brings it back. Open: the control sits in the strip right after the window buttons. Collapsed: it is the first thing in the top bar, on the left edge of the page's content. The choice is saved per device (`sidebar_collapsed`). The rail is 76 wide on macOS, to hold the window buttons, and 64 elsewhere. A window under 760 px still shows the rail on its own, without the control.
- **Sample data.** With no visible window title, "DEMO" or "ANTEPRIMA" is written in amber next to the logo.

## Empty states

One shared block, `empty_state` (widgets.rs), for every page that has nothing to show: the page's icon in amber on a 40 px tile, a title (Medium 17) that says what is missing, one or two lines (14, secondary text, at most 480 wide) that say why it matters and how to fill it, then the way forward as buttons. Left-aligned where the data would start, 24 px below the controls, with no card around it. Pages that have sessions elsewhere add "Show the last day with sessions". A count that would read "0 sessions · 0 s" is not shown above it, and Report Time no longer repeats "No sessions in this period" in its toolbar.

Used on: Today (no sessions; nothing needing details), Report Time, Statistics (with a different line when a project filter is on), Projects (none yet; none matching the filters) and a project's tasks.

## Conversions in project task rows

Task rows and the session lines under an open task use the same conversions icon as Today (`today::conversions_icon`): Tracked – icon – Start – ···, and duration – icon – Edit in the session lines. Hover shows the card, click keeps it open to copy a value.

## Project colour on the project page

The colour that marks a project in lists and charts also marks its page, in three places, never on text:
- a 12 px dot before the name, on its first line, as in the list it was opened from;
- a 3 px band in the colour in place of the grey line above the facts (the same band as the project bar on Today);
- with planned hours, a 4 px bar under the field (220 wide at most; 120 on a task) filled with the share used, in the project's colour and amber once passed. The words beside it say the same, so the colour is never the only signal.
Clients have no colour and keep the plain header.

## Planned hours (estimates), revised

- **Local task:** the person types the planned hours (field, placeholder "es. 2:45").
- **Azure DevOps task:** the planned hours are the work item's Original Estimate, read at import, at refresh and when the item is read again (`Task::source_estimate_secs`). Shown read-only as "8 h 00 min da Azure DevOps"; the tooltip says it changes in Azure DevOps. The DevOps block below no longer repeats it. Without one: "Nessuna stima in Azure DevOps".
- **Project:** the sum of its tasks' planned hours, read-only, never typed. Bar and words compare it with the time tracked on those same tasks, so tasks without an estimate don't make it look overrun. A line says how many tasks it covers ("Somma di 1 attività su 2"). With none: "—" and how to fill it. Value, bar and words share the cell's width and left edge. The old typed project value stays in the data but is no longer shown.
- `Task::planned_secs()` is the one source for the task row ("8 h 59 min / 8 h 00 min"), the project sum and the overrun notice.

## Buttons

Every text button is at least 100 wide (`BTN_MIN_W`); fixed-width ones (Start in rows and in the compact window, Pause, Stop, the period bar's Today) use it too. All buttons are 36 high (`CTRL_H`): the secondary and primary ones in a header measure the same, the white fill only makes the primary look larger.

## Dialogs: close control

Every dialog has an X at the top right, on the title's line (`modal_close`, 28 px, quiet until hovered, tooltip "Chiudi (Esc)"). It does exactly what Esc does in that dialog: one with unsaved changes asks before discarding them, the confirm "Leave without saving?" stays on the page, the idle dialog keeps the time.

## Destinations and codes explained

- New destination, step 1 opens with what a destination is: a system where hours are reported, for which DevTrack prepares values in its format and rounding, ready to copy.
- Codes: the dialog says, in the destination's own name, that the code is the job or project number as written in that system, carried by every line of the project.
- Task exceptions: rows start at the same left edge and span the same width as the code row. One line between the code row and the first exception says what an exception is. A project with no tasks shows why there is no "+ Add task exception" instead of an empty list.

## Menus that fit the window

A native window has no room outside itself, so every menu (`select`, project picker, project filter, task picker) opens below its trigger when there are at least 420 px there or more room than above, and above it otherwise (`menu_side`). The scrolling list takes the height left on that side once the search and pinned rows are drawn (`menu_list_h`), never less than three rows, instead of a fixed 280–300 px.

## Destructive buttons

`Kind::Danger` is an outline in red at rest (red border and text on the raised fill), with a faint red fill on hover. "Discard" in the unsaved-changes confirmations uses it, like Delete and Leave without saving.

## Statistics: per day

The chart is 132 high (20% more), columns are up to 56 wide (60% of a day's slot), and hovering a slice shows its project, time and day; the slice under the pointer is drawn at full colour.

## Platforms: one name over conversions and destinations

Conversions (Settings) and destinations (Report Time) are the same record (`TimesheetProfile`). The umbrella name is **platform**: a system where hours are reported.
- **Settings › Platforms** (was "Conversions"): each platform's name and its conversion — how it writes time — plus "Codici progetto…" (opens the destination dialog on the codes step) and "Rimuovi…". "+ Aggiungi piattaforma" opens the same dialog as Report Time's "+ Aggiungi destinazione": one way to create a platform.
- **Report Time** keeps the word *destination* for the same platform, with codes, grouping and lines to copy. Its "···" menu now has "Rimuovi destinazione…", which opens the settings tab with the removal already asked.
- **One editor** for how a platform writes time (`dest::conversion_editor`), used by both: format cards, decimals and separator, "Arrotonda" with step, direction and scope on one row, the rounding note and a live example. The dialog's collapsed "Company rounding" and the "System (optional)" note are gone.

## Menu bar item (macOS)

- **Left click** shows the window (unhides, deminiaturises, brings it to the front). **Right click** or Control-click opens the menu: Nuova sessione, Continua l'ultima sessione, Ferma · Mostra DevTrack · Sincronizza ora, Rendicontazione, Impostazioni… · Esci da DevTrack. Lines that can't act now are disabled (Ferma with nothing running, Sincronizza without a sync folder).
- **Time beside the mark** while a session runs, as h:mm in digits of equal width, refreshed by its own one-second timer so it counts while the window is closed or a menu is open.
- **Closing the window keeps DevTrack running** (the app hides; the item, the timer and reminders go on). ⌘Q and "Esci da DevTrack" quit.
- **Settings › Aspetto › Avvio**: "Apri DevTrack all'avvio del Mac" writes a login item (`~/Library/LaunchAgents/studio.brainup.devtrack.login.plist`) that opens the installed app with `--background`, window hidden. Only offered when running from DevTrack.app.
- The menu's commands are stored by the item and carried out by the app on its next frame, so data is only changed in one place.

## Amber: where you are, and what needs fixing

Amber marks the current place: sidebar icons and the label of every selected tab (`segmented`, the period bar's `segmented_equal`). The selected tab keeps its fill and Medium weight, so the colour is never the only cue (amber on the selected fill: 8.1:1 dark, 4.7:1 light).
Because amber also means "needs attention", warnings carry a shape as well: a leading "!" on every notice ("! 1 sessione da completare", "! Aggiungi codice", overruns), and the Today counter is a filled amber badge with dark (light theme: white) digits instead of an amber outline.

## A project's tasks: toolbar and "Add task" dialog

- No more "Tasks N | Add from Azure DevOps" tabs: the open-task count is already in the project's facts, and adding was looking like a separate place to track time.
- **Toolbar** above the list: a filter by title or #number (it takes the free width), which tasks to show (Attività aperte / completate / Tutte, replacing the "Show done" checkbox) and "+ Aggiungi attività".
- **"Aggiungi attività" dialog** (`add_task_modal`, up to 820 wide): the project it adds to under the title, then "Attività locale" (a title; Enter or the button adds it) or "Da Azure DevOps" (the work items finder, scrolling inside the dialog, with the selection count, Deselect and "Importa N in «Progetto»" in the footer). More sources would join the same switch.
- **Selection never carries over**: opening the dialog, closing it, opening a project and going back all clear the picked work items, so an item imported into one project is not still ticked in the next.

## Calendar fits the window; working days

- **Calendar height:** an hour is the height the window leaves for the calendar divided by the hours shown, between 48 and 140. A tall window shows the whole day without scrolling and each block says more, line by line as it fits: the title (on two lines in a tall block), then the time or what is missing, then the project. A short window keeps 48 and scrolls.
- **Working days** (Settings › Aspetto, after "Week starts on Monday"): seven toggles, Monday to Friday on by default; at least one stays on. Days off are left out of the week calendar, the weekly grid and the per-day chart unless they hold time, and reminders skip them. Stored in the synced settings (`work_days`, Monday = bit 0).

## Settings, redesigned

- **Six tabs by intent**, in this order: Generale (startup, appearance, keyboard), Tempo (how time is shown, the working week, idle time and reminders), Piattaforme, Connessioni, Sincronizzazione, Dati (export, data folder, sample data, the lock on reported days, long sessions). "Aspetto" used to hold six unrelated groups; the lock moved to Data, where it protects data. Under 720 px the tabs become a menu.
- **Grouped rows** (`settings_group` + `SettingRows`, widgets.rs): a group title (Medium 15) and an optional line on what it affects (12, secondary), then one panel (raised fill, 1 px line, radius 10) whose rows are separated by hairlines inset 16. A row: label (14) and help (12, secondary) on the left, measured first so a one-line label sits on the control's middle; the control on the right. `stacked` puts a wide control (day toggles, folder, date) under its label; `block` holds free content (status lines, shortcut list).
- **Switches** (`switch`) replace checkboxes for on/off settings: 40×22, amber track when on, like every selected state; 120 ms transition; focus ring.
- **Keyboard shortcuts** are a compact reference list with key caps (`keycap`).
- **Help text is one short sentence** per row; longer explanations moved to the group's line.

## Sync folder: one menu

The shared folder is one menu (Settings › Sincronizzazione): "Non usata: solo questo dispositivo", each cloud folder found on this Mac, the current folder when it is none of those, and "Altra cartella…" (the system picker). Names are readable and unique: "Google Drive", "Synology Drive · nas" / "· Pot" (an e-mail account name is never shown), the same folder reached two ways is listed once. The chosen folder's full path sits under the menu. Picking a folder still opens the review before saving; picking "Non usata" stops syncing at once, without touching the folder. The separate field, "Scegli…" button, "Trovate su questo computer" links and "Stop syncing" button are gone.

## Compact window

The logo sits in the middle of the top bar, clear of the window buttons on the left and of Expand on the right.

## Settings use the whole width

The settings column is the full width of the page (no 640 cap). The tab bar spans the same width in six equal segments (`segmented_equal`), and every group title, line and panel shares that left and right edge, in all six tabs. Under 720 the tabs become a menu.

## Settings for time, theme and the menu bar

- **Working hours** (Tempo › Settimana lavorativa): start and end in half hours (default 09:00 → 18:00). Reminders only fire inside them on working days; an empty calendar shows them.
- **Sessions** (Tempo): a default project for sessions started without one (fewer sessions waiting for details), and the calendar step (1, 5, 10, 15 or 30 min) for creating, moving and stretching sessions.
- **Theme** (Generale): System, Dark or Light; System follows the Mac's appearance.
- **Next to the menu bar icon** (Generale, macOS): the running session's time, today's total, or nothing.
- **Compact window** (Generale): keep it above other windows.
Left out on purpose: Pomodoro and goals (not reporting), activity recording and external calendars (privacy decision pending), snoozing reminders (system notifications have no actions here), Dock badge (the menu bar already shows the time), running-entry override prompt (starting a session already stops the running one).

## Import from another time tracker

Settings › Dati › "Importa da un altro strumento di tracciamento" (`integrations::entries_import`). The person types the service's API address and their API token; the import makes one read-only GET of `<address>/api/v9/me/time_entries` with the period and `meta=true`. Neither the address nor the token is stored: both are cleared after the import or Cancel. Period: last 7, 30 (default) or 90 days. Running entries are skipped.
Before anything is written, a preview says how many sessions are new, how many are already in DevTrack (same start, end and description: a second import adds nothing), and which projects and clients will be created. Projects and clients are matched by name, letter case aside; new projects take the colour the service gave them. Sessions keep description, times, billable and tags.

## Filling a session from its description

Wherever a description is typed (the timer, the compact window, Edit session, Add past time), typing opens the same list (`today::suggest_pick`): previous sessions that match, each with its task, project and client, then matching projects. Choosing a previous session fills description, project (so its client) and task at once; choosing a project fills the project. In the dialogs the list opens only once the person types, not on the description already there. The list sits above dialogs.
The project field shows the client after the project once chosen ("Design · Northwind"), so two projects with one name stay distinct.

## The task says what a session was; the description is a note

A session is named by its **task**:
- The timer's field ("Su quale attività stai lavorando?") is the task. Typing suggests existing open tasks (most recently used first) with project and client; choosing one fills task and project. A name that matches no task becomes a local task of the chosen project on Start (or on leaving the field while running). The timer has no separate task picker.
- Edit session and Add past time: Task, then Project, at the top; the description becomes **Note** (optional), next to the tags.
- Lists, calendar, compact window, menus and Report Time show the task as the title (`session_title`: task, else note, else "Sessione senza titolo"); under it project · client · note (`session_context`; a note equal to the task is not repeated).
- "Needs details" means no project, or neither task nor note (`needs_completion`).
- Import: each description becomes a task of its project, one per name; a description starting with the number of an Azure DevOps work item already in DevTrack is linked to it. Settings › Dati › "Descrizioni in attività" converts sessions already here by the same rule (shown only while some are left).
- The conversion also runs by itself once per data set when DevTrack opens (`settings.descriptions_are_tasks` marks it done), with a notice of how many sessions changed; the Settings row stays for sessions converted later.
- Timer row, wide windows: the task field and the project (240) end at the right edge of the period bar's "Today" button below (read from the last frame), so the rest of the row is left to the time. Every control's text is 14 px, the task field included.

### Status box (`status_chip`)
A status as tall as a button (36): the words in the status colour on a pale fill of the same colour (7%, 13% on hover), no lighter border, so it measures like the buttons beside it. Display only when there is nothing to do (green "✔ All sessions completed", grey "● A session is still running"); a button with a chevron when there is (amber "! N sessions need details", opening them on Today). Always on the right. Report Time: on the period toolbar's row when it fits (its width is measured first), opening the sessions on Today. Today: on the right of the total's line ("Show only these" with a chevron; while narrowed, "Showing N…" with a cross that shows all again), below it when narrow. Compact window: on the right, opening the first session to complete. The total on Today and its label share one baseline.

### Period bar and the top of Today
- Moving in time is one group: Previous · date (236) · Next · Today, 8 apart. Then 16 and Day/Week/Month at the width of their words. Shared by Today, Report Time and Statistics.
- Today's views (List · Calendar · Timesheet) sit right of the bar when it has room (`period::status_fits`); otherwise right on the summary row, just above the list they change, with the status box beside them. They never wrap under the date.
- Timer: the task, then the time and the actions, on one row; under the task's text its project and client as underlined words (`link_trigger`) that open the same searchable menus as the session window, and while it runs "Started hh:mm". In a menu only the search stays put: the recent ones, No project and the groups scroll together, at least as tall as the room the window has (`min_scrolled_height`, or egui keeps a size remembered from a short search).
- Narrow window: the subtitle and the page actions share one row.
- Status box fill: the colour at 14% in gamma (22% on hover); a linear 7% read as 30% on black.


## Report details: three blocks — 21 November 2026

A row's details are three blocks with a 14 medium heading each, 20 apart, on one left edge:
the sessions in the value; **Calculation** ("0:35:39 recorded → 0.5 to enter", then only the
differences that exist, in words); **Line for a spreadsheet**, with one sentence on what it is
for (paste into Excel, Numbers or Google Sheets, one column per part; its layout is set in the
destination). The line is a field-coloured box (tabs shown as wide gaps, the full line in its
tooltip) with **Copy line** on its right; below 420 the button goes under it. The amber
"no code" notice follows the line it describes. The destination's template field is named
"Line for a spreadsheet" too.


## Dark theme without pure black — 21 November 2026

The dark canvas is #131312, not #000: a near-black with a trace of warmth (the family of the
other Brainup apps), in steps that keep the layers apart — canvas #131312, sidebar #181817,
panels #1C1C1B, fields #161615 (a little below their panel), hover #252523, selected #2B2B29,
lines #2E2E2B, control borders #464642, text #EEEDE9 / #A6A59E. Accents are unchanged. Text is
15.9:1 on the canvas; the weakest pair, red on the selected row, is 4.8:1. The window is told
its theme (`ViewportCommand::SetTheme`), so macOS draws the dark window edge, not the light one.

Scrolling page lists fade at their edges (`scroll_fades`): where content continues above or
below, a 28-unit band goes from the canvas colour to transparent, growing in with the first
pixels scrolled.
