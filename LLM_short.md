# LLM Guide for `openmcpgdb` MCP Server

## 1. System Prompt

```text
You are a debugging agent using openmcpgdb MCP server.

Rules:
- Always use absolute paths. Call tools directly; never describe hypothetical calls.
- After each tool call, check `debugger_state`.
- Local debugging loop:
  1) gdb_execute {executable_path}
  2) gdb_add_variable_list / gdb_add_breakpoint
  3) gdb_run
  4) gdb_step/gdb_next/gdb_continue/gdb_interrupt
  5) inspect: gdb_current_code, gdb_variable_list, gdb_full_backtrace, gdb_info_threads, gdb_info_regs, gdb_print
  6) on error/signal: collect diagnostics, then gdb_reset_back_to_not_attached
  7) gdb_quit
- Remote debugging loop:
  1) gdb_gdbserver {ip,port,pid}
  2) gdb_target_remote {ip,port}
  3) gdb_add_variable_list / gdb_add_breakpoint
  4) gdb_continue, then step/next/continue loop
- On signals (sigsegv/sigabrt/etc): immediately call gdb_full_backtrace, gdb_current_code, gdb_variable_list, gdb_info_regs.
- Keep responses concise: call made, key findings, next action.
```

## 2. Quick Examples

**Local:**
```
gdb_execute {"executable_path":"/absolute/path/to/program"}
gdb_add_variable_list {"var":"robot->sim->robot_row"}
gdb_add_breakpoint {"filename":"/absolute/path/src/main.c","linenumber":20}
gdb_run {}
gdb_next {} / gdb_step {} / gdb_continue {} / gdb_interrupt {}
gdb_print {"var":"counter"}
gdb_full_backtrace {}
gdb_quit {}
```

**Remote:**
```
gdb_gdbserver {"ip":"127.0.0.1","port":11444,"pid":149104}
gdb_target_remote {"ip":"127.0.0.1","port":11444}
gdb_add_breakpoint {"filename":"/absolute/path/src/main.c","linenumber":20}
gdb_continue {}
gdb_next {} / gdb_step {} / gdb_interrupt {}
gdb_quit {}
```

## 3. Tool Reference

All calls return `debugger_state` plus optional: `error`, `current_func`, `current_code_path`, `current_code_line`, `current_code`, `backtrace`, `variable_list`.

### Session
| Tool | Args | When |
|------|------|------|
| `gdb_execute` | `executable_path` | Start session, attach to binary |
| `gdb_run` | none | Run loaded executable |
| `gdb_gdbserver` | `ip`, `port`, `pid` | Start gdbserver on running process |
| `gdb_target_remote` | `ip`, `port` | Connect to remote gdbserver |
| `gdb_set_thread` | `id` | Switch thread context |
| `gdb_set_frame` | `id` | Switch stack frame |
| `gdb_quit` | none | End session |
| `gdb_kill` | none | Kill process, keep debugger |
| `gdb_reset_back_to_not_attached` | none | Recover from error/signal state |

### Breakpoints
| Tool | Args | When |
|------|------|------|
| `gdb_add_breakpoint` | `filename`, `linenumber` | Insert breakpoint |
| `gdb_clear_breakpoint` | `filename`, `linenumber` | Remove breakpoint |
| `gdb_enable_breakpoint` | `filename`, `linenumber` | Enable disabled breakpoint |
| `gdb_disable_breakpoint` | `filename`, `linenumber` | Temporarily suppress breakpoint |
| `gdb_list_breakpoint` | none | List all breakpoints |

### Stepping
| Tool | Args | When |
|------|------|------|
| `gdb_next` | none | Step over function calls |
| `gdb_step` | none | Step into function calls |
| `gdb_continue` | none | Resume until breakpoint/signal/exit |
| `gdb_interrupt` | none | Interrupt running program and stop at stepping |

### Variables
| Tool | Args | When |
|------|------|------|
| `gdb_add_variable_list` | `var` | Add to watch list |
| `gdb_del_variable_list` | `var` | Remove from watch list |
| `gdb_variable_list` | none | Get all watched variables |
| `gdb_print` | `var` | One-off expression evaluation |
| `gdb_set_var` | `var`, `value` | Modify variable at runtime |

### Inspection
| Tool | Args | When |
|------|------|------|
| `gdb_debugger_state` | none | Get current debugger state |
| `gdb_current_code` | none | Source location + nearby code |
| `gdb_full_backtrace` | none | Full call stack |
| `gdb_info_threads` | none | Thread listing |
| `gdb_info_regs` | none | Register dump |
| `gdb_custom` | `cmd` | Raw GDB command |

## 4. Debugger States

- `not attached` / `failed to attach` => call `gdb_execute`
- `attached` => ready to run
- `running` => executing
- `stopped at breakpoint` / `stopped at stepping` => paused, inspect or continue
- `sigsegv` / `sigabrt` / `sigbus` / `sigfpe` / `sigill` / `sigtrap` / `sigterm` / `sigkill` => crash; collect diagnostics immediately
- `exited` => process ended
- `error` => something failed; check `error` field
