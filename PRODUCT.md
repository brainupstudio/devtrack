# Product

<!-- impeccable:product-schema 1 -->

## Platform

web (this record covers the public download page; the product itself is a desktop app for macOS, Windows and Linux built with Rust and eframe/egui)

## Stack

Download page: static site on GitHub Pages, three.js for the 3D scene. Download links point to the latest GitHub Release of `brainupstudio/devtrack.application`.

## Users

Public audience: developers and team members at any company who track their working time and then have to report the same hours into more than one platform (a work-item tracker such as Azure DevOps, a company timesheet such as Zucchetti, a client's own system), each with its own format and rounding. Also individuals who just want to organise their own time. They reach the page from a link or a colleague's recommendation, on the machine where they would install the app.

## Product Purpose

DevTrack keeps one real tracked time per session and task, and turns it into the value each platform expects. Example: 44 minutes tracked → rounded to 45 → `0.75` for Azure DevOps (decimal hours) and `0.45` for a Zucchetti timesheet (hours.minutes). Hours stop being retyped, recalculated and lost between platforms. Success for the page: the visitor understands that in one look and downloads the build for their OS.

## Positioning

One time, every format. Other trackers record time; DevTrack's mechanism is the per-platform conversion (format + rounding per destination) shown next to the real time, plus read-only Azure DevOps work items with their estimates. It never writes into other systems.

## Operating Context

Daily: timer with pause/resume, past sessions, weekly calendar and timesheet grid. Projects and clients with colours; tasks local or imported from Azure DevOps (read-only), planned hours from DevOps estimates. Reporting page lists, per platform, the values to enter with codes per project. Statistics per day, project, task. Sync between devices through a shared folder (e.g. iCloud Drive, OneDrive). macOS menu-bar item with the running session time. Italian and English UI, dark and light theme.

## Capabilities and Constraints

- Version 0.1.5. Packages: `DevTrack-<version>-macOS.dmg` (macOS 11+, universal), `DevTrack-<version>-Windows-x64.zip` (Windows 10/11), `DevTrack-<version>-Linux-x64.tar.gz` (Linux x64).
- Not signed yet: each OS shows a warning on first launch; first-launch instructions are in CHANGELOG.md and must be reachable from the page.
- Windows and Linux builds are produced by CI but not yet tested by hand. Menu-bar item and launch-at-login are macOS only.
- Azure DevOps access is read-only. Credentials stay in the system keychain; personal data stays on the device or the user's shared folder.
- No web, iOS or Android version.
- Undecided: licence, price (free is assumed by the release, not stated), public repository visibility.

## Brand Commitments

Name: DevTrack. App icon in `assets/icons/`. The page must be bilingual, Italian and English, with a language switch. The user wants a dark page with one minimal 3D object (three.js), very little copy that is absolutely clear, in the spirit of weevolveit.com. No generic AI-style feature lists.

## Evidence on Hand

App screenshots in `docs/screenshots/`; app icon in `assets/icons/`. No testimonials, user counts, customer logos or ratings exist: none may be invented.

## Product Principles

- Show the mechanism, not a feature list: one real time becoming each platform's value.
- Essential and clear over complete; the app itself is built this way.
- Honest about state: unsigned builds, untested platforms and read-only integrations are said plainly.
- The download for the visitor's own OS is always one click away.
