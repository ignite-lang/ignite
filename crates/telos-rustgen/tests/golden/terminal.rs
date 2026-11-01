#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, clippy::all)]
use telos_std::console::{Console, Reply, first_word, rest_after_word};
use telos_std::protocol::telnet::Telnet;

#[derive(Clone, Debug)]
pub enum Command {
    Help,
    Echo(String),
    Status,
    Quit,
}

#[allow(non_snake_case)]
pub fn PROMPT() -> String {
    String::from("terminal> ")
}

#[allow(non_snake_case)]
pub fn MAXIMUM_LINE() -> i64 {
    4096
}

pub fn respond(command: &Command) -> Reply {
    match command.clone() {
        Command::Help => Reply { lines: vec![String::from("help"), String::from("echo <text>"), String::from("status"), String::from("quit")], action: Reply::default_action() },
        Command::Echo(text) => Reply { lines: vec![text.clone()], action: Reply::default_action() },
        Command::Status => Reply { lines: vec![String::from("ready")], action: Reply::default_action() },
        Command::Quit => Reply { lines: vec![String::from("bye")], action: telos_std::console::Action::Close },
    }
}

pub fn parse(line: &String) -> Option<Command> {
    match first_word(line) {
        __m if (*(&__m) == "help") => Some(Command::Help),
        __m if (*(&__m) == "status") => Some(Command::Status),
        __m if (*(&__m) == "quit") => Some(Command::Quit),
        __m if (*(&__m) == "echo") => Some(Command::Echo(rest_after_word(line))),
        _ => None,
    }
}

pub fn execute(line: &String) -> Reply {
    let trimmed = line.trim().to_string();
    if trimmed.is_empty() {
        Reply { lines: vec![], action: Reply::default_action() }
    } else {
        match parse(&trimmed) {
            Some(command) => respond(&command),
            None => Reply { lines: vec![format!("{}{}", String::from("unknown command: "), first_word(&trimmed).clone())], action: Reply::default_action() },
        }
    }
}
