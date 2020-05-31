#[macro_use]
extern crate chan;
extern crate chan_signal;
use chan_signal::Signal;
use std::thread::spawn;

#[macro_use]
extern crate clap;

#[macro_use]
extern crate log;
extern crate env_logger;
use log::LogLevelFilter;
use env_logger::LogBuilder;

use std::convert::From;

extern crate irc;
use irc::client::prelude::*;
use irc::error::IrcError;
use std::default::Default;
use std::str::FromStr;

extern crate isatty;
use isatty::stdout_isatty;

use std::io::prelude::*;
use std::io::BufReader;
use std::io::Error as IoError;
use std::io::stdout;


/// Actions that threads can take
#[derive(Debug)]
enum Action {
    Connect,
    Quit,
    Join,
    JoinFail(String),
    IoError(IoError),
    IrcError(IrcError),
    ParseError(&'static str),
}

impl From<IrcError> for Action {
    fn from(err: IrcError) -> Action {
        Action::IrcError(err)
    }
}

impl From<IoError> for Action {
    fn from(err: IoError) -> Action {
        Action::IoError(err)
    }
}

impl From<&'static str> for Action {
    fn from(err: &'static str) -> Action {
        Action::ParseError(err)
    }
}


fn main() {
    // Catch signals we expect to exit cleanly from
    // I don't think rust actually sends SIGPIPE, but we'll catch it anyway
    let signal = chan_signal::notify(&[Signal::INT, Signal::TERM, Signal::PIPE]);


    // Parse args
    let matches = clap_app!
        (hashpipe =>
         (version: crate_version!())
         (author: crate_authors!())
         (about: "#|: Pipes data to and from an IRC connection")
         (@arg server: -s --server +required +takes_value "IRC server to connect to")
         (@arg port: -p --port +takes_value "Port to use (default: 6667, or 6697 with SSL)")
         (@arg no_ssl: -d long("--no-ssl") "Disable SSL encryption")
         (@arg nick: -n --nick +takes_value "Nickname to use (default: hashpipe)")
         (@arg channels: -c --channels +takes_value "Channel(s) to speak in \
          (defalt: #hashpipe, or nothing if using raw input)")
         // NOTE: long() is a required workaround for parsing long options with
         // hyphens; see https://github.com/kbknapp/clap-rs/issues/321
         (@arg raw_out: -o long("--raw-out") "Echo everything from the IRC server directly")
         (@arg raw_in: -i long("--raw-in") "Interpret STDIN as raw IRC commands")
         (@arg v: -v +multiple "Verbosity (1 for info, 2 for debug)")
         (@arg quiet: -q "Only print errors (overrides -v; overridden by raw output)")
        )
        .get_matches();

    let raw_out = matches.is_present("raw_out");
    let raw_in = matches.is_present("raw_in");
    let quiet = matches.is_present("quiet");

    let nick = matches.value_of("nick").unwrap_or("hashpipe").to_string();
    let server = matches.value_of("server").unwrap().to_string();
    let ssl = !matches.is_present("no_ssl");
    let port = matches.value_of("port").and_then(|p| p.parse().ok());

    // Filter empty strings from channel names
    let sanitized_chans: Vec<String> = matches.value_of("channels")
        .map(|chans| {
            chans.split(',')
                .filter(|x| *x != "")
                .map(|x| x.to_string())
                .collect()
        })
        .unwrap_or(vec![]);

    let channels: Vec<String> = if sanitized_chans.len() != 0 || raw_in {
        sanitized_chans
    } else {
        vec!["#hashpipe".to_string()]
    };


    // Set up logger
    let mut builder = LogBuilder::new();
    let level = match (matches.occurrences_of("v"), quiet) {
        (_, true) => LogLevelFilter::Error,
        (0, _) => LogLevelFilter::Warn,
        (1, _) => LogLevelFilter::Info,
        (2, _) | _ => LogLevelFilter::Debug,
    };
    builder.filter(None, level);
    builder.init().unwrap();


    // Set up IRC server
    let cfg = Config {
        nickname: Some(nick),
        server: Some(server),
        port,
        use_ssl: Some(ssl),
        channels: Some(channels.clone()),
        ..Default::default()
    };

    let client = match IrcClient::from_config(cfg) {
        Ok(val) => val,
        Err(e) => {
            error!("{}", e);
            return;
        }
    };

    // Connect to IRC on its own thread
    let irc_client = client.clone();
    let (sirc, rirc) = chan::sync(0);

    debug!("Spawning IRC client...");
    spawn(move || run_irc(irc_client, raw_out, quiet, sirc));

    // Wait until we've joined all the channels we need to
    let mut join_count = 0;
    let mut max_joins = channels.len();
    let mut connected = false;
    while !connected || join_count < max_joins {
        chan_select! {
            signal.recv() -> signal => {
                debug!("Received signal {:?}; quitting", signal);
                client.send_quit("#|").unwrap();
                return;
            },
            rirc.recv() -> action => match action {
                Some(Action::Connect) => {
                    connected = true;
                    debug!("Connected!");
                },
                Some(Action::Join) => join_count += 1,
                Some(Action::Quit) => {
                    debug!("QUIT received while attempting to join channels");
                    client.send_quit("#|").unwrap();
                    return;
                },
                Some(Action::JoinFail(err)) => {
                    max_joins -= 1;
                    warn!("{}", err);
                },
                Some(Action::IoError(err)) => {
                    error!("{}", err);
                    client.send_quit("#|").unwrap();
                    return;
                },
                _ => ()
            },
        }
    }

    info!("Joined {} channels", join_count);

    // Open stdin and write it to the desired channels
    let io_client = client.clone();
    let (sio, rio) = chan::sync(0);

    debug!("Spawning stdin reader...");
    spawn(move || run_io(io_client, channels, raw_in, sio));

    loop {
        chan_select! {
            signal.recv() -> signal => {
                debug!("Received signal {:?}; quitting", signal);
                break;
            },
            rio.recv() -> action => match action {
                Some(Action::IoError(err)) => {
                    error!("{}", err);
                    break;
                },
                Some(Action::ParseError(err)) => {
                    // TODO should this quit?
                    warn!("{}", err);
                },
                _ => if stdout_isatty() {
                    break;
                },
            },
            rirc.recv() -> action => match action {
                Some(Action::Quit) => {
                    debug!("Quit received");
                    break;
                },
                Some(Action::IoError(err)) => {
                    error!("{}", err);
                    break;
                },
                _ => (),
            },
        }
    }

    info!("Quitting!");
    client.send_quit("#|").unwrap();
}

