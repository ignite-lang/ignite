#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use ::ignite_std::console::CommandConsole;
use ::ignite_std::protocol::telnet::{TelnetServer, TelnetClient};

pub mod command_schema {

    pub fn commands() -> Vec<String> {
        vec![String::from("help"), String::from("echo"), String::from("status"), String::from("quit")]
    }
}

pub mod commands {
    use ::ignite_std::console::{Reply, Action, first_word, rest_after_word};

    #[derive(Clone)]
    pub enum Command {
        Help,
        Echo(String),
        Status,
        Quit,
    }

    pub fn answer(command: &Command) -> Reply {
        match command {
            Command::Help => Reply { lines: vec![String::from("help"), String::from("echo <text>"), String::from("status"), String::from("quit")], action: Reply::default_action() },
            Command::Echo(text) => Reply { lines: vec![text.clone()], action: Reply::default_action() },
            Command::Status => Reply { lines: vec![String::from("ready")], action: Reply::default_action() },
            Command::Quit => Reply { lines: vec![String::from("bye")], action: Action::Close },
        }
    }

    pub fn parse(line: &String) -> ::std::option::Option<Command> {
        match first_word(line) {
            __m if (*(&__m) == "help") => Some(Command::Help),
            __m if (*(&__m) == "status") => Some(Command::Status),
            __m if (*(&__m) == "quit") => Some(Command::Quit),
            __m if (*(&__m) == "echo") => Some(Command::Echo(rest_after_word(line))),
            _ => None,
        }
    }

    pub fn respond(line: &String) -> Reply {
        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            Reply { lines: vec![], action: Reply::default_action() }
        } else {
            match parse(&trimmed) {
                Some(command) => answer(&command),
                None => Reply { lines: vec![format!("unknown command: {}", first_word(&trimmed))], action: Reply::default_action() },
            }
        }
    }
}
