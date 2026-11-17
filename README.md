# ultraviewer-mcp

A headless Rust MCP server that controls an authorized remote computer over an UltraViewer-compatible connection. It reuses the protocol core from MacViewer's Rust LinuxViewer port. It does not run Wine, a Windows executable, or a GUI.

This is an independent interoperability project. It is not made, endorsed, or supported by the UltraViewer developers. Only connect to computers you are authorized to access.

## Build

Requires Rust and Cargo:

```sh
cargo build --release
```

The binary is `target/release/ultraviewer-mcp`. Run with no arguments, it starts a newline-delimited JSON-RPC MCP server on stdin/stdout. Diagnostics never use stdout.

## The screen dictionary (`8bit.dat`)

UltraViewer compresses screen updates with a Zstandard dictionary that ships with UltraViewer itself, a 112,640-byte file named `8bit.dat`. That file belongs to UltraViewer and carries no redistribution license, so this repository does not include it and the binary does not embed it. You supply your own copy once.

**Option 1: fetch it from ultraviewer.net (macOS or Linux).** The script downloads the official UltraViewer installer, extracts only `app/8bit.dat` with `innoextract` (the installer is never run), and installs it:

```sh
brew install innoextract        # or: apt install innoextract
scripts/fetch-dictionary.sh     # add --binary /path/to/ultraviewer-mcp if it is not on PATH
```

If ultraviewer.net's certificate has lapsed, the script stops. `--allow-stale-tls` skips the certificate check for the download only. That is safe here because nothing downloaded is executed and the extracted file must still match the pinned SHA-256.

**Option 2: copy it from an installation.** Copy `8bit.dat` out of the UltraViewer install folder on a Windows computer you use. If MacViewer is installed on this Mac, nothing is needed: its copy at `/Applications/MacViewer.app/Contents/Resources/8bit.dat` is found automatically. Then install it:

```sh
ultraviewer-mcp install-dictionary /path/to/8bit.dat
ultraviewer-mcp check-dictionary
```

You are responsible for following UltraViewer's terms of use for the file you download.

`install-dictionary` refuses any file whose size or SHA-256 does not match the expected dictionary:

```
SHA-256 9aa45789d9ff9d329a623a685d57b67e33c777019de9ce1f273f21860e4bfe6b
```

It copies the verified file to `~/.config/ultraviewer-mcp/8bit.dat` (or `$XDG_CONFIG_HOME/ultraviewer-mcp/8bit.dat`).

**Lookup order** when the server starts a session:

1. `$ULTRAVIEWER_MCP_DICTIONARY`, if set. Only that file is tried, so a wrong path fails instead of falling back.
2. `~/.config/ultraviewer-mcp/8bit.dat`
3. `8bit.dat` next to the `ultraviewer-mcp` binary
4. On macOS, `/Applications/MacViewer.app/Contents/Resources/8bit.dat`

Every candidate is hash-checked before use. If none is valid, `connect` fails before any network traffic and says what to do, and `session_status` always reports the dictionary state. Do not commit `8bit.dat` to a fork or attach it to a release; `.gitignore` excludes it.

## MCP setup

Point your MCP client at the binary as a stdio command:

```json
{
  "mcpServers": {
    "ultraviewer": {
      "command": "/absolute/path/to/ultraviewer-mcp"
    }
  }
}
```

For Claude Code:

```sh
claude mcp add ultraviewer -- /absolute/path/to/ultraviewer-mcp
```

The server owns one outgoing session at a time. It does not save the partner password. Its own registration identity is stored with user-only permissions in `~/.config/ultraviewer-mcp/`. Set `ULTRAVIEWER_MCP_NAME` to change the display name sent to the partner.

## Tools

| Tool | Purpose |
| --- | --- |
| `session_status` | Phase, partner, screen size, control permission, dictionary state |
| `connect` | Connect with the partner's UltraViewer ID and current password |
| `disconnect` | End the session |
| `screenshot` | PNG of the remote screen, scaled to `max_width` |
| `click`, `move_mouse`, `drag`, `scroll` | Mouse input in screenshot pixel coordinates |
| `type_text` | Unicode text |
| `press_keys` | Shortcuts such as `ctrl+c`, `alt+f4`, `win+r`, `enter` |
| `remote_action` | `ctrl_alt_delete` or `lock_screen` |

## Computer-use flow

1. Call `connect` with `partner_id` and the current `password` shown on the authorized remote computer.
2. Call `session_status`, then `screenshot`.
3. Use screenshot pixel coordinates with the mouse tools.
4. Take another screenshot to check the result. Call `disconnect` when finished.

Input is refused if the remote side allows viewing only. Closing the MCP process or its stdin disconnects the session.

## Scope

Included: relay and direct-LAN connection setup, login, TLS validation, screen decoding, PNG screenshots, mouse and keyboard control, and session cleanup.

Not exposed as tools: local hosting, multiple sessions, monitor switching, chat, file transfer, clipboard, and audio.

## Source provenance

Protocol modules and their tests come from [DeanOpen/MacViewer](https://github.com/DeanOpen/MacViewer), `LinuxViewer/`, commit `f3b56758a0746c74fdacfee006578a9be75c66b3`, under the included MIT license. See `UPSTREAM_NOTICE.md`.

## Verify

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

The test suite passes without `8bit.dat`. With a local copy installed, one extra test also decodes a dictionary-compressed fixture.
