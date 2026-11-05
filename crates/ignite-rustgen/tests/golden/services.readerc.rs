#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use services::*;
use ignite_std::net::Endpoint;

fn main() {
    std::process::exit(ignite_io::run(|| {
        let port = ignite_io::args::port()?;
        let back_endpoint = Endpoint { address: String::from("0.0.0.0"), port: port.unwrap_or(7001) };
        let reader = Reader { target: back_endpoint.clone(), executable: String::from("readerc") };
        Reader__main(&reader)
    }))
}
