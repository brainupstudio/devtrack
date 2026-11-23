# DevTrack guide

DevTrack keeps one real tracked time per session and turns it into the value each system you
report hours to expects. This guide follows the app page by page. The interface is in English
or Italian (**Settings › General › Language**); the names below are the English ones.

## The window

On the left, the sidebar: **Today**, **Work**, **Report Time** and **Statistics**, with
**Settings** at the bottom. The number next to Today counts the sessions that still need
details. The control at the top collapses the sidebar to its icons (⌘B / Ctrl+B).

The **compact window** (⌘⇧M / Ctrl+Shift+M, or the button at the top right) is a small timer
that can stay above other windows (**Settings › General › Compact window**). Clicking a session there opens a
small editor by the row: task, project, duration, start and end, and the day; the full editor, with
notes, tags and billable, is in the main window. On macOS a menu bar
item shows the running session's time or today's total; closing the window keeps DevTrack
running there, and **Quit DevTrack** in its menu (or ⌘Q) ends it.

## Today

**The timer.** Type what you are working on in the task field. DevTrack suggests open tasks
(the most recent first), then the Azure DevOps work items assigned to you that are not in
DevTrack yet, then projects; a name that matches no task becomes a new task when you press
**Start**. A work item becomes a task only when its session starts, in the project its
neighbours went to (or the one you chose); with no project to go to, the timer says so in amber
and you can choose one at any time. The assigned work items are read in the background and kept
on this computer, so they are there at once and offline; when the list is old or Azure DevOps
can't be reached, the suggestions say so and offer to read it again. A **#number** that is not
in the list can be looked up from the suggestions. Under the field, the project and
the client are underlined: click either to choose it from a searchable list, or create a new
one by typing its name. Choosing a client first narrows the projects to its own.

While a session runs: **Pause**, **Stop**, and **…** for more. **Started hh:mm** under the field
changes when it started.

**The period.** Go back and forth with the arrows, return with **Today**, and switch between
**Day**, **Week** and **Month**. Click the date between the arrows to type one (23/11/2026, 23/11,
2026-11-23) or pick it on a small calendar: the view jumps there and keeps Day, Week or Month. On the right, three views:

- **List**: every session with its time and duration; ⇄ shows the value for each platform,
  ▷ starts it again, **…** edits, duplicates or deletes it.
- **Timeline**: the sessions placed on the hours of the day; drag to create, move or stretch
  them (the step is set in **Settings › Time & days › Sessions**).
- **Timesheet**: one row per project, one column per day. Type a day's hours in a cell (8, 1,5
  or 1:30): DevTrack lengthens that day's last session on the project, or adds one. Under the
  grid, **Add a project to the week** adds the row of a project with no time yet (× removes a
  row that has no hours), and **Copy last week** copies last week's hours into the empty cells
  up to today. The **i** beside each one says what it will do.

Under the bar, the period's total and a status box: amber when sessions need details (click it
to show only those), grey while a session runs, green when every session is complete.

**+ Add past time** adds a session that already happened; it never touches the running one.
The same button is in the top bar of every page when no session is running: the dialog opens
over the page you are on.
**Edit session** (from **…** or by clicking a row) changes task, project, client, start, end,
duration, note, tags and billable. In both, the task list also offers the Azure DevOps work items
assigned to you that are not in DevTrack yet: choosing one imports it and picks it. Changing the duration moves the end; an end before the start
is an error, never a jump to the next day.

## Work

Three tabs: **Tasks**, **Projects** and **Clients**. Work opens on Tasks.

- **Tasks**: every task with its project, client, source and tracked time. Click a row to open it
  in place: its planned hours, its source data and its most recent sessions (**Show all** lists
  the rest). The project and client under the title open them. When a task has planned hours,
  the row shows "tracked / planned", with a **!** once they are passed. Filters: search, status
  (open, done, all), client, project (including "No project"), source (added in DevTrack, or
  read from Azure DevOps) and estimates (over the planned hours, or without any).
  **+ Add task** asks for the project.
