#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use terminal::*;
use ::ignite_std::console::CommandConsole;
use ::ignite_std::protocol::telnet::{TelnetServer, TelnetClient};

fn main() {
    ::std::process::exit(::ignite_io::run(|| {
        let __port = ::ignite_io::args::port()?;
        let ingress = ::ignite_io::TcpListener::bind(String::from("127.0.0.1"), __port.unwrap_or(2323))?;
        ::ignite_io::log(&format!("listening on 127.0.0.1:{}", ingress.local_port()));
        let console = CommandConsole { implementation: ::ignite_std::console::ConsoleCommands { respond: (::std::sync::Arc::new(|__a0: String| terminal::commands::respond(&__a0)) as ::std::sync::Arc<dyn Fn(String) -> ::ignite_std::console::Reply + Send + Sync>) }, prompt: String::from("terminal> "), maximum_line: 4096i64, command_schema: Some(::ignite_std::console::CommandSchema { commands: (::std::sync::Arc::new(|| terminal::command_schema::commands()) as ::std::sync::Arc<dyn Fn() -> Vec<String> + Send + Sync>) }) };
        let server = TelnetServer { listener: ingress.clone(), application: console.clone(), executable: String::from("terminald"), protocol: TelnetServer::default_protocol(), sessions: TelnetServer::default_sessions(), recovery: TelnetServer::default_recovery(), rollout: TelnetServer::default_rollout() };
        ::ignite_std::protocol::telnet::TelnetServer__main(server)
    }))
}
