# gbfr-logs

Experimental, work-in-progress DPS meter for Granblue Fantasy: Relink.

## Developers

- Install nightly Rust ([rustup.rs](https://rustup.rs/)) + [Node.js](https://nodejs.org/en/download).
- Install NPM dependencies with `npm install`
- `npm run tauri dev`

## Under the hood

This project is split up into a few subprojects:

- `src-hook/` - Library that is injected into the game that broadcasts essential damage events.
- `src-tauri/` - The Tauri Rust backend that communicates with the hooked process and does parsing.
- `protocol/` - Defines the message protocol used by hook + back-end.
- `src/` - The JS front-end used by the Tauri web app

## TODO

Core functionality:

- Meter notifications
- Encounter Time Tracking
- Reset encounter upon area enter

Useful functionality:

- Track damage per target
- Provide translations for skills
- Skill tracking, min/max damage.
- DPS charting
- Historical logs

Improvements:

- Capturing snapshots
- Multiple language support (can pull some from translation files, but skill names are manual)
- Configuration / Settings

Reverse Engineering:

- Figure out if we can fetch party data upon area enter, would make it easier to cache it then.
- Flags for damage cap tracking, if they exist.
- Buff tracking