- **Projects**: name, client, colour, whether new sessions are billable, and planned hours (the
  sum of their tasks' estimates). A project lists its tasks, which open in place the same way.
- **Clients**: active and archived ones, with their projects.

Every list starts with a search field that works the same way: each word must appear in the
name or in what it belongs to (a task's #number, project and client). ⌘F / Ctrl+F puts the
cursor in it and Esc empties it. The row above a project or a client shows the way you came;
each step in it, and Back (Alt+←), returns there with the list as you left it.

## Report Time

A **destination** is a system you enter hours into. Each one decides:

- **the format**: hours and minutes, minutes, or decimal hours with a number of places;
- **the rounding** your company requires: to the closest, up or down, to a step of minutes;
- **the grouping**: one line per day and code, per day, project and code, per day, task and
  code, or per week and code;
- **the codes**: a code (and an optional activity) for each project, exceptions for single
  tasks, and a fallback code.

For the chosen period each destination lists its lines with the recorded time and the value to
enter. **Copy** copies the value; **Confirm entry** marks it as entered (it changes nothing
elsewhere). Open a line to see its sessions, how the value is calculated, and a line for a
spreadsheet: pasted into Excel, Numbers or Google Sheets, each part lands in its own column.
Its layout is set in the destination's settings.

A line without a code says so and offers **Add code**. If a confirmed line changes later, it is
marked as changed after confirmation.

## Statistics

Time per day, per project and per task for the chosen period, with a project filter.

## Azure DevOps

In **Settings › Connections**, add your organisation and a personal access token with read
access to work items. The token is kept in the system keychain. DevTrack only reads: it never
writes to Azure DevOps.

In **Work**, choose **+ Add task › From Azure DevOps** (on the Tasks tab or in a project): you find the work
items assigned to you, or search by title or number, and import them into the project with
their number and estimate. They
are refreshed from Azure DevOps; their details (remaining and completed work, latest updates)
are shown read-only.

## AI assistants (MCP)

An assistant such as Claude can read your tracked time and turn it into answers and reports,
for example "How many hours did I spend on Northwind last week, by project?" or "Prepare the
weekly report for the Client portal PM". DevTrack gives it clean figures, computed exactly as
the app shows them; the assistant writes the text around them.

1. In **Settings › AI assistants**, read what an assistant can see, then turn on **Let AI
   assistants read DevTrack**. It stays off until you do, and applies to this device only. The
   line under the switch checks by itself that assistants can reach DevTrack.
2. Under **Connect your assistant**, pick yours:
   - **Claude Desktop**: **Add to Claude Desktop** adds DevTrack to Claude Desktop's
     configuration after a confirmation, keeping a copy of the file; then restart Claude
     Desktop. **Set it up by hand** shows the entry to add yourself.
   - **Claude Code**: run the command shown, once, in a terminal.
   - **Other assistants** (Codex CLI, Cursor…): the command and its arguments.
3. **Try asking** lists questions to start with, ready to copy.

If DevTrack is moved or replaced, the line under the switch says so and **Update Claude
Desktop** points Claude Desktop to the new place.

What the assistant can do:

- **Read only.** It reads sessions, clients, projects, tasks, estimates and the values to enter
  in Report Time. It can't change, add or delete anything, and never sees tokens or connection
  details.
- **Reports.** In Claude Desktop, the **+** menu lists three ready-made reports: *Weekly report
  for a PM*, *Project status* and *Client summary*. Their tables are computed by DevTrack, so
  the totals match the app.
- **Anonymized.** Ask for an anonymized report when it goes outside the team: clients become
  "Client A", "Client B"… and session notes are left out.

What the assistant reads goes to the service that runs it (Anthropic for Claude, OpenAI for
ChatGPT and Codex). ChatGPT itself only connects to servers on the internet, so it can't reach
DevTrack on your computer yet.

## Sync between your devices

**Settings › Sync** uses a folder you already share: iCloud Drive, OneDrive, Dropbox or a
network share. Each device writes its own file there and reads the others'. Only DevTrack's data
is synced; there is no server and no account. If two devices run a session at once, the one
started last keeps running and the other stops where it started, and the status says so.

## Settings

- **General**: updates, startup and the menu bar (macOS), theme (System, Dark, Light),
  language, reduced motion, the compact window, keyboard shortcuts.
- **Time & days**: how time is shown, the working week and hours, a default project and the
  calendar step for sessions, idle time and reminders.
- **Platforms**: each platform's name and conversion, and its project codes.
- **Connections**: Azure DevOps organisations and tokens.
- **Sync**: the shared folder and the state of each device.
- **Data**: export all sessions (copied as CSV) or a period to a CSV file, the data folder,
  sample data, turning old descriptions into tasks, locking days already reported, the warning
  for long sessions, and **Import from another time tracker**: type the service's API address
  and your API token to read your entries once, review what would change, then import. Neither
  is saved.

## Keyboard shortcuts

| Keys (macOS / Windows and Linux) | What |
| --- | --- |
| ⌘1 – ⌘4 / Ctrl+1 – Ctrl+4 | Today, Work, Report Time, Statistics |
| ⌘Enter / Ctrl+Enter | Start or stop the session |
| ⌘P / Ctrl+P | Pause or resume |
| S | Start or stop, when you're not typing |
| C | Continue the last session, when you're not typing |
| ⌘B / Ctrl+B | Show or hide the sidebar |
| ⌘F / Ctrl+F | Search the list in Work |
| Alt+← | Back, in Work |
| ⌘⇧M / Ctrl+Shift+M | Compact window |
| Esc | Close a dialog |
