#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use capabilities::*;
use ignite_std::process::Invocation;

fn main() {
    std::process::exit(ignite_io::run(|| {
        let tool = ignite_io::Setting::launch("--tool", Some("/bin/sh"))?;
        let files = ignite_io::Directory::launch("workspace", true, "--root")?;
        let tools = ignite_io::Launcher::with_maximum(4);
        let entropy = ignite_io::Entropy::open()?;
        let bench = Bench { files: files.clone(), tools: tools.clone(), entropy: entropy.clone(), tool: tool.clone(), executable: String::from("bench") };
        Bench__main(&bench)
    }))
}
