//! `std.protocol.mysql` against real MySQL servers (`docs/slice.md`, the
//! MySQL client section): two containers of the image [`IMAGE`] on free
//! loopback ports, one whose default authentication plugin is
//! `caching_sha2_password` and one whose default is
//! `mysql_native_password`, both with TLS under an authority this test
//! generates, both initialised from [`INIT`]. The Ignite tests of
//! [`PROGRAM`] then run once per target against the same two servers:
//! both plugins directly and after an authentication switch, the password
//! itself over TLS only, the refusals, every column type by `query` and by
//! `execute`, parameters, a read-only session, a pool and the read
//! timeout. Single connections, no load.
//!
//! The test needs a container runtime and the image: it runs where `docker
//! version` succeeds and the image is present (`docker pull mysql:8.0`),
//! and says on standard error why it did nothing anywhere else. It pulls
//! nothing itself. `IGNITE_MYSQL_IMAGE` names another image,
//! `IGNITE_CONTAINER_LABEL` a `key=value` label for the containers. The
//! inline tests of `std/protocol/mysql.ignite` cover the same client over
//! a scripted server in every gate. The accounts and passwords below exist
//! only inside these containers.

use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, DnType, IsCa, KeyPair, KeyUsagePurpose,
};

use crate::common;

/// The server image; `IGNITE_MYSQL_IMAGE` overrides it.
const IMAGE: &str = "mysql:8.0";

/// What both servers start with: one account per plugin, one with an empty
/// password, one with a plugin the client does not speak, and a table
/// with a column of every type.
const INIT: &str = r#"SET NAMES utf8mb4;
CREATE DATABASE shop CHARACTER SET utf8mb4;
CREATE USER 'reader'@'%' IDENTIFIED WITH caching_sha2_password BY 'reader-pw';
CREATE USER 'legacy'@'%' IDENTIFIED WITH mysql_native_password BY 'legacy-pw';
CREATE USER 'nobody'@'%' IDENTIFIED WITH caching_sha2_password BY '';
CREATE USER 'old'@'%' IDENTIFIED WITH sha256_password BY 'old-pw';
GRANT SELECT, INSERT ON shop.* TO 'reader'@'%', 'legacy'@'%', 'nobody'@'%', 'old'@'%';
CREATE TABLE shop.things (
  id INT PRIMARY KEY,
  flag TINYINT(1),
  tiny TINYINT,
  utiny TINYINT UNSIGNED,
  small SMALLINT,
  medium MEDIUMINT,
  whole INT,
  big BIGINT,
  huge BIGINT UNSIGNED,
  yr YEAR,
  price DECIMAL(10,2),
  ratio FLOAT,
  exact DOUBLE,
  name VARCHAR(100),
  code CHAR(4),
  note TEXT,
  raw BLOB,
  fixed VARBINARY(8),
  bit1 BIT(1),
  bits BIT(12),
  doc JSON,
  day DATE,
  at DATETIME(6),
  stamp TIMESTAMP NULL,
  span TIME,
  mood ENUM('glad','sad'),
  tags SET('a','b','c')
) CHARACTER SET utf8mb4;
INSERT INTO shop.things VALUES (1, 1, -128, 255, -32768, -8388608, -2147483648, 9223372036854775807, 18446744073709551615, 2024, -1234.50, 1.5, 0.1, 'Zoë', 'ab', 'long text', x'00ff', x'0102', b'1', b'101001011010', '{"a": 1}', '2024-02-29', '2024-02-29 23:59:58.123456', '2024-02-29 23:59:58', '-34:11:12', 'glad', 'a,b');
INSERT INTO shop.things (id) VALUES (2);
"#;

/// The Ignite tests; `@SHA2@`, `@NATIVE@` and `@DEAD@` are replaced by
/// ports, `@AUTHORITY@` by the authority's certificate.
const PROGRAM: &str = r##"package real

import std.net.{Endpoint}
import std.tls.{TlsOptions}
import std.text.{contains}
import std.protocol.mysql.{MysqlOptions, MysqlError, MysqlValue, ServerError, ResultSet, Pool, connect, connect_tls, query, prepare, execute, close, execute_once, ping, quit, describe, pool_of, pool_query, pool_execute, checkout, checkin, quit_idle}

