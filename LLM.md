# LLM Guide for `openmcpgdb` MCP Server

This file gives a ready-to-use prompt and concrete MCP tool-call examples for every `gdb_*` tool.

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
- Use this loop repeatedly for local debugging:
  1) gdb_execute            local executable
  2) gdb_add_variable_list  watches variable
  3) gdb_add_breakpoint     breakpoint setup
  3) gdb_run                only use this for local executable
  4) gdb_step/gdb_next/gdb_continue/gdb_interrupt loops
  5) inspect state (gdb_current_code, gdb_variable_list, gdb_full_backtrace, gdb_info_threads, gdb_info_regs, gdb_print)
  6) if session is in `error`/signal state and you need a clean slate, call gdb_reset_back_to_not_attached
  7) stop session with gdb_quit
- Use this loop repeatedly for remote debugging:
  1) gdb_gdbserver          starts gdbserver
  2) gdb_target_remote      attach to gdbserver via ip:port
  3) gdb_add_variable_list  watches variable
  4) gdb_add_breakpoint     breakpoint setup
  5) gdb_continue           resume running
  6) gdb_step/gdb_next/gdb_continue loops same as above
- When modifying values, use gdb_set_var (or gdb_print with value).
- If debugger_state is `not attached` or `failed to attach`, recover by calling gdb_execute again with the correct absolute executable path.
- If debugger_state indicates a signal (sigsegv/sigabrt/sigbus/sigfpe/sigill/sigtrap/sigterm/sigkill), immediately collect:
  - gdb_full_backtrace
  - gdb_current_code
  - gdb_variable_list
  - gdb_info_regs
- Keep responses concise and include:
  - what call was made
  - key findings from the returned payload
  - next debugging action
