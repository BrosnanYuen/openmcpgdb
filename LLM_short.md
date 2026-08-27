# LLM Guide for `openmcpgdb` MCP Server

## 1. System Prompt

```text
You are a debugging agent using openmcpgdb MCP server.

Rules:
- Always use absolute paths. Call tools directly; never describe hypothetical calls.
- After each tool call, check `debugger_state` and `stop_reason`
  (e.g. "breakpoint 1", "watchpoint 2", "sigsegv", "exited").
- Local debugging loop:
  1) gdb_execute {executable_path}
  2) gdb_add_variable_list / gdb_add_breakpoint {location}
  3) gdb_run
  4) gdb_step/gdb_next/gdb_continue/gdb_finish/gdb_interrupt
  5) inspect: gdb_current_code, gdb_variable_list, gdb_full_backtrace,
     gdb_info_threads, gdb_info_regs, gdb_print, gdb_frame_info,
     gdb_examine_memory, gdb_disassemble
  6) on error/signal: collect diagnostics, then gdb_reset_back_to_not_attached
  7) gdb_quit
- Attach flow: gdb_attach {pid} ... gdb_detach
- Remote debugging loop:
  1) gdb_gdbserver {ip,port,pid}
  2) gdb_target_remote {ip,port}
  3) breakpoints / stepping loop as above
- On signals (sigsegv/sigabrt/etc): immediately call gdb_full_backtrace,
  gdb_current_code, gdb_variable_list, gdb_info_regs.
- Keep responses concise: call made, key findings, next action.
```

## 2. Quick Examples

**Local:**
```
gdb_execute {"executable_path":"/absolute/path/to/program"}
gdb_add_variable_list {"var":"robot->sim->robot_row"}
gdb_add_breakpoint {"location":"main.c:20"}
gdb_add_breakpoint {"location":"compute_pi","condition":"value > 3.0"}
gdb_watch {"expression":"counter"}
gdb_run {}
gdb_next {} / gdb_step {} / gdb_continue {} / gdb_finish {}
gdb_print {"expression":"*(int*)0x7fff0000"}
gdb_examine_memory {"address":"&counter","count":4,"format":"x","size":"w"}
gdb_disassemble {"address":"main"} / gdb_frame_info {}
gdb_disable_breakpoint {"target":"main.c:20"}   // or breakpoint number: "1"
gdb_quit {}
```

**Attach to a running process:**
```
gdb_attach {"pid":149104}
gdb_interrupt {} / gdb_continue {}
gdb_detach {}
```

**Remote:**
```
gdb_gdbserver {"ip":"127.0.0.1","port":11444,"pid":149104}
gdb_target_remote {"ip":"127.0.0.1","port":11444}
gdb_add_breakpoint {"location":"/absolute/path/src/main.c:20"}
gdb_continue {}
gdb_next {} / gdb_step {} / gdb_interrupt {}
gdb_quit {}
```

## 3. Tool Reference

All calls return `debugger_state`, optional `stop_reason`, plus optional:
`error`, `current_func`, `current_code_path`, `current_code_line`,
`current_code`, `backtrace`, `variable_list`, `breakpoints`, `memory`,
`command_output`.

### Session
| Tool | Args | When |
|------|------|------|
| `gdb_execute` | `executable_path` | Start session, attach to binary |
| `gdb_attach` | `pid` | Attach debugger to running process |
| `gdb_detach` | none | Detach, leave process running |
| `gdb_run` | none | Run loaded executable |
| `gdb_gdbserver` | `ip`, `port`, `pid` | Start gdbserver on running process |
| `gdb_target_remote` | `ip`, `port` | Connect to remote gdbserver |
| `gdb_set_thread` | `id` | Switch thread context |
| `gdb_set_frame` | `id` | Switch stack frame |
| `gdb_quit` | none | End session |
| `gdb_kill` | none | Kill process, keep debugger |
| `gdb_reset_back_to_not_attached` | none | Recover from error/signal state |

### Breakpoints & Watchpoints
Breakpoints accept any gdb location: `"file.c:55"`, `"function_name"`,
`"file.c:function"`, `"symbol+16"`, or `"*0x4005a0"` (memory address).
Clear/enable/disable accept a breakpoint **number** (from listing) or location.

| Tool | Args | When |
|------|------|------|
| `gdb_add_breakpoint` | `location`, `condition?` | Insert breakpoint; condition like `"count == 3"` |
| `gdb_clear_breakpoint` | `target` | Delete by number or location |
| `gdb_enable_breakpoint` | `target` | Enable disabled breakpoint |
| `gdb_disable_breakpoint` | `target` | Temporarily suppress breakpoint |
| `gdb_list_breakpoint` | none | Structured list: number/kind/enabled/detail |
| `gdb_watch` | `expression`, `mode?` | Stop when written (`write`, default), read (`read`), or either (`access`) |

### Stepping
| Tool | Args | When |
|------|------|------|
| `gdb_next` | none | Step over function calls |
| `gdb_step` | none | Step into function calls |
| `gdb_nexti` / `gdb_stepi` | none | Instruction-level step over/into |
| `gdb_finish` | none | Run until current function returns |
| `gdb_continue` | none | Resume until breakpoint/signal/exit |
| `gdb_interrupt` | none | Interrupt running program and stop at stepping |

### Variables & Memory
| Tool | Args | When |
|------|------|------|
| `gdb_add_variable_list` | `var` | Add to watch list |
| `gdb_del_variable_list` | `var` | Remove from watch list |
| `gdb_variable_list` | none | Get all watched variables |
| `gdb_print` | `expression` | Evaluate any expression: casts, derefs, arithmetic |
| `gdb_set_var` | `var`, `value` | Modify variable at runtime |
| `gdb_examine_memory` | `address`, `count?`, `format?`, `size?` | Raw memory dump; formats x/d/u/o/t/c/s/i, sizes b/h/w/g |

### Inspection
| Tool | Args | When |
|------|------|------|
| `gdb_debugger_state` | none | Get current debugger state |
| `gdb_current_code` | none | Source location + nearby code |
| `gdb_frame_info` | none | Arguments and locals of selected frame |
| `gdb_full_backtrace` | none | Full call stack |
| `gdb_info_threads` | none | Thread listing |
| `gdb_info_regs` | none | Register dump |
| `gdb_disassemble` | `address?` | Disassemble current function or around symbol/address |
| `gdb_custom` | `cmd` | Raw GDB command |

Display tuning lives in one tool: `gdb_set_display` with optional
`lines_before_current`, `lines_after_current`, `backtrace`, `variable_list`.

## 4. Debugger States

- `not attached` / `failed to attach` => call `gdb_execute` or `gdb_attach`
- `attached` => ready to run
- `running` => executing
- `stopped at breakpoint` / `stopped at stepping` => paused, inspect or continue;
  check `stop_reason` to see which breakpoint/watchpoint/interrupt caused it
- `sigsegv` / `sigabrt` / `sigbus` / `sigfpe` / `sigill` / `sigtrap` / `sigterm` / `sigkill` => crash; collect diagnostics immediately
- `exited` => process ended
- `error` => something failed; check `error` field
