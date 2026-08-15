# GazeGuard Design

## Screens

### Settings window

`src/index.html` is settings only. It contains:

- Break schedule
- Notifications
- Break experience
- Appearance
- Behavior
- Status and Test Break actions

Desktop settings fill the window at 800×600. Small/mobile layouts use a compact 400×560 card.

### Break screen

`src/break.html` is separate from settings. It is a full-screen break experience and must not contain settings controls.

Tray actions:

- Settings → show the settings window
- Test Break → show the break screen
- Quit → exit the app

## Theme

Supported values:

- `system` — follow OS appearance
- `light`
- `dark`

The selected value is stored as `gazeguard-theme` in `localStorage`.

## Palette

- Primary: `#306363`
- Light background: `#f8f9ff`
- Light card: `#ffffff`
- Light soft surface: `#eff4ff`
- Dark background: `#202124`
- Dark card: `#303134`
- Dark soft surface: `#282a2d`
- Text: `#121c2a`
- Dark text: `#f6f6f6`

Boolean controls use macOS-style toggle switches:

- Checked: primary teal
- Unchecked: neutral gray
- Thumb: white
- Keyboard focus: teal focus ring

## Interaction rules

- Closing the macOS settings window hides it; the app remains in the tray.
- `Cmd-Q` and tray Quit terminate the app.
- Test Break opens the separate break screen.
- Settings must remain usable offline; avoid CDN-only UI dependencies.
