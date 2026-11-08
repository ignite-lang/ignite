//! The toolchain image of a release (`dist/toolchain/Dockerfile`, `task
//! image`): `ignite --version` prints the release, and inside the image,
//! without a network, `ignite build` of the todo example succeeds on both
//! targets and each built `todod` answers one HTTP request. Run by `task
//! image-check IMAGE=… VERSION=…`; ignored otherwise, since it needs docker
//! and a built image.

use std::process::{Command, Output};
use std::time::{Duration, Instant};

/// The tag's source inside the image (`dist/toolchain/Dockerfile`).
const SOURCE: &str = "/usr/local/src/ignite";

fn docker(args: &[&str]) -> Output {
    Command::new("docker")
        .args(args)
        .output()
        .expect("start docker")
}

fn ok(args: &[&str]) -> String {
    let output = docker(args);
    assert!(
        output.status.success(),
        "docker {}\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Removes the container when the test ends, passed or not.
struct Container(String);

impl Drop for Container {
    fn drop(&mut self) {
        let _ = docker(&["rm", "-f", &self.0]);
    }
}

/// Starts `binary` in the container's `/work` and returns the body of
/// `GET /health` once it answers, within 20 s; then stops the server.
fn health(container: &str, binary: &str) -> String {
    ok(&["exec", "-d", "-w", "/work", container, binary]);
    let deadline = Instant::now() + Duration::from_secs(20);
    let answer = loop {
        let output = docker(&[
            "exec",
            container,
            "curl",
            "-sS",
            "-m",
            "2",
            "http://127.0.0.1:8080/health",
        ]);
        if output.status.success() {
            break String::from_utf8_lossy(&output.stdout).into_owned();
        }
        assert!(
            Instant::now() < deadline,
            "{binary} did not answer: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::thread::sleep(Duration::from_millis(200));
    };
    ok(&["exec", container, "pkill", "-x", "todod"]);
    let deadline = Instant::now() + Duration::from_secs(10);
    while docker(&["exec", container, "pgrep", "-x", "todod"])
        .status
        .success()
    {
        assert!(Instant::now() < deadline, "{binary} did not stop");
        std::thread::sleep(Duration::from_millis(100));
    }
    answer
}

#[test]
#[ignore = "needs docker and IGNITE_IMAGE; run by `task image-check`"]
fn the_image_builds_and_serves_the_todo_example_on_both_targets() {
    let image = std::env::var("IGNITE_IMAGE").expect("IGNITE_IMAGE names the image");
    let version = std::env::var("IGNITE_VERSION").expect("IGNITE_VERSION names the release");

    let printed = ok(&[
        "run",
        "--rm",
        "--network",
        "none",
        &image,
        "ignite",
        "--version",
    ]);
    assert_eq!(printed.trim(), format!("ignite {version}"));

    let name = format!("ignite-image-check-{}", std::process::id());
    ok(&[
        "run",
        "-d",
        "--name",
        &name,
        "--network",
        "none",
        &image,
        "sleep",
        "infinity",
    ]);
    let container = Container(name);
    ok(&["exec", &container.0, "mkdir", "-p", "/work"]);
    let todo = format!("{SOURCE}/examples/todo/todo.ignite");
    for target in ["llvm", "rust"] {
        let out = format!("/work/{target}");
        ok(&[
            "exec",
            "-w",
            "/work",
            &container.0,
            "ignite",
            "build",
            &todo,
            "--target",
            target,
            "--out",
            &out,
        ]);
        let binary = match target {
            "llvm" => format!("{out}/bin/todod"),
            _ => format!("{out}/target/debug/todod"),
        };
        assert_eq!(health(&container.0, &binary), "ok\n", "{target}");
    }
}
