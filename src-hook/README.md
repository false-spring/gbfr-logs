# gbfr-logs-hook

This contains the library that is injected into the running game process.

It is responsible for:

- Initializes hooks into functions related to damage calculation and actor information.
- Sets up a named pipe broadcast server on `\\.\pipe\gbfr-logs` for other applications (like the DPS Meter) to listen for events.
