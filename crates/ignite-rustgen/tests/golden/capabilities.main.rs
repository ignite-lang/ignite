#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use ::capabilities::*;
use ::ignite_std::process::Invocation;

fn main() {
    ::std::process::exit(::ignite_io::run(|| {
        let __res_tool = ::ignite_io::Setting::launch("--tool", Some("/bin/sh"))?;
        let __res_files = ::ignite_io::Directory::launch("workspace", true, "--root")?;
        let __res_tools = ::ignite_io::Launcher::with_maximum(4);
        let __res_entropy = ::ignite_io::Entropy::open()?;
        let __res_bench = Bench { files: __res_files.clone(), tools: __res_tools.clone(), entropy: __res_entropy.clone(), tool: __res_tool.clone(), executable: String::from("bench") };
        Bench__main(&__res_bench)
    }))
}