const SHA2: Int = @SHA2@
const NATIVE: Int = @NATIVE@
const DEAD: Int = @DEAD@
const AUTHORITY: Text = r#"@AUTHORITY@"#
const NEEDS_TLS: Text = "the server asks for the password itself (caching_sha2_password full authentication), which is sent over TLS only"

test fn at(port: Int) -> Endpoint = Endpoint { address = "127.0.0.1", port }

test fn account(user: Text, password: Text) -> MysqlOptions =
  MysqlOptions { user, password, database = "shop", server_name = "localhost", tls = TlsOptions { authorities = AUTHORITY } }

test fn legacy() -> MysqlOptions = account("legacy", "legacy-pw")

test fn failure<R>(outcome: Result<R, MysqlError>) -> Text = match outcome {
  Ok(_) => "no error"
  Err(error) => describe(error)
}

test fn code<R>(outcome: Result<R, MysqlError>) -> Int = match outcome {
  Err(Server(failed)) => failed.code
  _ => 0
}

test "caching_sha2_password: the password itself goes over TLS only, then the scramble is enough" {
  let root = connect_tls(at(SHA2), account("root", "root-pw"))?
  assert root.secure && contains(root.version, "8.")
  _ = query(root, "FLUSH PRIVILEGES")?
  quit(root)?
  assert failure(connect(at(SHA2), account("reader", "reader-pw"))) == NEEDS_TLS
  let secure = connect_tls(at(SHA2), account("reader", "reader-pw"))?
  assert query(secure, "SELECT CURRENT_USER()")?.rows == [[Some(Text("reader@%"))]]
  let cipher = query(secure, "SHOW SESSION STATUS LIKE 'Ssl_cipher'")?
  assert cipher.rows.len() == 1 && cipher.rows[0][1] != Some(Text(""))
  quit(secure)?
  let plain = connect(at(SHA2), account("reader", "reader-pw"))?
  assert !plain.secure
  assert query(plain, "SELECT CURRENT_USER()")?.rows == [[Some(Text("reader@%"))]]
  let unencrypted = query(plain, "SHOW SESSION STATUS LIKE 'Ssl_cipher'")?
  assert unencrypted.rows[0][1] == Some(Text(""))
  quit(plain)?
  let nobody = connect(at(SHA2), MysqlOptions { user = "nobody" })?
  assert ping(nobody) == Ok(())
  quit(nobody)?
}

test "mysql_native_password: after an authentication switch, as the server's default, and over TLS" {
  let switched = connect(at(SHA2), legacy())?
  assert query(switched, "SELECT CURRENT_USER()")?.rows == [[Some(Text("legacy@%"))]]
  quit(switched)?
  let direct = connect(at(NATIVE), legacy())?
  assert query(direct, "SELECT CURRENT_USER(), DATABASE()")?.rows == [[Some(Text("legacy@%")), Some(Text("shop"))]]
  quit(direct)?
  let secure = connect_tls(at(NATIVE), legacy())?
  assert secure.secure && ping(secure) == Ok(())
  quit(secure)?
}

test "caching_sha2_password after an authentication switch from mysql_native_password" {
  let root = connect(at(NATIVE), account("root", "root-pw"))?
  _ = query(root, "FLUSH PRIVILEGES")?
  quit(root)?
  assert failure(connect(at(NATIVE), account("reader", "reader-pw"))) == NEEDS_TLS
  let secure = connect_tls(at(NATIVE), account("reader", "reader-pw"))?
  assert ping(secure) == Ok(())
  quit(secure)?
  let plain = connect(at(NATIVE), account("reader", "reader-pw"))?
  assert query(plain, "SELECT CURRENT_USER()")?.rows == [[Some(Text("reader@%"))]]
  quit(plain)?
}

