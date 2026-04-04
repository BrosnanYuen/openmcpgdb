# LLM Guide for `openmcpgdb` MCP Server

This file gives a ready-to-use prompt and concrete MCP tool-call examples for every `openmcpgdb_*` tool.

## 1. Copy-Paste LLM System Prompt

Use this as a system/developer prompt for any LLM connected to this MCP server:

```text
You are a debugging agent connected to the openmcpgdb MCP server.

Primary goal:
- Debug native programs with GDB through openmcpgdb MCP tools.

Rules:
- Always use absolute filesystem paths.
- Always call tools directly; do not describe hypothetical calls.
- After each tool call, read and use `debugger_state`.
- Prefer this flow:
  1) openmcpgdb_execute
  2) breakpoint setup
  3) openmcpgdb_run
  4) step/next/continue loops
  5) inspect state (current_code, variable_list, full_backtrace, info_threads, info_regs, print)
  6) stop session with openmcpgdb_quit
- When modifying values, use openmcpgdb_set_var (or openmcpgdb_print with value).
- If debugger_state is `not attached` or `failed to attach`, recover by calling openmcpgdb_execute again with the correct absolute executable path.
- If debugger_state indicates a signal (sigsegv/sigabrt/sigbus/sigfpe/sigill/sigtrap/sigterm/sigkill), immediately collect:
  - openmcpgdb_full_backtrace
  - openmcpgdb_current_code
  - openmcpgdb_variable_list
  - openmcpgdb_info_regs
- Keep responses concise and include:
  - what call was made
  - key findings from the returned payload
  - next debugging action
```

## 2. Debug Session Example on Local GDB

```text
openmcpgdb_execute {"executable_path":"/home/brosnan/openmcpgdb/openmcpgdb/examples/mazerobot/maze_robot"}
openmcpgdb_debugger_state {}
openmcpgdb_add_variable_list {"var":"robot_state"}
openmcpgdb_add_breakpoint {"filename":"/home/brosnan/openmcpgdb/openmcpgdb/examples/mazerobot/src/main.c","linenumber":20}
openmcpgdb_run {}
openmcpgdb_next {}
openmcpgdb_step {}
openmcpgdb_print {"var":"counter"}
openmcpgdb_full_backtrace {}
openmcpgdb_continue {}
openmcpgdb_quit {}
```

## 3. Tool Reference (All Tools)

All calls return a JSON payload with `debugger_state` and may include:
- `error`: verbose GDB/tool error text.
- `current_func`, `current_code_path`, `current_code_line`, `current_code`: execution location context.
- `backtrace`: frame map from deepest frame (`0`) upward.
- `variable_list`: watched variable values.

### Execution and Session

`openmcpgdb_execute`
- What it does: starts a new GDB session and attaches it to a local executable path.
- Arguments: `executable_path` (string, absolute path to binary).
- Call this when: debugger is `not attached`, after `quit`, or after wrong target attach.
- Expected response: `attached` on success; `failed to attach`/`error` with details on failure.
```text
openmcpgdb_execute {"executable_path":"/absolute/path/to/program"}
```

`openmcpgdb_run`
- What it does: runs the loaded executable in GDB (`run`).
- Arguments: none.
- Call this when: binary is loaded and you are ready to start execution from program entry.
- Expected response: often `running` or `stopped at breakpoint`; on crash may return signal states.
```text
openmcpgdb_run {}
```

`openmcpgdb_target_remote`
- What it does: connects GDB to a remote debug target (`target remote ip:port`).
- Arguments: `ip` (string), `port` (u16).
- Call this when: debugging with `gdbserver` or another remote endpoint.
- Expected response: `attached` on success; `error` if network/target setup fails.
```text
openmcpgdb_target_remote {"ip":"127.0.0.1","port":1234}
```

`openmcpgdb_set_thread`
- What it does: switches active thread context (`thread <id>`).
- Arguments: `id` (i64 thread id from GDB thread list).
- Call this when: analyzing multi-threaded behavior or thread-specific stack/locals.
- Expected response: updated context; verify with `openmcpgdb_info_threads`.
```text
openmcpgdb_set_thread {"id":1}
```

`openmcpgdb_set_frame`
- What it does: switches active stack frame (`frame <id>`).
- Arguments: `id` (i64 frame index, typically from backtrace).
- Call this when: inspecting caller variables or code path above the current frame.
- Expected response: current frame context updates for `current_code`, `print`, and watched vars.
```text
openmcpgdb_set_frame {"id":0}
```

### Breakpoints

