# openmcpgdb

`openmcpgdb` is an asynchronous Rust MCP server for debugging native programs through GDB.
It is implemented with the `rmcp` crate and exposes a full `openmcpgdb_*` tool API for MCP clients (Codex, Claude Code, opencode-compatible clients).

## Features

- MCP server built with `rmcp` (`tools/list`, `tools/call` support).
- One dedicated worker thread per MCP client session.
- GDB process isolation per client.
- Configurable display windows for source, backtrace, and watched variables.
- Transport modes:
  - `https://...` and `http://...` streamable HTTP mode (default)
  - `stdio://...` for stdio MCP wiring when needed
- Interactive MCP test client binary included.
- Rust tests include:
  - in-process MCP server/client connectivity
  - tool-call coverage
  - integration test against `examples/mazerobot/maze_robot`

## Requirements

- Rust toolchain (stable)
- GDB at an absolute path (default `/usr/bin/gdb`)
- Linux/macOS environment for the provided examples

## Project Layout

- `src/main.rs`: server entrypoint
- `src/runtime.rs`: transport/runtime bootstrapping
- `src/server.rs`: MCP tool routing and handlers
- `src/session.rs`: per-client worker thread and operation execution
- `src/gdb.rs`: real and mock GDB backends
- `src/bin/interactive_client.rs`: interactive MCP client
- `tests/mazerobot_mcp_client.rs`: integration MCP client test against mazerobot
- `config.json`: default config template

## Configuration

The server loads JSON config from the first CLI argument, defaulting to `config.json` in project root.

All filesystem paths must be absolute.

Example:

```json
{
  "gdb_path": "/usr/bin/gdb",
  "gdb_options": "",
  "codebase_dir": "/home/brosnan/openmcpgdb/openmcpgdb/examples/mazerobot",
  "executable_path": "/home/brosnan/openmcpgdb/openmcpgdb/examples/mazerobot/maze_robot",
  "mcp_server_name": "MCP GDB Server",
  "mcp_server_url": "https://localhost:9443",
  "display_lines_before_current": 7,
  "display_lines_after_current": 8,
  "display_backtrace": 6,
  "display_variable_list": 9
}
```

### `mcp_server_url`

- Default: `https://localhost:9443`
- `http://host:port/path` or `https://host:port/path`: run streamable HTTP on that bind address/path.
- `stdio://...`: run MCP over stdio.

Note: `https://` is parsed and accepted, but this project currently binds plain TCP HTTP directly. For real TLS, run behind a TLS-terminating reverse proxy.

## Build

```bash
cargo build
```

## Run the MCP Server

### 1. Default HTTPS URL mode

Use default config:

```bash
cargo run -- config.json
```

### 2. HTTP/HTTPS custom URL mode

Set config URL, for example:

```json
"mcp_server_url": "https://localhost:9443"
```

Run:

```bash
cargo run -- config.json
```

### 3. Optional stdio mode

Set:

```json
"mcp_server_url": "stdio://local"
```

Run:

```bash
cargo run -- config.json
```

## Interactive MCP Client

An interactive MCP client is included for manual testing against HTTP server mode.

Run:

```bash
cargo run --bin interactive_client -- https://localhost:9443
```

Input format:

```text
<tool_name> <json-args>
```

Examples:

```text
openmcpgdb_execute {"executable_path":"/home/brosnan/openmcpgdb/openmcpgdb/examples/mazerobot/maze_robot"}
openmcpgdb_debugger_state {}
openmcpgdb_add_breakpoint {"filename":"/home/brosnan/openmcpgdb/openmcpgdb/examples/mazerobot/src/main.c","linenumber":20}
openmcpgdb_run {}
openmcpgdb_next {}
openmcpgdb_full_backtrace {}
openmcpgdb_quit {}
```

Type `quit` to exit the interactive client.

## Tool API

All tool responses include `debugger_state` and optional fields like `variable_list`, `backtrace`, `current_code*`, and `error`.

### Execution/session

- `openmcpgdb_execute(executable_path)`
- `openmcpgdb_run()`
- `openmcpgdb_target_remote(ip, port)`
- `openmcpgdb_set_thread(id)`
- `openmcpgdb_set_frame(id)`

### Breakpoints

- `openmcpgdb_add_breakpoint(filename, linenumber)`
- `openmcpgdb_clear_breakpoint(filename, linenumber)`
- `openmcpgdb_enable_breakpoint(filename, linenumber)`
- `openmcpgdb_disable_breakpoint(filename, linenumber)`
- `openmcpgdb_list_breakpoint()`

### Stepping

- `openmcpgdb_next()`
- `openmcpgdb_step()`
- `openmcpgdb_continue()`

### Variable watch list

- `openmcpgdb_add_variable_list(var)`
- `openmcpgdb_del_variable_list(var)`
- `openmcpgdb_variable_list()`

### Inspection

- `openmcpgdb_debugger_state()`
- `openmcpgdb_current_code()`
- `openmcpgdb_full_backtrace()`
- `openmcpgdb_info_threads()`
- `openmcpgdb_info_regs()`
- `openmcpgdb_print(var)`
- `openmcpgdb_print(var, value)`

### Control/config/custom

- `openmcpgdb_quit()`
- `openmcpgdb_kill()`
- `openmcpgdb_display_lines_before_current(size)`
- `openmcpgdb_display_lines_after_current(size)`
- `openmcpgdb_display_backtrace(size)`
- `openmcpgdb_display_variable_list(size)`
- `openmcpgdb_custom(cmd)`

## Testing

Run all tests:

```bash
cargo test
```

Included test coverage:

- MCP server starts and MCP client connects.
- Unit-style all-tool-call response checks.
- Integration test starting MCP server with:
  - codebase: `/home/brosnan/openmcpgdb/openmcpgdb/examples/mazerobot/`
  - binary: `/home/brosnan/openmcpgdb/openmcpgdb/examples/mazerobot/maze_robot`
  - MCP client in `tests/mazerobot_mcp_client.rs`

## Quick Start (Mazerobot)

1. Ensure binary exists:

```bash
ls -l /home/brosnan/openmcpgdb/openmcpgdb/examples/mazerobot/maze_robot
```

2. Set `config.json` for mazerobot absolute paths.
3. Start server:

```bash
cargo run -- config.json
```

4. In another terminal (HTTP mode), run interactive client and call tools.
