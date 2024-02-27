# gbfr-logs

[![GitHub Release](https://img.shields.io/github/v/release/false-spring/gbfr-logs)](https://github.com/false-spring/gbfr-logs/releases)

Experimental DPS meter for Granblue Fantasy: Relink, based on the reverse engineering work from [naoouo/GBFR-ACT](https://github.com/nyaoouo/GBFR-ACT).

## Screenshots

![Meter](./docs/screenshots/meter.png)

## How to install

- Go to [Releases](https://github.com/false-spring/gbfr-logs/releases)
- Run the installer.
- Open GBFR Logs after the game is already running.

## Known Issues

- DoT skills do not count towards damage.

## Troubleshooting

> Q: The meter isn't updating or displaying anything.

Try running the program after the game has been launched.

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

Useful functionality:

- Track player index in party (for distinguising between duplicate charas)
- Track damage per target
- Provide translations for skills
- Skill tracking, min/max damage.
- DPS charting
- Historical logs

Improvements:

- Multiple language support (can pull some from translation files, but skill names are manual)
- Configuration / Settings

Reverse Engineering:

- Figure out if we can fetch party data upon area enter, would make it easier to cache it then.
- Flags for damage cap tracking, if they exist.
- Buff tracking