`openmcpgdb_add_breakpoint`
- What it does: inserts a source breakpoint (`break file:line`).
- Arguments: `filename` (absolute path), `linenumber` (u64, 1-based source line).
- Call this when: defining stop points before `run` or while paused.
- Expected response: success info in output; use `openmcpgdb_list_breakpoint` to confirm.
```text
openmcpgdb_add_breakpoint {"filename":"/absolute/path/to/file.c","linenumber":20}
```

`openmcpgdb_clear_breakpoint`
- What it does: removes breakpoint at source location (`clear file:line`).
- Arguments: `filename`, `linenumber`.
- Call this when: a breakpoint is no longer needed or was placed incorrectly.
- Expected response: location removed or an error if no matching breakpoint exists.
```text
openmcpgdb_clear_breakpoint {"filename":"/absolute/path/to/file.c","linenumber":20}
```

`openmcpgdb_enable_breakpoint`
- What it does: enables a disabled breakpoint at location.
- Arguments: `filename`, `linenumber`.
- Call this when: restoring temporarily disabled breakpoints.
- Expected response: breakpoint state toggled to enabled.
```text
openmcpgdb_enable_breakpoint {"filename":"/absolute/path/to/file.c","linenumber":20}
```

`openmcpgdb_disable_breakpoint`
- What it does: disables (but does not remove) breakpoint at location.
- Arguments: `filename`, `linenumber`.
- Call this when: temporarily suppressing stops without losing breakpoint definitions.
- Expected response: breakpoint state toggled to disabled.
```text
openmcpgdb_disable_breakpoint {"filename":"/absolute/path/to/file.c","linenumber":20}
```

`openmcpgdb_list_breakpoint`
- What it does: lists all breakpoints (`info breakpoints`).
- Arguments: none.
- Call this when: validating current breakpoint set and enable/disable status.
- Expected response: GDB listing in payload text, with ids/locations/conditions.
```text
openmcpgdb_list_breakpoint {}
```

### Stepping Control

`openmcpgdb_next`
- What it does: executes the next source line, stepping over function calls.
- Arguments: none.
- Call this when: isolating logic in the current frame without entering callees.
- Expected response: usually `stopped at breakpoint`/paused with updated code context.
```text
openmcpgdb_next {}
```

`openmcpgdb_step`
- What it does: executes next line and steps into function calls.
- Arguments: none.
- Call this when: tracing into called function internals.
- Expected response: paused in callee or next line; inspect with `current_code`.
```text
openmcpgdb_step {}
```

`openmcpgdb_continue`
- What it does: resumes execution until next breakpoint, signal, or exit.
- Arguments: none.
- Call this when: you want to run to next interesting stop point.
- Expected response: `running`, `stopped at breakpoint`, signal state, or `exited`.
```text
openmcpgdb_continue {}
```

### Watch Variable List

`openmcpgdb_add_variable_list`
- What it does: adds expression/variable to watch list maintained by server session.
- Arguments: `var` (string expression, e.g. `obj->field`).
- Call this when: you want variable values returned in `variable_list`.
- Expected response: watch list updated; verify with `openmcpgdb_variable_list`.
```text
openmcpgdb_add_variable_list {"var":"robot_state"}
```

`openmcpgdb_del_variable_list`
- What it does: removes expression/variable from watch list.
- Arguments: `var` (string).
- Call this when: watched item is noisy/no longer relevant.
- Expected response: watch list entry removed.
```text
openmcpgdb_del_variable_list {"var":"robot_state"}
```

`openmcpgdb_variable_list`
- What it does: evaluates and returns all watched variables in current context.
- Arguments: none.
- Call this when: stopped at a location and you need a compact state snapshot.
- Expected response: `variable_list` map limited by `display_variable_list` config.
```text
openmcpgdb_variable_list {}
```

### Inspection

`openmcpgdb_debugger_state`
- What it does: returns only/primarily the current debugger state.
- Arguments: none.
- Call this when: gating next action (`run`, `continue`, diagnostics, or `execute`).
- Expected response: one of the documented state values plus optional `error`.
```text
openmcpgdb_debugger_state {}
```

`openmcpgdb_current_code`
- What it does: returns source file, line, function, and nearby code window.
- Arguments: none.
- Call this when: you need exact program counter location and nearby context.
- Expected response: `current_code_path`, `current_code_line`, and `current_code`.
```text
openmcpgdb_current_code {}
```

`openmcpgdb_full_backtrace`
- What it does: runs full backtrace and returns frame stack.
- Arguments: none.
- Call this when: root-causing crashes or understanding call chain.
- Expected response: `backtrace` map and current function context.
```text
openmcpgdb_full_backtrace {}
```

