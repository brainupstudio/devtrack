# DevTrack guide

DevTrack keeps one real tracked time per session and turns it into the value each system you
report hours to expects. This guide follows the app page by page. The interface is in English
or Italian (**Settings › General › Language**); the names below are the English ones.

## The window

On the left, the sidebar: **Today**, **Work**, **Report Time** and **Statistics**, with
**Settings** at the bottom. The number next to Today counts the sessions that still need
details. The control at the top collapses the sidebar to its icons (⌘B / Ctrl+B).

The **compact window** (⌘⇧M / Ctrl+Shift+M, or the button at the top right) is a small timer
that can stay above other windows (**Settings › General › Compact window**). On macOS a menu bar
item shows the running session's time or today's total; closing the window keeps DevTrack
running there, and **Quit DevTrack** in its menu (or ⌘Q) ends it.

## Today

**The timer.** Type what you are working on in the task field. DevTrack suggests open tasks
(the most recent first) and Azure DevOps work items with their project and client; a name that
matches no task becomes a new task when you press **Start**. Under the field, the project and
the client are underlined: click either to choose it from a searchable list, or create a new
one by typing its name. Choosing a client first narrows the projects to its own.

While a session runs: **Pause**, **Stop**, and **…** for more. **Started hh:mm** under the field
changes when it started.

**The period.** Go back and forth with the arrows, return with **Today**, and switch between
**Day**, **Week** and **Month**. On the right, three views:

- **List**: every session with its time and duration; ⇄ shows the value for each platform,
  ▷ starts it again, **…** edits, duplicates or deletes it.
- **Timeline**: the sessions placed on the hours of the day; drag to create, move or stretch
  them (the step is set in **Settings › Time & days › Sessions**).
- **Timesheet**: one row per project, one column per day.

Under the bar, the period's total and a status box: amber when sessions need details (click it
to show only those), grey while a session runs, green when every session is complete.

**+ Add past time** adds a session that already happened; it never touches the running one.
**Edit session** (from **…** or by clicking a row) changes task, project, client, start, end,
duration, note, tags and billable. Changing the duration moves the end; an end before the start
is an error, never a jump to the next day.

## Work

Three tabs: **Projects**, **Clients** and **Tasks**.

- **Projects**: name, client, colour, whether new sessions are billable, and planned hours (the
  sum of their tasks' estimates).
- **Clients**: active and archived ones, with their projects.
- **Tasks**: every task with its project, client, source and tracked time. Filters: search,
  status (open, done, all), client, project (including "No project") and source (added in
  DevTrack, or read from Azure DevOps).

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

In **Work**, open a project and choose **Add task › From Azure DevOps**: you find the work
items assigned to you, or search by title or number, and import them into the project with
their number and estimate. They
are refreshed from Azure DevOps; their details (remaining and completed work, latest updates)
are shown read-only.

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
| ⌘⇧M / Ctrl+Shift+M | Compact window |
| Esc | Close a dialog |
