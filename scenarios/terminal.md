# Terminals

**X1 output.** Given a Project, when `terminal.spawn {command: ["/bin/sh","-c","echo hi"]}` is called and a client is subscribed, then a `terminal.output` event whose base64 `data` decodes to bytes containing `hi` is emitted.

**X2 input.** Given a Terminal running `cat`, when `terminal.write` sends `"ping\n"` (base64), then `terminal.output` events carry `ping` back.

**X3 exit.** Given a Terminal running a command that exits with code 3, then `terminal.exited {code: 3}` is emitted and `terminal.list` shows `running: false, exit_code: 3`. A program killed by a signal has `code: null`.

**X4 title.** Given a Terminal whose program prints the OSC sequence `ESC ] 0 ; hello BEL`, then `terminal.title {title: "hello"}` is emitted and `terminal.list` shows `title: "hello"`. A later title replaces it.

**X5 resize.** Given a Terminal spawned at 80x24, when `terminal.resize {cols: 100, rows: 30}` is called, then a program that prints its size (`stty size`) reports `30 100`.

**X6 kill.** Given a running Terminal, when `terminal.kill` is called, then the program stops, `terminal.exited` is emitted, and the Terminal stays in `terminal.list` as exited. Killing an exited or unknown Terminal is `NOT_FOUND`.

**X7 cwd and env.** Given `cwd` and `env` in `SpawnParams`, then the program runs in that directory and sees those variables (and the parent's, minus nothing).

**X8 in-process API.** The same operations (spawn, write, resize, kill, subscribe to a Terminal's events) are plain async Rust methods on `Terminals`, usable without a socket; the agents module calls them. Output events keep arriving to a subscriber in order, and a slow subscriber never blocks the PTY reader.