`openmcpgdb_info_threads`
- What it does: queries thread listing/details (`info threads`).
- Arguments: none.
- Call this when: thread-aware debugging, deadlock, races, or selecting target thread.
- Expected response: thread information text for inspection.
```text
openmcpgdb_info_threads {}
```

`openmcpgdb_info_regs`
- What it does: queries register dump (`info all-registers`).
- Arguments: none.
- Call this when: low-level crashes, ABI issues, corrupted pointers, asm-level checks.
- Expected response: register values in output text.
```text
openmcpgdb_info_regs {}
```

`openmcpgdb_print` (read expression)
- What it does: evaluates an expression (`print <var>`).
- Arguments: `var` (required), `value` omitted.
- Call this when: one-off variable/expression evaluation is needed.
- Expected response: printed expression value in output text.
```text
openmcpgdb_print {"var":"counter"}
```

`openmcpgdb_set_var` (preferred set call)
- What it does: sets runtime variable (`set variable <var> = <value>` semantics).
- Arguments: `var` (string expression target), `value` (string literal/expression).
- Call this when: changing program state intentionally during diagnosis.
- Expected response: mutation result; verify using `openmcpgdb_print`.
```text
openmcpgdb_set_var {"var":"counter","value":"42"}
```

### Control and Server Display Tuning

`openmcpgdb_quit`
- What it does: quits/detaches GDB and ends active debug session.
- Arguments: none.
- Call this when: debug session is complete and resources should be released.
- Expected response: detached/not attached style state.
```text
openmcpgdb_quit {}
```

`openmcpgdb_kill`
- What it does: kills the debugged process (`kill`) but keeps GDB session.
- Arguments: none.
- Call this when: stopping runaway/bad process while keeping debugger context alive.
- Expected response: terminated state; may still remain attached to debugger shell.
```text
openmcpgdb_kill {}
```

`openmcpgdb_display_lines_before_current`
- What it does: sets number of source lines shown before current line in `current_code`.
- Arguments: `size` (usize).
- Call this when: expanding/reducing pre-context around instruction pointer.
- Expected response: config update acknowledged in session response.
```text
openmcpgdb_display_lines_before_current {"size":7}
```

`openmcpgdb_display_lines_after_current`
- What it does: sets number of source lines shown after current line in `current_code`.
- Arguments: `size` (usize).
- Call this when: expanding/reducing post-context around instruction pointer.
- Expected response: config update acknowledged in session response.
```text
openmcpgdb_display_lines_after_current {"size":8}
```

`openmcpgdb_display_backtrace`
- What it does: sets max number of backtrace frames returned in summarized outputs.
- Arguments: `size` (usize).
- Call this when: reducing noise or broadening call-chain visibility.
- Expected response: config update acknowledged in session response.
```text
openmcpgdb_display_backtrace {"size":6}
```

`openmcpgdb_display_variable_list`
- What it does: sets max number of watched variables returned in `variable_list`.
- Arguments: `size` (usize).
- Call this when: tuning response size for token budget/readability.
- Expected response: config update acknowledged in session response.
```text
openmcpgdb_display_variable_list {"size":9}
```

`openmcpgdb_custom`
- What it does: runs a raw custom GDB command string.
- Arguments: `cmd` (string, exact GDB command).
- Call this when: using advanced GDB features not covered by dedicated tools.
- Expected response: raw command output; treat as expert/unsafe escape hatch.
```text
openmcpgdb_custom {"cmd":"info locals"}
```

## 4. Recommended LLM Operating Loop

Use this loop repeatedly:

1. Ensure debugger is attached (`openmcpgdb_execute` if needed).
2. Install breakpoints.
3. Start/resume execution (`openmcpgdb_run` / `openmcpgdb_continue`).
4. On stop/crash, collect:
   - `openmcpgdb_current_code`
   - `openmcpgdb_full_backtrace`
   - `openmcpgdb_variable_list`
   - `openmcpgdb_info_regs` (for low-level failures)
5. Use `openmcpgdb_step`/`openmcpgdb_next` for root-cause isolation.
6. Optionally change runtime variables (`openmcpgdb_set_var`).
7. End with `openmcpgdb_quit`.

## 5. Debugger State Values to Handle

The LLM should branch behavior based on `debugger_state`, including:

- `not attached`
- `failed to attach`
- `attached`
- `stopped at breakpoint`
- `running`
- `sigsegv`
- `sigabrt`
- `sigbus`
- `sigfpe`
- `sigill`
- `sigtrap`
- `sigterm`
- `sigkill`
- `exited`
- `error`

For any signal or `error`, capture diagnostics before ending the session.
