#[macro_use]
extern crate chan;
extern crate irc;

use std::io::prelude::*;
use std::io::BufReader;

use std::default::Default;
use irc::client::prelude::*;

use std::thread::spawn;

fn main() {
    let channels = vec![format!("#hashpipe")];
    let cfg = Config {
        nickname: Some(format!("hashpipe")),
        server: Some(format!("irc.wobscale.website")),
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
            rirc.recv() => {
                join_count+=1;
            },
        }
    }

    // Open stdin and write it to the desired channels
    let io_server = server.clone();
    let (sio, rio) = chan::sync(0);

    spawn (move || run_io(io_server, channels, sio));

    chan_select! {
        rio.recv() => {
            println!("Exiting!");
        },
    }
}

/*
 * Manage IRC connection; read messages and signal on JOIN
 */
fn run_irc(server: IrcServer, sjoin: chan::Sender<()>) {
    server.identify().unwrap();
    for message in server.iter() {
        let msg = message.unwrap();
        println!("{}", msg);
        match msg.command {
            Command::JOIN(ref _channel, ref _a, ref _b) => sjoin.send(()),
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
    server.send_quit("#|").unwrap();
    // When this function ends, it drops _sdone, signaling main
}
