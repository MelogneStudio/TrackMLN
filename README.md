<img align="center" src=".github/assets/logo.svg">

TrackMLN is a Windows desktop app for screen time, tasks, focus sessions and your daily schedule, all in one place.

It runs as a Tauri app with a React frontend, logs usage locally, and shows everything in one dashboard you can toggle with a global shortcut. Now with screen time limits that will personally bother you until you close YouTube.

### ⚠️ Common "bugs" for people who don't read the readme and go straight to downloading
1. **Does not** open directly by clicking it through windows start menu. **Use Hotkey or system tray** Control + Shift + Space
2. It is **NOT** a virus. It costs money and alot of time to buy a certificate to stop that from happening, as a hobby dev, I cannot buy that.

> ℹ️ Windows only. (Because I don't have MacOS or Linux 🥀. I won't download Linux for 1 single repo)

## Features

**Screen time**
- Tracks the active app every second, stored locally in SQLite
- Today, 7 day and 30 day views with hourly usage bars
- Per-app deep dives, comparison with the previous period
- Daily average, busiest day, longest session and session count
- Pause when idle

**Focus**
- Custom presets (focus length, break length, sessions)
- Link a session to a task
- Ring timer with pause, skip, stop and +5 minutes
- 7 day focus history
- Focus overlay, summoned with a global hotkey or shown when a phase ends

**Tasks**
- Priority, estimate, due date/time and notes
- Repeating tasks via templates
- One click from a task to a focus session

**Timetable**
- Week and day views with time blocks
- Overlapping blocks sit side by side
- Live "Now" line

**Limits**
- Limit total screen time or individual apps
- **Warn**: a small popup at the bottom of the screen. Easy to ignore.
- **Annoy**: a full-screen popup you can't move or Alt+F4. Snooze is your only option, and it shrinks every time: 5 min → 3 min → 1 min → forever. You did this to yourself.

**Personalisation**
- Color themes and Solid → Glassy sidebar
- Manage Names: map `javaw.exe` to `Minecraft`
- Configurable global hotkeys, onboarding flow, system tray
- Local-first: no accounts, no cloud

## How to Download

Click the download button, or grab `trackmln-installer-v3.x.x.exe` from the Releases page.

<p align="left">
  <a href="https://github.com/WTRMLNv1/TrackMLN/releases/download/v3.0.0/trackmln-installer-v3.0.0.exe">
    <img src="https://raw.githubusercontent.com/WTRMLNv1/WTRMLNv1/main/TrackMLN-assets/badges/download_button.svg" height="30">
  </a>
</p>

<p align="left">
  <a href="https://github.com/WTRMLNv1/TrackMLN/releases">
    <img src="https://raw.githubusercontent.com/WTRMLNv1/WTRMLNv1/main/TrackMLN-assets/badges/releases-page.svg" height="30">
  </a>
</p>

> ⚠️ Do **not** install `trackmln-main-v3.x.x.exe` - it's not intended for normal use.

1. Run the installer
2. If Windows SmartScreen appears, click `More info` → `Run anyway` *(the app isn't code-signed yet)*
3. Finish setup

The installer copies TrackMLN to your local app data folder, creates a Start Menu shortcut, adds it to Windows startup and launches it. To disable startup: `Settings → Apps → Startup Apps → TrackMLN → Off`

## What's New

### v3.0.0
- Brand new UI and onboarding flow
- Added Focus page: presets, ring timer, task linking, 7 day analytics
- Added focus overlay with a global hotkey
- Added Tasks with priorities, estimates, due times and repeating templates
- Added Timetable with week/day views and a live "Now" line
- Added 7 day and 30 day ranges, per-app deep dives and new stats
- Added Pause when idle
- More reliable tracking across sleep and midnight
- Live progress meters for Warn and Annoy limits
- Added color themes and sidebar translucency slider
- Added a global hotkey for the quick overlay
- Renamed "Exe Labels" to Manage Names

Older versions are on the [Releases page](https://github.com/WTRMLNv1/TrackMLN/releases).

## Screenshot

<img src=".github/assets/Screenshot 2026-11-01 205254.png" a="Dashboard">

## How Tracking Works

The Rust backend polls the foreground window every second and writes a session to the local database whenever the focused app changes. Idle time is paused, sleep is never counted, and sessions that cross midnight are split across both days.

App names are resolved in this order:
1. **Manage Names** - your custom names, checked first
2. **AppxManifest** - for Windows Store apps like `ChatGPT`
3. **Version info** - Windows executable metadata
4. **Prettify fallback** - strips `.exe`, capitalizes, cleans up separators

Apps are tracked by full executable path internally, so renaming an app won't split its history.

## Tech Stack

Tauri 2, React 18, TypeScript, Vite, Rust, SQLite via `rusqlite`, Recharts

```text
.
├─ src/                     React UI for the main app
├─ src-tauri/               Rust backend for the main app
├─ installer/               Separate Tauri app that installs TrackMLN
│  ├─ src/                  Installer frontend
│  └─ src-tauri/            Installer backend
```

## Local Data

Everything is stored in the Tauri app data directory:

- `trackmln.db` - sessions, goals, tasks, focus sessions and timetable
- `settings.json` - shortcuts, theme, translucency, app names and onboarding state

The main window starts hidden. Closing it hides it instead of quitting, so use the tray icon to exit fully. Default shortcut: `Ctrl + Shift + Space`.

## Known Bugs

There might still be small bugs, mostly in screen time limits and the new v3 UI. If anything explodes, report a bug :,)

<p align="left">
  <a href="https://github.com/WTRMLNv1/TrackMLN/issues">
    <img src="https://raw.githubusercontent.com/WTRMLNv1/WTRMLNv1/main/TrackMLN-assets/badges/report-a-bug.svg" height="30">
  </a>
</p>

## Development

Requirements: Windows, Node.js + npm, Rust toolchain, Tauri prerequisites for Windows.

```powershell
npm install          # from the project root, the installer reuses this
npm run tauri:dev    # Vite on http://localhost:1420 + the Tauri app
npm run tauri:build  # build the main app
```

> If you run a debug binary without the dev server running, it'll probably break. Use a release build or the installer.

To build the installer, build the main app first, then:

```powershell
cd installer
npm run tauri:build
```

The installer build copies the main app executable into `installer/src-tauri/assets/trackmln.exe` automatically. That file and other generated binaries are gitignored.

## License

Source available under an NC License.

---

Made with 💚, Debugged with 😭 by [WTRMLN](https://github.com/WTRMLNv1) [@Melogne Studio](https://github.com/MelogneStudio)
© 2026 Melogne Studio
