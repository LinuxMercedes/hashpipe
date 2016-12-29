#[macro_use]
extern crate chan;
extern crate chan_signal;

#[macro_use]
extern crate clap;

extern crate irc;

use std::io::prelude::*;
use std::io::BufReader;

use irc::client::prelude::*;
use std::default::Default;
use std::str::FromStr;

use chan_signal::Signal;
use std::thread::spawn;

/// Actions that the IRC server can take
enum Action {
    Quit,
    Join,
}

fn main() {
    // Catch signals we expect to exit cleanly from
    let signal = chan_signal::notify(&[Signal::INT, Signal::TERM, Signal::PIPE]);

    let matches = clap_app!
        (hashpipe =>
         (version: "0.1")
         (author: "LinuxMercedes <linuxmercedes@gmail.com>")
         (about: "Hashpipe: Pipes data to and from an IRC connection")
         (@arg server: -s --server +required +takes_value "IRC server to connect to")
         (@arg nick: -n --nick +takes_value "Nickname to use")
         (@arg channels: -c --channels +takes_value "Channel(s) to speak in")
         // NOTE: long() is a required workaround for parsing long options with
         // hyphens; see https://github.com/kbknapp/clap-rs/issues/321
         (@arg raw_out: -o long("--raw-out") "Echo everything from the IRC server directly")
         (@arg raw_in: -i long("--raw-in") "Interpret STDIN as raw IRC commands")
        )
        .get_matches();

    let raw_out = matches.is_present("raw_out");
    let raw_in = matches.is_present("raw_in");

    let nick = matches.value_of("nick").unwrap_or("hashpipe").to_string();
    let server = matches.value_of("server").unwrap().to_string();
    let channels: Vec<String> = match matches.value_of("channels") {
        Some(chans) => chans.split(",").map(|x| x.to_string()).collect(),
        None => {
            if raw_in {
                vec![]
            } else {
                vec!["#hashpipe".to_string()]
            }
        }
    };

    let cfg = Config {
        nickname: Some(nick),
        server: Some(server),
        channels: Some(channels.clone()),
        ..Default::default()
    };

    let server = IrcServer::from_config(cfg).unwrap();

    // Connect to IRC on its own thread
    let irc_server = server.clone();
    let (sirc, rirc) = chan::sync(0);

    spawn(move || run_irc(irc_server, raw_out, sirc));

    // Wait until we've joined all the channels we need to
    let mut join_count = 0;
    while join_count < channels.len() {
        chan_select! {
            signal.recv() -> _signal => {
                server.send_quit("#|").unwrap();
                return;
            },
            rirc.recv() -> action => match action {
                Some(Action::Join) => join_count += 1,
                Some(Action::Quit) => {
                    server.send_quit("#|").unwrap();
                    return;
                },
                _ => ()
            },
        }
    }

    println!("Joined {} channels", join_count);

    // Open stdin and write it to the desired channels
    let io_server = server.clone();
    let (sio, rio) = chan::sync(0);

    spawn(move || run_io(io_server, channels, raw_in, sio));

    loop {
        chan_select! {
            signal.recv() -> _signal => break,
            rio.recv() => break,
            rirc.recv() -> action => match action {
                Some(Action::Quit) => break,
                _ => (),
            },
        }
    }

    println!("Exiting!");
    server.send_quit("#|").unwrap();
}

/// Manage IRC connection; read messages and signal on JOIN
fn run_irc(server: IrcServer, raw: bool, sjoin: chan::Sender<Action>) {
    server.identify().unwrap();
    for message in server.iter() {
        let msg = message.unwrap();
        if raw {
            print!("{}", msg);
        }
        match msg.command {
            Command::JOIN(ref _channel, ref _a, ref _b) => sjoin.send(Action::Join),
            Command::PRIVMSG(ref target, ref what_was_said) => {
                if !raw {
                    println!("{}{}: {}",
                             msg.source_nickname().unwrap(),
                             target,
                             what_was_said)
                }
            }
            Command::QUIT(ref _quitmessage) => sjoin.send(Action::Quit),
            _ => (),
        }
    }

    sjoin.send(Action::Quit)
}

/// Read stdin and write each line to all channels
fn run_io(server: IrcServer, channels: Vec<String>, raw: bool, _sdone: chan::Sender<()>) {
    let stdin = BufReader::new(std::io::stdin());
    for line in stdin.lines() {
        let ln = line.unwrap();
        if raw {
            let raw_line = ln + "\r\n"; // IRC line terminator
            let msg = Message::from_str(&raw_line).unwrap();
            server.send(msg).unwrap();
        } else {
            for channel in &channels {
                server.send_privmsg(&channel, &ln).unwrap()
            }
        }
    }
    // When this function ends, it drops _sdone, signaling main
}
