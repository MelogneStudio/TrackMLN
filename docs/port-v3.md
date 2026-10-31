# TrackMLN3 Rewrite Plan

> TrackMLN2 is the current Rust + Tauri version.
> TrackMLN3 will be a full rewrite using WinUI 3 + C# + XAML.

## 1. Keep TrackMLN2 intact

Do not delete or replace TrackMLN2 yet.

Use it as the reference for the existing functionality:
- Foreground-app tracking
- SQLite session history
- Today / Week dashboards
- Goals and screen-time limits
- Warn / Annoy behavior
- Global shortcut
- System tray
- Settings
- Startup behavior
- App-name resolution

TrackMLN3 should replace the implementation, not the functionality.

## 2. Prove the transparent window first

Before rebuilding the actual app, make a tiny WinUI 3 prototype.

Test:
- Borderless window
- True transparent window/background
- Desktop visible through transparent areas
- Custom UI rendered over the transparent window
- Rounded/glass surfaces
- Always-on-top behavior
- Show/hide behavior
- Global hotkey
- Basic animations

**This is the main technical gate.**

Do not start the full rewrite until the window behaves the way TrackMLN3 needs.

## 3. Create the TrackMLN3 project

Use:

- C#
- XAML
- WinUI 3
- Windows App SDK
- SQLite

Basic structure:

```text
TrackMLN3/
├── App.xaml
├── MainWindow.xaml
├── Views/
│   ├── Today/
│   ├── Week/
│   ├── Goals/
│   └── Settings/
├── Services/
│   ├── TrackerService
│   ├── DatabaseService
│   ├── HotkeyService
│   ├── TrayService
│   └── SettingsService
├── Models/
└── Assets/
```

## 4. Rebuild the backend

Implement the core services independently from the UI.

### TrackerService

Reimplement:
- Foreground-window tracking
- Session tracking
- Idle/unknown window filtering
- Executable-path identity
- Friendly app-name resolution
- Custom executable labels

### DatabaseService

Recreate the local SQLite storage for:
- Session history
- Goals
- Settings/data required by TrackMLN3

Decide whether TrackMLN3 should migrate the existing TrackMLN2 database before release.

## 5. Build the application shell

Implement:
- Transparent main window
- Navigation
- Today
- Week
- Goals
- Settings
- System tray
- Global shortcut
- Hidden startup
- Close → hide
- Always-on-top behavior

Focus on functionality before visual polish.

## 6. Rebuild the dashboards

### Today
- Total tracked time
- Most-used apps
- Hourly usage

### Week
- Weekly totals
- Most-used apps
- Daily usage trends
- Per-day app breakdown

Use TrackMLN2 as the functional reference.

## 7. Rebuild Goals / limit enforcement

Implement:
- Per-app warn time
- Per-app annoy time
- Warn popup
- Full-screen Annoy overlay
- Snooze behavior
- Escalating snooze periods
- Cooldown/interaction behavior

Keep the existing behavior unless intentionally redesigning it.

## 8. Redesign the visual identity

This is a clean rewrite, so do **not** feel obligated to reproduce TrackMLN2 pixel-for-pixel.

Design TrackMLN3 around the native transparent window.

Explore:
- Glass surfaces
- Blur
- Depth
- Native composition effects
- Subtle animations
- Better transitions
- Custom navigation
- More immersive overlay behavior

The window itself should feel like part of the experience.

## 9. Windows integration

Finish:
- Startup with Windows
- System tray
- Global hotkey
- Show/hide animation
- Close → hide
- Always-on-top behavior
- Full-screen Annoy mode

## 10. Installer

Once TrackMLN3 is stable:
- Build the application
- Create the installer
- Install to the desired user location
- Create Start Menu shortcut
- Configure startup
- Launch after installation
- Test uninstall/update behavior

## 11. Testing

Before replacing TrackMLN2:

- Test long tracking sessions
- Test sleep/idle behavior
- Test database persistence
- Test goals
- Test Warn
- Test Annoy
- Test global hotkey
- Test tray behavior
- Test startup
- Test transparent windows on a clean Windows environment
- Test installer

## 12. Release

Only after TrackMLN3 passes testing should it replace TrackMLN2 as the main release.

## Order

```text
TrackMLN2 stays untouched
        ↓
Transparent WinUI 3 prototype
        ↓
TrackMLN3 project setup
        ↓
Native window + hotkey + tray
        ↓
SQLite + tracking backend
        ↓
Today
        ↓
Week
        ↓
Goals / limits
        ↓
Settings
        ↓
Full visual redesign
        ↓
Installer
        ↓
Testing
        ↓
TrackMLN3 release
```

## Most Important Rule

**Prove the transparent native window before doing the full rewrite.**

If that works, TrackMLN3 can be built around the window behavior that TrackMLN2 could not provide cleanly.