# #|: Pipe data to and from IRC

Hashpipe lets you connect stdin and stdout to IRC with Unix pipes. For example, `sleep 5; echo "done" | hashpipe -s irc.freenode.net` messages `#hashpipe` when a long-running command finishes.

By default, stdin is echoed to all channels hashpipe joins, and NOTICEs and PRIVMSGs are printed to stdout. You can even use it as a (poor) IRC client:

```
$ hashpipe -s irc.freenode.net
lm->#hashpipe: hello there hashpipe
hello to you, lm
lm->hashpipe: I can even /msg you
neat!
(I can't msg you back, though)
```

## Raw mode
Hashpipe can also parse stdin as raw IRC commands and/or output everything the server sends. This is primarily useful for sending direct PRIVMSGs or automating various oper tasks.

It understands how to parse the commands listed [here](http://aatxe.github.io/irc/irc/client/data/command/enum.Command.html).
If a command fails to parse, it will warn you, but continue processing stdin.

## Usage
```
USAGE:
    hashpipe [FLAGS] [OPTIONS] --server <server>

FLAGS:
    -h, --help       Prints help information
    -q               Only print errors (overrides -v; overridden by raw output)
    -i, --raw-in     Interpret STDIN as raw IRC commands
    -o, --raw-out    Echo everything from the IRC server directly
    -e, --ssl        Enable SSL encryption
    -v               Verbosity (1 for info, 2 for debug)
    -V, --version    Prints version information

OPTIONS:
    -c, --channels <channels>    Channel(s) to speak in (defalt: #hashpipe, or nothing if using raw input)
    -n, --nick <nick>            Nickname to use (default: hashpipe)
    -p, --port <port>            Port to use (default: 6667, or 6697 with SSL)
    -s, --server <server>        IRC server to connect to
```

