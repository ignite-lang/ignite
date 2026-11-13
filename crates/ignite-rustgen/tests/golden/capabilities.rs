#![allow(unused_mut, unused_variables, dead_code, unused_parens, unused_imports, unreachable_code, non_snake_case, bindings_with_variant_name, clippy::all)]
use ::ignite_std::process::Invocation;

#[derive(Clone)]
pub struct Bench {
    pub files: ::ignite_io::Directory,
    pub tools: ::ignite_io::Launcher,
    pub entropy: ::ignite_io::Entropy,
    pub tool: ::ignite_io::Setting,
    pub executable: String,
}

impl Bench {
    #[allow(non_snake_case)]
    pub fn default_executable() -> String {
        String::from("bench")
    }
}

#[allow(non_snake_case)]
pub fn Bench__main(self_: &Bench) -> ::std::result::Result<(), ::ignite_io::IoError> {
    let data = (self_.files.read_file(String::from("in.txt")))?;
    (self_.files.write_file(String::from("out.txt"), data.clone()))?;
    let names = (self_.files.read_directory(String::from("")))?;
    (self_.files.create_directory(String::from("made")))?;
    (self_.files.remove(String::from("out.txt")))?;
    (self_.files.remove_tree(String::from("made")))?;
    let root = (self_.files.absolute_path())?;
    let invocation = Invocation { program: self_.tool.value(), arguments: ::ignite_io::List::of(vec![String::from("-c"), String::from("true")]), directory: root, environment: Invocation::default_environment(), inherit_environment: Invocation::default_inherit_environment(), timeout: Invocation::default_timeout(), output_limit: Invocation::default_output_limit(), input: Invocation::default_input() };
    let ran = (self_.tools.execute({ let i = invocation.clone(); ::ignite_io::Invocation { program: i.program, arguments: i.arguments.into(), directory: i.directory, environment: i.environment.into(), inherit_environment: i.inherit_environment, timeout: i.timeout, output_limit: i.output_limit, input: i.input } }).map(|e| ::ignite_std::process::ChildExit { ending: e.ending, code: e.code, stdout: e.stdout, stderr: e.stderr, dropped: e.dropped }))?;
    let child = (self_.tools.start({ let i = invocation.clone(); ::ignite_io::Invocation { program: i.program, arguments: i.arguments.into(), directory: i.directory, environment: i.environment.into(), inherit_environment: i.inherit_environment, timeout: i.timeout, output_limit: i.output_limit, input: i.input } }))?;
    let id = child.id();
    (child.kill())?;
    let waited = (child.wait().map(|e| ::ignite_std::process::ChildExit { ending: e.ending, code: e.code, stdout: e.stdout, stderr: e.stderr, dropped: e.dropped }))?;
    let token = (self_.entropy.random_bytes(32))?;
    Ok(())
}