/// Manage IRC connection; read and print messages; signal on JOIN or QUIT
fn run_irc(client: IrcClient, raw: bool, quiet: bool, sjoin: chan::Sender<Action>) {
    client.identify().unwrap_or_else(|err| sjoin.send(From::from(err)));

    let res = client.for_each_incoming(|msg| {
        if raw {
            print!("{}", msg);
            stdout().flush().unwrap_or_else(|err| sjoin.send(From::from(err)));
        }
        match msg.command {
            Command::JOIN(ref _channel, ref _a, ref _b) => sjoin.send(Action::Join),
            Command::PRIVMSG(ref target, ref what_was_said) => {
                if !raw && !quiet {
                    println!("{}->{}: {}",
                                msg.source_nickname().unwrap_or("* "),
                                target,
                                what_was_said);
                    stdout().flush().unwrap_or_else(|err| sjoin.send(From::from(err)));
                }
            }
            Command::NOTICE(ref target, ref what_was_said) => {
                if !raw && !quiet {
                    println!("{}->{}: {}",
                                msg.source_nickname().unwrap_or("* "),
                                target,
                                what_was_said);
                    stdout().flush().unwrap_or_else(|err| sjoin.send(From::from(err)));
                }
            }
            Command::QUIT(ref _quitmessage) => sjoin.send(Action::Quit),
            Command::Response(ref response, ref command, ref err) => {
                // Handle un-joinable channels
                let the_problem = command.get(1).map_or("", |s| &*s);
                let errmsg = the_problem.to_string() + ": " +
                                &err.clone().unwrap_or("".to_string());

                match *response {
                    Response::ERR_CHANNELISFULL => sjoin.send(Action::JoinFail(errmsg)),
                    Response::ERR_INVITEONLYCHAN => sjoin.send(Action::JoinFail(errmsg)),
                    Response::ERR_BANNEDFROMCHAN => sjoin.send(Action::JoinFail(errmsg)),
                    Response::ERR_BADCHANNELKEY => sjoin.send(Action::JoinFail(errmsg)),
                    Response::ERR_NOSUCHCHANNEL => sjoin.send(Action::JoinFail(errmsg)),

                    Response::RPL_ENDOFMOTD => sjoin.send(Action::Connect),
                    Response::ERR_NOMOTD => sjoin.send(Action::Connect),
                    _ => (),
                }
            }
            _ => (),
        }
    });
    if let Err(err) = res {
        sjoin.send(From::from(err));
    }

    sjoin.send(Action::Quit)
}

/// Read stdin and write each line to channels/the server
fn run_io(client: IrcClient, channels: Vec<String>, raw: bool, sdone: chan::Sender<Action>) {
    let stdin = BufReader::new(std::io::stdin());
    for line in stdin.lines() {
        match line {
            Ok(ln) => {
                if raw {
                    let raw_line = ln + "\r\n"; // IRC line terminator
                    match Message::from_str(&raw_line) {
                        Ok(msg) => {
                            client.send(msg).unwrap_or_else(|err| sdone.send(From::from(err)))
                        }
                        Err(err) => sdone.send(From::from(err)),
                    }
                } else {
                    for channel in &channels {
                        client.send_privmsg(&channel, &ln)
                            .unwrap_or_else(|err| sdone.send(From::from(err)));
                    }
                }
            }
            Err(err) => sdone.send(From::from(err)),
        }
    }
    // When this function ends, it drops sdone, signaling main
}