```

## 2. Debug Session Example 

Examples of Debugging on  Local GDB:
```text
gdb_execute {"executable_path":"/home/brosnan/openmcpgdb/openmcpgdb/examples/mazerobot/maze_robot"}
gdb_debugger_state {}
gdb_add_variable_list {"var":"robot_state"}
gdb_add_breakpoint {"filename":"/home/brosnan/openmcpgdb/openmcpgdb/examples/mazerobot/src/main.c","linenumber":20}
gdb_run {}
gdb_next {}
gdb_step {}
gdb_interrupt {}
gdb_print {"var":"counter"}
gdb_full_backtrace {}
gdb_continue {}
gdb_quit {}
gdb_reset_back_to_not_attached {}
```

Examples of Starting a remote gdbserver and attaching to it and debugging
```
gdb_gdbserver {"ip":"127.0.0.1","port":11444,"pid":149104}
gdb_target_remote {"ip":"127.0.0.1","port":11444}
gdb_debugger_state {}
gdb_add_variable_list {"var":"robot_state"}
gdb_add_breakpoint {"filename":"/home/brosnan/openmcpgdb/openmcpgdb/examples/mazerobot/src/main.c","linenumber":20}
gdb_continue {}
gdb_next {}
gdb_step {}
gdb_interrupt {}
gdb_print {"var":"counter"}
gdb_full_backtrace {}
gdb_quit {}
gdb_reset_back_to_not_attached {}
```

## 3. Tool Reference (All Tools)

All calls return a JSON payload with `debugger_state` and may include:
- `error`: verbose GDB/tool error text.
- `current_func`, `current_code_path`, `current_code_line`, `current_code`: execution location context.
- `backtrace`: frame map from deepest frame (`0`) upward.
- `variable_list`: watched variable values.

### Execution and Session

`gdb_execute`
- What it does: starts a new GDB session and attaches it to a local executable path.
- Arguments: `executable_path` (string, absolute path to binary).
- Call this when: debugger is `not attached`, after `quit`, or after wrong target attach.
- Expected response: `attached` on success; `failed to attach`/`error` with details on failure.
```text
gdb_execute {"executable_path":"/absolute/path/to/program"}
```

`gdb_run`
- What it does: runs the loaded executable in GDB (`run`).
- Arguments: none.
- Call this when: binary is loaded and you are ready to start execution from program entry.
- Expected response: often `running` or `stopped at breakpoint`; on crash may return signal states.
```text
gdb_run {}
```

`gdb_gdbserver`
- What it does: starts `gdbserver --attach <ip>:<port> <pid>` from the MCP server process.
- Arguments: `ip` (string), `port` (u16), `pid` (i64, must be > 0).
- Call this when: attaching gdbserver to a running process before connecting with `gdb_target_remote`.
- Expected response: `gdbserver attached` on success; `failed to attach` with error details otherwise.
```text
gdb_gdbserver {"ip":"127.0.0.1","port":1234,"pid":12345}
```

`gdb_target_remote`
- What it does: connects GDB to a remote debug target (`target remote ip:port`).
- Arguments: `ip` (string), `port` (u16).
- Call this when: debugging with `gdbserver` or another remote endpoint.
- Expected response: `attached` on success; `error` if network/target setup fails.
```text
gdb_target_remote {"ip":"127.0.0.1","port":1234}
```

`gdb_set_thread`
- What it does: switches active thread context (`thread <id>`).
- Arguments: `id` (i64 thread id from GDB thread list).
- Call this when: analyzing multi-threaded behavior or thread-specific stack/locals.
- Expected response: updated context; verify with `gdb_info_threads`.
```text
gdb_set_thread {"id":1}
```

`gdb_set_frame`
- What it does: switches active stack frame (`frame <id>`).
- Arguments: `id` (i64 frame index, typically from backtrace).
- Call this when: inspecting caller variables or code path above the current frame.
- Expected response: current frame context updates for `current_code`, `print`, and watched vars.
```text
gdb_set_frame {"id":0}
```

### Breakpoints

`gdb_add_breakpoint`
- What it does: inserts a source breakpoint (`break file:line`).
- Arguments: `filename` (absolute path), `linenumber` (u64, 1-based source line).
- Call this when: defining stop points before `run` or while paused.
- Expected response: success info in output; use `gdb_list_breakpoint` to confirm.
```text
gdb_add_breakpoint {"filename":"/absolute/path/to/file.c","linenumber":20}
```

`gdb_clear_breakpoint`
- What it does: removes breakpoint at source location (`clear file:line`).
- Arguments: `filename`, `linenumber`.
- Call this when: a breakpoint is no longer needed or was placed incorrectly.
- Expected response: location removed or an error if no matching breakpoint exists.
```text
gdb_clear_breakpoint {"filename":"/absolute/path/to/file.c","linenumber":20}
```

`gdb_enable_breakpoint`
- What it does: enables a disabled breakpoint at location.
- Arguments: `filename`, `linenumber`.
- Call this when: restoring temporarily disabled breakpoints.
- Expected response: breakpoint state toggled to enabled.
```text
gdb_enable_breakpoint {"filename":"/absolute/path/to/file.c","linenumber":20}
```

`gdb_disable_breakpoint`
- What it does: disables (but does not remove) breakpoint at location.
- Arguments: `filename`, `linenumber`.
- Call this when: temporarily suppressing stops without losing breakpoint definitions.
- Expected response: breakpoint state toggled to disabled.
```text
gdb_disable_breakpoint {"filename":"/absolute/path/to/file.c","linenumber":20}
```

`gdb_list_breakpoint`
- What it does: lists all breakpoints (`info breakpoints`).
- Arguments: none.
- Call this when: validating current breakpoint set and enable/disable status.
- Expected response: GDB listing in payload text, with ids/locations/conditions.
```text
gdb_list_breakpoint {}
```

### Stepping Control

`gdb_next`
- What it does: executes the next source line, stepping over function calls.
- Arguments: none.
- Call this when: isolating logic in the current frame without entering callees.
- Expected response: `stopped at stepping` when stepping through code without hitting a breakpoint, or `stopped at breakpoint` if a breakpoint was encountered.
```text
gdb_next {}
```

`gdb_step`
- What it does: executes next line and steps into function calls.
- Arguments: none.
- Call this when: tracing into called function internals.
- Expected response: `stopped at stepping` when stepping through code without hitting a breakpoint, or `stopped at breakpoint` if a breakpoint was encountered.
```text
gdb_step {}
```

`gdb_step`
- What it does: executes next line and steps into function calls.
- Arguments: none.
- Call this when: tracing into called function internals.
- Expected response: `stopped at stepping` when stepping through code without hitting a breakpoint, or `stopped at breakpoint` if a breakpoint was encountered.
```text
gdb_step {}
```

`gdb_continue`
- What it does: resumes execution until next breakpoint, signal, or exit.
- Arguments: none.
- Call this when: you want to run to next interesting stop point.
- Expected response: `running`, `stopped at breakpoint`, signal state, or `exited`.
```text
gdb_continue {}
```

`gdb_interrupt`
- What it does: interrupts a running debuggee and transitions debugger state to `stopped at stepping`.
- Arguments: none.
- Call this when: execution is `running` and you need to pause immediately for inspection.
- Expected response: normal full debugger response with `variable_list`, `backtrace`, `current_func`, and current source context.
```text
gdb_interrupt {}
```

### Watch Variable List

`gdb_add_variable_list`
- What it does: adds expression/variable to watch list maintained by server session.
- Arguments: `var` (string expression, e.g. `obj->field`).
- Call this when: you want variable values returned in `variable_list`.
- Expected response: watch list updated; verify with `gdb_variable_list`.
```text
gdb_add_variable_list {"var":"robot_state"}
```

`gdb_del_variable_list`
- What it does: removes expression/variable from watch list.
- Arguments: `var` (string).
- Call this when: watched item is noisy/no longer relevant.
- Expected response: watch list entry removed.
```text
gdb_del_variable_list {"var":"robot_state"}
```

`gdb_variable_list`
- What it does: evaluates and returns all watched variables in current context.
- Arguments: none.
- Call this when: stopped at a location and you need a compact state snapshot.
- Expected response: `variable_list` map limited by `display_variable_list` config.
```text
gdb_variable_list {}
```

### Inspection

`gdb_debugger_state`
- What it does: returns only/primarily the current debugger state.
- Arguments: none.
- Call this when: gating next action (`run`, `continue`, diagnostics, or `execute`).
- Expected response: one of the documented state values plus optional `error`.
```text
gdb_debugger_state {}
```

`gdb_current_code`
- What it does: returns source file, line, function, and nearby code window.
- Arguments: none.
- Call this when: you need exact program counter location and nearby context.
- Expected response: `current_code_path`, `current_code_line`, and `current_code`.
```text
gdb_current_code {}
```

`gdb_full_backtrace`
- What it does: runs full backtrace and returns frame stack.
- Arguments: none.
- Call this when: root-causing crashes or understanding call chain.
- Expected response: `backtrace` map and current function context.
```text
gdb_full_backtrace {}
```

`gdb_info_threads`
- What it does: queries thread listing/details (`info threads`).
- Arguments: none.
- Call this when: thread-aware debugging, deadlock, races, or selecting target thread.
- Expected response: thread information text for inspection.
```text
gdb_info_threads {}
```

`gdb_info_regs`
- What it does: queries register dump (`info all-registers`).
- Arguments: none.
- Call this when: low-level crashes, ABI issues, corrupted pointers, asm-level checks.
- Expected response: register values in output text.
```text
gdb_info_regs {}
```

`gdb_print` (read expression)
- What it does: evaluates an expression (`print <var>`).
- Arguments: `var` (required), `value` omitted.
- Call this when: one-off variable/expression evaluation is needed.
- Expected response: printed expression value in output text.
```text
gdb_print {"var":"counter"}
```

`gdb_set_var` (preferred set call)
- What it does: sets runtime variable (`set variable <var> = <value>` semantics).
- Arguments: `var` (string expression target), `value` (string literal/expression).
- Call this when: changing program state intentionally during diagnosis.
- Expected response: mutation result; verify using `gdb_print`.
```text
gdb_set_var {"var":"counter","value":"42"}
```

### Control and Server Display Tuning

`gdb_quit`
- What it does: quits/detaches GDB and ends active debug session.
- Arguments: none.
- Call this when: debug session is complete and resources should be released.
- Expected response: detached/not attached style state.
```text
gdb_quit {}
```

`gdb_kill`
- What it does: kills the debugged process (`kill`) but keeps GDB session.
- Arguments: none.
- Call this when: stopping runaway/bad process while keeping debugger context alive.
- Expected response: terminated state; may still remain attached to debugger shell.
```text
gdb_kill {}
```

`gdb_reset_back_to_not_attached`
- What it does: force-clears session error context and resets debugger state to `not attached`.
- Arguments: none.
- Call this when: you want to recover from a bad/error/signal state and start a fresh attach cycle.
- Expected response: `not attached` with cleared `error`.
```text
gdb_reset_back_to_not_attached {}
```

`gdb_display_lines_before_current`
- What it does: sets number of source lines shown before current line in `current_code`.
- Arguments: `size` (usize).
- Call this when: expanding/reducing pre-context around instruction pointer.
- Expected response: config update acknowledged in session response.
```text
gdb_display_lines_before_current {"size":7}
```

`gdb_display_lines_after_current`
- What it does: sets number of source lines shown after current line in `current_code`.
- Arguments: `size` (usize).
- Call this when: expanding/reducing post-context around instruction pointer.
- Expected response: config update acknowledged in session response.
```text
gdb_display_lines_after_current {"size":8}
```

`gdb_display_backtrace`
- What it does: sets max number of backtrace frames returned in summarized outputs.
- Arguments: `size` (usize).
- Call this when: reducing noise or broadening call-chain visibility.
- Expected response: config update acknowledged in session response.
```text
gdb_display_backtrace {"size":6}
```

`gdb_display_variable_list`
- What it does: sets max number of watched variables returned in `variable_list`.
- Arguments: `size` (usize).
- Call this when: tuning response size for token budget/readability.
- Expected response: config update acknowledged in session response.
```text
gdb_display_variable_list {"size":9}
```

`gdb_custom`
- What it does: runs a raw custom GDB command string.
- Arguments: `cmd` (string, exact GDB command).
- Call this when: using advanced GDB features not covered by dedicated tools.
- Expected response: raw command output; treat as expert/unsafe escape hatch.
```text
gdb_custom {"cmd":"info locals"}
```

## 4. Debugger State Values to Handle

The LLM should branch behavior based on `debugger_state`, including:

- `not attached` => gdb hasn't attached to any executable yet
- `failed to attach` => gdb failed to attach to executable
- `gdbserver failed to attach` => gdbserver failed to attach to executable
- `gdbserver attached` => gdbserver successfully attached
- `attached` => after gdb attached
- `stopped at breakpoint` => gdb ONLY stopped at defined breakpoint in code
- `stopped at stepping` => gdb ONLY stopped when stepping through the code and there is no breakpoints at the line
- `running` => gdb is running the code without stopping
- `sigsegv` => executable has segment fault.
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