test "refusals: a wrong password, another plugin, a certificate no authority signed, another host name, a closed port" {
  let wrong = connect(at(SHA2), account("legacy", "not-the-password"))
  assert match wrong {
    Err(Server(failed)) => failed.code == 1045 && failed.sql_state == "28000" && contains(failed.message, "legacy")
    _ => false
  }
  assert failure(connect(at(SHA2), account("legacy", "not-the-password"))) == "server error 1045 (28000)"
  assert failure(connect(at(NATIVE), account("reader", "not-the-password"))) == NEEDS_TLS
  assert failure(connect_tls(at(SHA2), account("reader", "not-the-password"))) == "server error 1045 (28000)"
  assert failure(connect(at(SHA2), account("old", "old-pw"))) == "authentication plugin sha256_password is not supported"
  let unknown = MysqlOptions { user = "legacy", password = "legacy-pw", server_name = "localhost" }
  assert contains(failure(connect_tls(at(SHA2), unknown)), "certificate refused")
  let unnamed = MysqlOptions { user = "legacy", password = "legacy-pw", tls = TlsOptions { authorities = AUTHORITY } }
  assert contains(failure(connect_tls(at(SHA2), unnamed)), "certificate refused")
  assert failure(connect(at(DEAD), legacy())) != "no error"
  assert code(connect(at(SHA2), MysqlOptions { user = "legacy", password = "legacy-pw", database = "elsewhere" })) == 1044
}

