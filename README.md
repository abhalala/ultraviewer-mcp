# ultraviewer-mcp

A headless Rust MCP server that controls an authorized remote computer over an UltraViewer-compatible connection. It reuses the protocol core from MacViewer's Rust LinuxViewer port. It does not run Wine, a Windows executable, or a GUI.

This is an independent interoperability project. It is not made, endorsed, or supported by the UltraViewer developers. Only connect to computers you are authorized to access.

## Build

Requires Rust and Cargo:

```sh
cargo build --release
```

The binary is `target/release/ultraviewer-mcp`. Run with no arguments, it starts a newline-delimited JSON-RPC MCP server on stdin/stdout. Diagnostics never use stdout.

## Screen dictionary

UltraViewer compresses screen updates with its own Zstandard dictionary. A copy of it, `assets/8bit.dat`, is embedded in the binary, so there is nothing to set up. It is UltraViewer's data and is not covered by the MIT license; see `THIRD_PARTY_NOTICES.md`.

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

## Agent skill

`skills/ultraviewer-computer-use/SKILL.md` teaches an agent the setup check and control loop. Install it with:

```sh
npx skills add abhalala/ultraviewer-mcp
```

## Tools

| Tool | Purpose |
| --- | --- |
| `session_status` | Phase, partner, screen size, control permission |
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

Protocol modules and their tests come from [DeanOpen/MacViewer](https://github.com/DeanOpen/MacViewer), `LinuxViewer/`, commit `f3b56758a0746c74fdacfee006578a9be75c66b3`, under the included MIT license. See `THIRD_PARTY_NOTICES.md` for the G.722, Zstandard and UltraViewer dictionary notices.

## Verify

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

