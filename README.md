# #|: Pipe data to and from IRC

Hashpipe lets you connect stdin and stdout to IRC with Unix pipes. For example,

```
sleep 5; echo "done" | hashpipe --server irc.freenode.net
```

messages the `#hashpipe` channel on the Freenode network when a long-running command finishes.

By default, stdin is echoed to all channels hashpipe joins, and NOTICEs and PRIVMSGs are printed to stdout. You can even use it as a (poor) IRC client:

```
$ hashpipe -s irc.freenode.net
lm->#hashpipe: hello there hashpipe
hello to you, lm
lm->hashpipe: I can even /msg you
neat!
(I can't msg you back, though)
```

Hashpipe is also handy for snooping on IRC channels:

```
if hashpipe --server my.irc.server --channels "#commandcentre" | grep -m 1 "oh geez, it's the law"; then
  shred ~/Documents/secret/**/*
fi
```

You can even do semi-interactive things with hashpipe:

```
echo "Say 'stop' in the next 60 seconds to cancel a reboot" | timeout 60 hashpipe -s my.irc.server | grep -m 1 "stop"
ret=$?
if [[ $ret -eq 1 ]]; then
  reboot;
fi
```

## Raw mode
Hashpipe can also parse stdin as raw IRC commands and/or output everything the server sends. This is primarily useful for sending direct PRIVMSGs or automating various oper tasks.

It understands how to parse the commands listed [here](http://aatxe.github.io/irc/irc/client/data/command/enum.Command.html).
If a command fails to parse, it will warn you, but continue processing stdin.

For instance,
```
$ hashpipe --server my.irc.server -io
:my.irc.server 1 hashpipe :Welcome to My IRC Server hashpipe!hashpipe@213.456.8.92
<snip>
:hashpipe!hashpipe@213.456.8.92 MODE hashpipe +x
JOIN #hashpipe
:hashpipe!hashpipe@213.456.8.92 JOIN #hashpipe
:my.irc.server 353 hashpipe @ #hashpipe :hashpipe @`lm` 
:my.irc.server 366 hashpipe #hashpipe :End of /NAMES list.
PRIVMSG #hashpipe :hello
PRIVMSG lm :hi there
NICK bob
:hashpipe!hashpipe@213.456.8.92 NICK :bob
```

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
    -v               Verbosity (-v for info, -vv for debug)
    -V, --version    Prints version information

OPTIONS:
    -c, --channels <channels>    Channel(s) to speak in (defalt: #hashpipe, or nothing if using raw input)
    -n, --nick <nick>            Nickname to use (default: hashpipe)
    -p, --port <port>            Port to use (default: 6667, or 6697 with SSL)
    -s, --server <server>        IRC server to connect to
```

