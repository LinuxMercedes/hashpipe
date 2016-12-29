#[macro_use]
extern crate chan;
extern crate chan_signal;

#[macro_use]
extern crate clap;

extern crate irc;

use std::io::prelude::*;
use std::io::BufReader;

use std::default::Default;
use irc::client::prelude::*;

use chan_signal::Signal;
use std::thread::spawn;

fn main() {
    // Catch signals we expect to exit cleanly from
    let signal = chan_signal::notify(&[Signal::INT, Signal::TERM, Signal::PIPE]);

    let matches = clap_app!(hashpipe =>
                            (version: "0.1")
                            (author: "LinuxMercedes <linuxmercedes@gmail.com>")
                            (about: "Hashpipe: Pipes data to and from an IRC connection")
                            (@arg server: -s --server +required +takes_value "IRC server to connect to")
                            (@arg nick: -n --nick +takes_value "Nickname to use")
                            (@arg channels: -c --channels +takes_value "Channel(s) to speak in (if not in raw mode)")
                           ).get_matches();

    let nick = matches.value_of("nick").unwrap_or("hashpipe").to_string();
    let server = matches.value_of("server").unwrap().to_string();
    let channels :Vec<String> = matches.value_of("channels").unwrap_or("#hashpipe").split(",").map(|x| x.to_string()).collect();

    let cfg = Config {
        nickname: Some(nick),
        server: Some(server),
        channels: Some(channels.clone()),
        .. Default::default()
    };

    let server = IrcServer::from_config(cfg).unwrap();

    // Connect to IRC on its own thread
    let irc_server = server.clone();
    let (sirc, rirc) = chan::sync(0);

    spawn (move || run_irc(irc_server, sirc));

    // Wait until we've joined all the channels we need to
    let mut join_count = 0;
    while join_count < channels.len() {
        chan_select! {
            signal.recv() -> _signal => {
                server.send_quit("#|").unwrap();
                return;
            },
            rirc.recv() => {
                join_count+=1;
            },
        }
    }

    println!("Joined {} channels", join_count);

    // Open stdin and write it to the desired channels
    let io_server = server.clone();
    let (sio, rio) = chan::sync(0);

    spawn (move || run_io(io_server, channels, sio));

    chan_select! {
        signal.recv() -> _signal => {
            /* Falls through to quit after this select block */
        },
        rio.recv() => {
            /* Falls through to quit after this select block */
        },
    }

    println!("Exiting!");
    server.send_quit("#|").unwrap();
}

/*
 * Manage IRC connection; read messages and signal on JOIN
 */
fn run_irc(server: IrcServer, sjoin: chan::Sender<()>) {
    server.identify().unwrap();
    for message in server.iter() {
        let msg = message.unwrap();
        match msg.command {
            Command::JOIN(ref _channel, ref _a, ref _b) => sjoin.send(()),
            Command::PRIVMSG(ref target, ref what_was_said) => println!("{}{}: {}", msg.source_nickname().unwrap(), target, what_was_said),
            _ => (),
        }
    }
}

/*
 * Read stdin and write each line to all channels
 */
fn run_io(server: IrcServer, channels: Vec<String>, _sdone: chan::Sender<()>) {
    let stdin = BufReader::new(std::io::stdin());
    for line in stdin.lines() {
        let ln = line.unwrap();
        for channel in &channels {
            server.send_privmsg(&channel, &ln).unwrap()
        }
    }
    // When this function ends, it drops _sdone, signaling main
}
