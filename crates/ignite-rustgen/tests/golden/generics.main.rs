#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use ::counters::*;
#[global_allocator]
static __IGNITE_ALLOCATOR: ::ignite_io::census::Counting = ::ignite_io::census::Counting;

fn main() {
    ::std::process::exit(::ignite_io::run(|| {
        let __res_counts: ::ignite_io::Store<i64> = ::ignite_io::Store::new();
        let __port = ::ignite_io::args::port()?;
        let __res_ingress = ::ignite_io::TcpListener::bind(String::from("127.0.0.1"), __port.unwrap_or(7400))?;
        ::ignite_io::log(&format!("listening on 127.0.0.1:{}", __res_ingress.local_port()));
        let __res_server = Server::<::ignite_io::Store<i64>> { listener: __res_ingress.clone(), application: App { state: __res_counts.clone(), handle: (::std::sync::Arc::new(|__a0: ::ignite_io::Store<i64>, __a1: String| respond(__a0, &__a1)) as ::std::sync::Arc<dyn Fn(::ignite_io::Store<i64>, String) -> String + Send + Sync>) }, executable: String::from("countersd") };
        Server__main(&__res_server)
    }))
}
