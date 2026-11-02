
warning: Section .debug_names in /home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/s9c-todo/bin/todod length 124 does not match section length 3100, ignoring .debug_names.
Breakpoint 1 at 0x13632: file /home/timo/projects/telos/examples/todo/todo.telos, line 28.

This GDB supports auto-downloading debuginfo from the following URLs:
  <https://debuginfod.archlinux.org>
Enable debuginfod for this session? (y or [n]) [answered N; input not from terminal]
Debuginfod has been disabled.
To make this setting permanent, add 'set debuginfod enabled off' to .gdbinit.
[Thread debugging using libthread_db enabled]
Using host libthread_db library "/usr/lib/libthread_db.so.1".
listening on 127.0.0.1:18431
[New Thread 0x7ffff79ff6c0 (LWP 2898502)]
[Switching to Thread 0x7ffff79ff6c0 (LWP 2898502)]

Thread 2 "todod" hit Breakpoint 1, examples.todo.create () at /home/timo/projects/telos/examples/todo/todo.telos:28
28	      let todo = Todo { id = todos.next_id(), title, done = value.bool("done").unwrap_or(false) }
#0  examples.todo.create () at /home/timo/projects/telos/examples/todo/todo.telos:28
#1  0x0000555555568177 in examples.todo.handle () at /home/timo/projects/telos/examples/todo/todo.telos:66
#2  0x0000555555565ede in fnval.examples.todo[handle] ()
#3  0x00005555555665e1 in std.protocol.http.session<Store<examples.todo.Todo>> () at /home/timo/projects/telos/std/protocol/http.telos:527
Current source file is /home/timo/projects/telos/examples/todo/todo.telos
Compilation directory is /home/timo/projects/telos
Located in /home/timo/projects/telos/examples/todo/todo.telos
Contains 147 lines.
Source language is unknown.
Producer is telos.
Compiled with DWARF 5 debugging format.
Does not include preprocessor macro info.
Kill the program being debugged? (y or n) [answered Y; input not from terminal]
[Inferior 1 (process 2898477) killed]