test "every column type decodes alike from query and from execute" {
  let client = connect(at(SHA2), legacy())?
  let texts = query(client, "SELECT * FROM things ORDER BY id")?
  let binaries = execute_once(client, "SELECT * FROM things ORDER BY id", [])?
  let expected = [
    Some(Int(1)), Some(Bool(true)), Some(Int(-128)), Some(Int(255)), Some(Int(-32768)), Some(Int(-8388608)), Some(Int(-2147483648)),
    Some(Int(9223372036854775807)), Some(Decimal("18446744073709551615")), Some(Int(2024)), Some(Decimal("-1234.50")),
    Some(Decimal("1.5")), Some(Decimal("0.1")), Some(Text("Zoë")), Some(Text("ab")), Some(Text("long text")),
    Some(Bytes(b"\x00\xff")), Some(Bytes(b"\x01\x02")), Some(Bool(true)), Some(Bytes(b"\x0a\x5a")), Some(Text(r#"{"a": 1}"#)),
    Some(Text("2024-02-29")), Some(Text("2024-02-29 23:59:58.123456")), Some(Text("2024-02-29 23:59:58")), Some(Text("-34:11:12")),
    Some(Text("glad")), Some(Text("a,b"))
  ]
  assert texts.columns.len() == 27 && binaries.columns.len() == 27
  assert texts.rows.len() == 2 && binaries.rows.len() == 2
  var i = 0
  while i < expected.len() {
    let name = texts.columns[i].name
    assert texts.rows[0][i] == expected[i], "query, column {name}"
    assert binaries.rows[0][i] == expected[i], "execute, column {name}"
    i = i + 1
  }
  assert texts.rows == binaries.rows
  assert texts.rows[1][0] == Some(Int(2)) && texts.rows[1][1] == None && texts.rows[1][26] == None
  let shortest = execute_once(client, "SELECT CAST(1.1 AS FLOAT), 1e0/3, -1e21, CAST(0 AS DOUBLE)", [])?
  assert shortest.rows == [[Some(Decimal("1.1")), Some(Decimal("0.3333333333333333")), Some(Decimal("-1e21")), Some(Decimal("0"))]]
  // The server prints a FLOAT with six significant digits; its four bytes hold more.
  let wide = "SELECT CAST(16777216 AS FLOAT)"
  assert query(client, wide)?.rows == [[Some(Decimal("16777200"))]]
  assert execute_once(client, wide, [])?.rows == [[Some(Decimal("16777216"))]]
  quit(client)?
}

test "a parameter is a value: it selects by equality and never changes the statement" {
  let client = connect(at(SHA2), legacy())?
  let found = execute_once(client, "SELECT id FROM things WHERE id = ? AND name = ?", [Some(Int(1)), Some(Text("Zoë"))])?
  assert found.rows == [[Some(Int(1))]]
  let hostile = execute_once(client, "SELECT id FROM things WHERE name = ?", [Some(Text("x' OR '1'='1"))])?
  assert hostile.columns.len() == 1 && hostile.rows.is_empty()
  let statement = prepare(client, "SELECT ?, ?, ?, ?, ?, ?")?
  assert statement.parameters == 6 && statement.columns == 6
  let values = [Some(Int(-2)), None, Some(Text("O'Reilly")), Some(Bytes(b"\x00\x01")), Some(Bool(true)), Some(Decimal("12.50"))]
  let echoed = execute(client, statement, values)?
  assert echoed.rows == [[Some(Int(-2)), None, Some(Text("O'Reilly")), Some(Bytes(b"\x00\x01")), Some(Int(1)), Some(Decimal("12.50"))]]
  assert execute(client, statement, []) == Err(WrongParameterCount(6, 0))
  close(client, statement)?
  assert code(execute(client, statement, values)) == 1243
  assert failure(query(client, "SELEC 1")) == "server error 1064 (42000)"
  assert ping(client) == Ok(())
  quit(client)?
}

test "a read-only session reads and refuses a write" {
  let client = connect(at(SHA2), MysqlOptions { user = "legacy", password = "legacy-pw", database = "shop", read_only = true })?
  assert query(client, "SELECT COUNT(*) FROM things")?.rows == [[Some(Int(2))]]
  assert match query(client, "INSERT INTO things (id) VALUES (3)") {
    Err(Server(failed)) => failed.code == 1792 && failed.sql_state == "25006"
    _ => false
  }
  assert code(execute_once(client, "INSERT INTO things (id) VALUES (?)", [Some(Int(3))])) == 1792
  assert query(client, "SELECT COUNT(*) FROM things")?.rows == [[Some(Int(2))]]
  quit(client)?
}

test fn asks(pool: Pool<TcpStream>, done: Sender<Bool>) -> Result<Unit, IoError> {
  done.send(match pool_execute(pool, "SELECT ? + 1", [Some(Int(41))]) {
    Ok(result) => result.rows == [[Some(Int(42))]]
    Err(_) => false
  })
  Ok(())
}

test "a pool shares two connections among tasks and takes a failed connection out" {
  let pool = pool_of([connect(at(NATIVE), legacy())?, connect(at(NATIVE), legacy())?], 2 seconds)
  let (done, results) = Channel.bounded(3)
  spawn asks(pool, done)
  spawn asks(pool, done)
  spawn asks(pool, done)
  assert results.receive() == Some(true) && results.receive() == Some(true) && results.receive() == Some(true)
  assert pool_query(pool, "SELECT 1")?.rows == [[Some(Int(1))]]
  let doomed = checkout(pool)?
  assert code(query(doomed, "KILL CONNECTION_ID()")) == 1317
  let other = checkout(pool)?
  checkin(pool, doomed)
  assert failure(pool_query(pool, "SELECT 1")) != "no error"
  assert failure(pool_query(pool, "SELECT 1")) == "no connection of the pool was free in time"
  checkin(pool, other)
  assert pool_query(pool, "SELECT 1")?.rows == [[Some(Int(1))]]
  quit_idle(pool)
}

test "a statement that does not answer within the read timeout ends the connection" {
  let client = connect(at(SHA2), MysqlOptions { user = "legacy", password = "legacy-pw", read_timeout = 1 seconds })?
  assert query(client, "SELECT SLEEP(4)") == Err(Closed)
  assert failure(ping(client)) == "write to a closed stream"
}
"##;

/// The number of tests in [`PROGRAM`].
const CASES: usize = 9;

fn docker(args: &[&str]) -> Output {
    Command::new("docker")
        .args(args)
        .output()
        .expect("start docker")
}

/// Removes the container when the test ends, passed or not.
struct Container(String);

impl Drop for Container {
    fn drop(&mut self) {
        let _ = docker(&["rm", "-f", "-v", &self.0]);
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind a free port")
        .local_addr()
        .expect("local address")
        .port()
}

/// The directory of this run, with what the containers mount: the script
/// that initialises a server and the files of its TLS identity. Returns
/// it and the authority's certificate.
fn prepare() -> (PathBuf, String) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("mysql");
    let _ = std::fs::remove_dir_all(&dir);
    let certs = dir.join("certs");
    std::fs::create_dir_all(&certs).expect("create the test directory");
    let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params
        .distinguished_name
        .push(DnType::CommonName, "ignite test authority");
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    let issuer = CertifiedIssuer::self_signed(params, KeyPair::generate().unwrap()).unwrap();
    let key = KeyPair::generate().unwrap();
    let server = CertificateParams::new(vec!["localhost".to_string()])
        .unwrap()
        .signed_by(&key, &issuer)
        .unwrap();
    std::fs::write(certs.join("ca.pem"), issuer.pem()).unwrap();
    std::fs::write(certs.join("server.pem"), server.pem()).unwrap();
    std::fs::write(certs.join("server-key.pem"), key.serialize_pem()).unwrap();
    std::fs::write(dir.join("init.sql"), INIT).unwrap();
    (dir, issuer.pem())
}

/// Starts a server of `image` on the loopback port `port`, with `extra`
/// after the server's TLS options.
fn server(image: &str, name: &str, port: u16, dir: &Path, extra: &[&str]) -> Container {
    let container = format!("ignite-mysql-{}-{name}", std::process::id());
    let publish = format!("127.0.0.1:{port}:3306");
    let init = format!(
        "{}:/docker-entrypoint-initdb.d/init.sql:ro",
        dir.join("init.sql").display()
    );
    let certs = format!("{}:/certs:ro", dir.join("certs").display());
    let mut args = vec!["run", "-d", "--rm", "--name", &container];
    let label = std::env::var("IGNITE_CONTAINER_LABEL").ok();
    if let Some(label) = &label {
        args.extend(["--label", label]);
    }
    args.extend([
        "-p",
        &publish,
        "-e",
        "MYSQL_ROOT_PASSWORD=root-pw",
        "-v",
        &init,
        "-v",
        &certs,
        image,
        "--ssl-ca=/certs/ca.pem",
        "--ssl-cert=/certs/server.pem",
        "--ssl-key=/certs/server-key.pem",
    ]);
    args.extend(extra);
    let output = docker(&args);
    assert!(
        output.status.success(),
        "docker {}\nstderr:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    Container(container)
}

/// Waits until the server behind `port` sends its handshake: the entry
/// point of the image first runs a server without a network to initialise
/// the data directory, and the published port answers only after it.
fn ready(container: &Container, port: u16) {
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let mut head = [0u8; 5];
            if stream.read_exact(&mut head).is_ok() && head[4] == 10 {
                return;
            }
        }
        if Instant::now() >= deadline {
            let logs = docker(&["logs", "--tail", "40", &container.0]);
            panic!(
                "{} sends no handshake on {port}:\n{}",
                container.0,
                String::from_utf8_lossy(&logs.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Runs [`PROGRAM`] on `target` and returns what `ignite test` printed.
fn run(target: &str, dir: &Path, source: &str) -> String {
    let dir = dir.join(target);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("real.ignite");
    std::fs::write(&file, source).expect("write the program");
    let output = Command::new(env!("CARGO_BIN_EXE_ignite"))
        .arg("test")
        .arg(&file)
        .arg(format!("--target={target}"))
        .arg("--out")
        .arg(dir.join("out"))
        .arg("--target-dir")
        .arg(dir.join("cargo"))
        .arg("--runtime")
        .arg(common::runner_archive())
        .args(["--timeout", "120"])
        .output()
        .expect("run ignite test");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{target}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains(&format!("test result: ok. {CASES} passed; 0 failed;")),
        "{target}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    stdout
}

/// One test for both servers and both targets, one after the other: the
/// servers start once.
#[test]
fn real_servers_with_both_plugins_on_both_targets() {
    let image = std::env::var("IGNITE_MYSQL_IMAGE").unwrap_or_else(|_| IMAGE.to_string());
    let runtime = Command::new("docker").arg("version").output();
    if !runtime.is_ok_and(|output| output.status.success()) {
        eprintln!(
            "mysql: skipped, no container runtime (`docker version` fails); std/protocol/mysql.ignite's inline tests cover the client over a scripted server"
        );
        return;
    }
    if !docker(&["image", "inspect", &image]).status.success() {
        eprintln!(
            "mysql: skipped, the image {image} is not present (`docker pull {image}` fetches it); std/protocol/mysql.ignite's inline tests cover the client over a scripted server"
        );
        return;
    }
    let (dir, authority) = prepare();
    let (sha2, native, dead) = (free_port(), free_port(), free_port());
    let first = server(&image, "sha2", sha2, &dir, &[]);
    let second = server(
        &image,
        "native",
        native,
        &dir,
        &["--default-authentication-plugin=mysql_native_password"],
    );
    ready(&first, sha2);
    ready(&second, native);
    let source = PROGRAM
        .replace("@SHA2@", &sha2.to_string())
        .replace("@NATIVE@", &native.to_string())
        .replace("@DEAD@", &dead.to_string())
        .replace("@AUTHORITY@", &authority);
    for target in ["llvm", "rust"] {
        let printed = run(target, &dir, &source);
        println!("mysql: {image}, target {target}\n{printed}");
    }
}
