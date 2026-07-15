# ASUS Keyboard Backlight Enforcer

A small native Windows utility that controls an ASUS keyboard backlight through
the `AsusAtkWmi_WMNB` provider in `root\\WMI`. It can set the brightness once,
or run as a Windows service that continually restores the configured level.

The implementation currently targets the verified ASUS device ID `0x00050021`.
It does not synthesize Fn-key input and does not require MyASUS.

## Prerequisites

- Windows 11
- ASUS System Control Interface v3
- An ASUS model exposing `AsusAtkWmi_WMNB` in `root\\WMI`

## Build

```powershell
cargo build --release
```

The executable is `target\\release\\asus-kbdlight.exe`.

The release build statically links the Microsoft C runtime. The target laptop
does not need Rust, Cargo, or a separately installed Visual C++ Redistributable.

## One-file installer

With Inno Setup 6 installed on the build machine:

```powershell
.\\build-installer.cmd
```

The resulting `dist\\AsusKbdLightSetup-0.1.6-x64.exe` is the only file that
needs to be copied to the laptop. Setup requests elevation once, installs and
starts the boot service, and registers the tray icon for login. Normal startup,
resume, enforcement, and tray-level changes do not display elevation prompts.

The installer is not code-signed, so Windows SmartScreen may require **More
info** followed by **Run anyway** on the first installation. A commercial code-
signing certificate would be needed to remove that reputation warning.

For the remote laptop:

1. Copy the one setup executable over RustDesk.
2. Run it and approve the installer/UAC prompts.
3. Leave **Start the keyboard backlight tray icon** selected on the final page.
4. Right-click the blue keyboard tray icon and select **High**. Select
   **Status...** to see the desired setting, service state, firmware state, and
   raw ASUS status value.
5. Inspect `C:\\ProgramData\\AsusKbdLight\\events.jsonl` if the light changes
   unexpectedly; the service will log the observed raw state and correction.

## Direct use

```powershell
asus-kbdlight status
asus-kbdlight status --json
asus-kbdlight set off
asus-kbdlight set low
asus-kbdlight set medium
asus-kbdlight set high
```

`on` is an alias for `high`; numeric levels `0` through `3` are also accepted.

The initial ASUS control mapping is:

| Setting | `Control_status` |
| --- | ---: |
| off | `0x80` |
| low | `0x81` |
| medium | `0x82` |
| high | `0x83` |

Only `0x83` has been verified on the target laptop so far. Validate the other
three levels before relying on them.

## Foreground enforcement and diagnostics

Before installing the service, test the loop interactively:

```powershell
asus-kbdlight set high
asus-kbdlight watch
```

The monitor prints JSON Lines records whenever it observes a state transition or
performs a corrective write. Press Ctrl+C to stop it.

The WMI provider reports state but not the process responsible for changing it.
The service therefore logs firmware state transitions and nearby Windows power
and session notifications for correlation; it cannot conclusively attribute a
change to a process using this firmware interface alone.

## Boot service

From an Administrator terminal, run the release executable from its permanent
location:

```powershell
asus-kbdlight service install
```

The service starts automatically, runs as LocalSystem, and restarts after a
failure. Other commands are:

```powershell
asus-kbdlight service stop
asus-kbdlight service start
asus-kbdlight service uninstall
```

Show the persistent paths with:

```powershell
asus-kbdlight paths
```

By default they are:

- Configuration: `C:\\ProgramData\\AsusKbdLight\\config\\config.json`
- Event log: `C:\\ProgramData\\AsusKbdLight\\events.jsonl`

The event log is bounded. When it exceeds 5 MiB, the service keeps the newest
roughly 4 MiB beginning at a complete JSON record. This uses hysteresis so
compaction is occasional rather than happening on every log write.

The installer grants ordinary local users permission to update only the config
directory so the tray app can change the desired level without elevation.

## Events and sleep/resume

The service immediately rechecks the firmware after Windows sends any of these
notifications:

- suspend/resume and other power events
- session logon, logoff, lock, unlock, and console connect/disconnect
- hardware-profile changes
- service parameter-change and continue events
- startup and periodic polling (30 seconds by default)

It accepts stop, preshutdown, and shutdown notifications and exits cleanly. If a
vendor component changes the light without emitting an event, polling detects
and corrects the change within the configured interval.
