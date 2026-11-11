//! Mode `quality-remote` (`docs/quality.md`, `task quality-remote`): the timing rows of the
//! quality metrics (5.3, 6.1, 6.2) measured on a single-tenant EC2 bare-metal host made for
//! the run and terminated after it.
//!
//! The mode drives the `aws` CLI with the profile `--profile` (or `IGNITE_PERF_AWS_PROFILE`;
//! there is no default) and the region `--region` (or `IGNITE_PERF_AWS_REGION`, else the
//! CLI's own). It launches one spot `--instance-type` (default `c7i.metal-24xl`) in the zone
//! with the lowest spot price, on-demand only when no zone has spot capacity, with
//! instance-initiated shutdown set to terminate and a `shutdown -h` timer of `--max-hours` in
//! its user-data, so the instance ends itself when the mode cannot. A security group that
//! admits SSH from the operator's address only and an ed25519 key pair are made for the run
//! and deleted after it; every resource carries the tag `ignite-quality-run=<run id>`.
//!
//! On the host: the trees of BASE and HEAD (`git archive`), each with its compiler and runtime
//! built, and a third tree, the commit of the checkout running the mode, whose `ignite-perf`
//! runs `quality --timing-only` over each tree (so BASE and HEAD may predate this mode): one
//! discarded warm-up of HEAD, then `--passes` passes (default 5) in ABBA order (base, head;
//! head, base; …). The values are fetched, the instance terminated, and the mode checks by the run tag
//! that no instance, security group or key pair is left; any one left is an error. The
//! instance, group and key pair are cleaned up on every exit path: success, failure and an
//! interrupt (SIGINT, SIGTERM, SIGHUP; the commands it starts run in their own process
//! group, so a terminal's Ctrl+C reaches only the mode).
//!
//! Each timing row becomes one record of `quality/history.jsonl`: the median of HEAD's
//! passes, `host` `ec2:<type>`, `machine` (type, CPU model, kernel, SMT and turbo as read on
//! the host, the market, price, instance-hours from the mode's own clock and their cost), and
//! BASE's median as `base_value`. The record names no instance, account or zone. With
//! `--run vX.Y.Z` the run is a release: a row whose HEAD median is worse than BASE's (the last
//! release, measured on the same host in the same run) beyond the metric's tolerance in
//! `quality/baseline.json`, or worse in every pass than every pass of BASE, refuses it, and the
//! records are labelled `run`. A cell of the size row (3.9) that is worse is held by a
//! `decision` on its bar and printed instead ([`judge`]).
//!
//! `--dry-run` makes the read-only calls, makes every call that creates with EC2's
//! `--dry-run` (the answer `DryRunOperation` is success) and prints the rest without making
//! them.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Args;

use crate::cli::default_repo;
use crate::json::{Json, obj};
use crate::perf::{R, io_err, path_str};

/// The tag every resource of a run carries, with the run id as its value.
pub const RUN_TAG: &str = "ignite-quality-run";
const PURPOSE: &str = "ignite-quality-timing";
const AMI_PARAMETER: &str =
    "/aws/service/canonical/ubuntu/server/24.04/stable/current/amd64/hvm/ebs-gp3/ami-id";
const USER: &str = "ubuntu";
const CAPACITY_ERRORS: [&str; 6] = [
    "InsufficientInstanceCapacity",
    "InsufficientCapacity",
    "SpotMaxPriceTooLow",
    "MaxSpotInstanceCountExceeded",
    "Unsupported",
    "InsufficientFreeAddressesInSubnet",
];
const SPOT: &str = r#"{"MarketType":"spot","SpotOptions":{"SpotInstanceType":"one-time","InstanceInterruptionBehavior":"terminate"}}"#;

#[derive(Args, Debug, Clone)]
pub struct RemoteArgs {
    #[arg(long, default_value_os_t = default_repo())]
    pub repo: PathBuf,
    /// The revision measured as the reference (the last release for a release run).
    #[arg(long)]
    pub base: String,
    /// The revision measured and recorded.
    #[arg(long)]
    pub head: String,
    #[arg(long, default_value = "c7i.metal-24xl")]
    pub instance_type: String,
    /// The AWS CLI profile; else `IGNITE_PERF_AWS_PROFILE`. Required, no default.
    #[arg(long)]
    pub profile: Option<String>,
    /// The AWS region; else `IGNITE_PERF_AWS_REGION`, else the CLI's configured region.
    #[arg(long)]
    pub region: Option<String>,
    /// Hours after which the instance shuts down (and so terminates) by itself.
    #[arg(long, default_value_t = 3.0)]
    pub max_hours: f64,
    /// Measured passes of each tree after the warm-up, interleaved ABBA.
    #[arg(long, default_value_t = 5)]
    pub passes: usize,
    #[arg(long, default_value_t = 15)]
    pub check_runs: usize,
    #[arg(long, default_value_t = 3)]
    pub build_runs: usize,
    /// Passes of the native benchmark, servers in ABBA order (`bench.rs`).
    #[arg(long, default_value_t = 5)]
    pub bench_passes: usize,
    /// Print the aws calls; make the creating ones with EC2's `--dry-run`.
    #[arg(long)]
    pub dry_run: bool,
    /// The run's local directory (key, user-data, fetched values); default
    /// `$XDG_CACHE_HOME/ignite/quality-remote/<run id>`.
    #[arg(long)]
    pub state: Option<PathBuf>,
    /// `bar`, `vX.Y.Z` (a release, refused when worse than BASE) or `run`.
    #[arg(long, default_value = "run")]
    pub run: String,
}

/// The profile of a run: the flag, else the environment's `IGNITE_PERF_AWS_PROFILE`.
pub fn profile(args: &RemoteArgs, env: Option<String>) -> R<String> {
    args.profile
        .clone()
        .or(env)
        .filter(|p| !p.is_empty())
        .ok_or_else(|| {
            "quality-remote needs an AWS profile: --profile or IGNITE_PERF_AWS_PROFILE".to_string()
        })
}

/// One finished command.
#[derive(Clone, Debug)]
pub struct Ran {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Where the mode's commands run, and its clock: the system in a real run, a script in the
/// tests.
pub trait Shell {
    fn run(&mut self, argv: &[String]) -> Ran;
    fn sleep(&mut self, secs: u64);
    fn now(&mut self) -> f64;
    fn log(&mut self, line: &str);
}

/// The system: each command in its own process group, stdin from `/dev/null`.
pub struct System;

impl Shell for System {
    fn run(&mut self, argv: &[String]) -> Ran {
        use std::os::unix::process::CommandExt as _;
        match Command::new(&argv[0])
            .args(&argv[1..])
            .stdin(Stdio::null())
            .process_group(0)
            .output()
        {
            Ok(out) => Ran {
                code: out.status.code().unwrap_or(-1),
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            },
            Err(error) => Ran {
                code: -1,
                stdout: String::new(),
                stderr: format!("{}: {error}", argv[0]),
            },
        }
    }
    fn sleep(&mut self, secs: u64) {
        for _ in 0..secs {
            if interrupted() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    fn now(&mut self) -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }
    fn log(&mut self, line: &str) {
        let secs = self.now() as u64;
        eprintln!(
            "{:02}:{:02}:{:02}Z {line}",
            secs / 3600 % 24,
            secs / 60 % 60,
            secs % 60
        );
    }
}

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: i32) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

/// Whether SIGINT, SIGTERM or SIGHUP arrived since [`catch_signals`].
pub fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

/// Turn SIGINT, SIGTERM and SIGHUP into [`interrupted`], so the run stops at its next step
/// and still cleans up; a signal during the cleanup changes nothing.
pub fn catch_signals() -> R<()> {
    use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};
    let action = SigAction::new(
        SigHandler::Handler(on_signal),
        SaFlags::empty(),
        SigSet::empty(),
    );
    for signal in [Signal::SIGINT, Signal::SIGTERM, Signal::SIGHUP] {
        // SAFETY: the handler only stores into an atomic, which is async-signal-safe.
        unsafe { sigaction(signal, &action) }.map_err(|e| format!("sigaction: {e}"))?;
    }
    Ok(())
}

/// Everything a run needs, resolved.
#[derive(Clone, Debug)]
pub struct Settings {
    pub run_id: String,
    pub instance_type: String,
    pub profile: String,
    pub region: Option<String>,
    pub max_hours: f64,
    pub passes: usize,
    pub check_runs: usize,
    pub build_runs: usize,
    pub bench_passes: usize,
    pub dry_run: bool,
    pub state: PathBuf,
    pub repo: PathBuf,
    pub base: String,
    pub head: String,
    /// The commit whose `ignite-perf` runs on the host: the checkout running the mode.
    pub harness: String,
}

/// How a run ended on the host.
#[derive(Clone, Debug)]
pub struct Outcome {
    /// The host script's exit status; 0 for a dry run.
    pub status: i32,
}

/// `'text'` for a POSIX shell.
fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

fn words(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// The user-data: the shutdown timer first, then a quiet host with a C toolchain, clang from
/// apt.llvm.org (the newest of the listed majors it has), Rust stable for `ubuntu`, SMT and
/// turbo off and the governor `performance` where the instance allows it.
pub fn user_data(max_minutes: u64) -> String {
    include_str!("remote-user-data.sh").replace("@MAX_MINUTES@", &max_minutes.to_string())
}

/// The trees in the order the host measures them: one discarded warm-up of HEAD (pass 0),
/// then `passes` passes in ABBA order.
pub fn schedule(passes: usize) -> Vec<(&'static str, usize)> {
    let mut order = vec![("head", 0)];
    for pass in 1..=passes {
        let trees = if pass % 2 == 1 {
            ["base", "head"]
        } else {
            ["head", "base"]
        };
        order.extend(trees.into_iter().map(|tree| (tree, pass)));
    }
    order
}

/// The one command that starts the host's part, detached: build the harness tree (the commit of
/// the checkout running the mode), then its `ignite-perf quality-host` over `~/q`. Only the
/// started process is in the background, in a session of its own with every stream
/// redirected, so the SSH channel closes at once.
pub fn host_command(
    passes: usize,
    check_runs: usize,
    build_runs: usize,
    bench_passes: usize,
) -> String {
    let inner = format!(
        ". \"$HOME/.cargo/env\" && cd harness && CARGO_INCREMENTAL=0 cargo build -q --locked -p ignite-perf \
         && target/debug/ignite-perf quality-host --root \"$HOME/q\" --passes {passes} \
         --check-runs {check_runs} --build-runs {build_runs} --bench-passes {bench_passes}; echo $? > \"$HOME/q/run.exit\""
    );
    format!(
        "cd q && rm -f run.exit && {{ setsid nohup sh -c {} > run.log 2>&1 < /dev/null & }}",
        quote(&inner)
    )
}

#[derive(Args, Debug, Clone)]
pub struct HostArgs {
    /// The directory holding the trees `base` and `head`; `out` and `work` are made in it.
    #[arg(long)]
    pub root: PathBuf,
    #[arg(long, default_value_t = 5)]
    pub passes: usize,
    #[arg(long, default_value_t = 15)]
    pub check_runs: usize,
    #[arg(long, default_value_t = 3)]
    pub build_runs: usize,
    /// Only the timing rows: leave out the native benchmark (`bench.rs`).
    #[arg(long)]
    pub no_bench: bool,
    #[command(flatten)]
    pub bench: crate::bench::BenchArgs,
}

fn read_trimmed(path: &str, absent: &str) -> String {
    fs::read_to_string(path).map_or_else(|_| absent.to_string(), |t| t.trim().to_string())
}

fn first_line(argv: &[&str]) -> String {
    Command::new(argv[0])
        .args(&argv[1..])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .to_string()
        })
        .unwrap_or_default()
}

/// The state of the machine this runs on: CPU model, kernel, SMT, turbo, governors and the
/// toolchain, as read from `/proc` and `/sys`.
pub fn host_state() -> Json {
    let cpu = "/sys/devices/system/cpu";
    let model = fs::read_to_string("/proc/cpuinfo")
        .unwrap_or_default()
        .lines()
        .find_map(|l| {
            l.strip_prefix("model name")
                .map(|r| r.trim_start_matches([' ', '\t', ':']).to_string())
        })
        .unwrap_or_default();
    let mut governors: Vec<String> = fs::read_dir(cpu)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| fs::read_to_string(e.path().join("cpufreq/scaling_governor")).ok())
                .map(|g| g.trim().to_string())
                .collect()
        })
        .unwrap_or_default();
    governors.sort();
    governors.dedup();
    let nproc = std::thread::available_parallelism().map_or(0, |n| n.get());
    obj(vec![
        ("cpu", Json::from(model)),
        (
            "kernel",
            Json::from(read_trimmed("/proc/sys/kernel/osrelease", "")),
        ),
        ("nproc", Json::from(nproc.to_string())),
        (
            "online_cpus",
            Json::from(read_trimmed(&format!("{cpu}/online"), "")),
        ),
        (
            "smt_control",
            Json::from(read_trimmed(&format!("{cpu}/smt/control"), "unknown")),
        ),
        (
            "smt_active",
            Json::from(read_trimmed(&format!("{cpu}/smt/active"), "unknown")),
        ),
        (
            "intel_pstate_no_turbo",
            Json::from(read_trimmed(
                &format!("{cpu}/intel_pstate/no_turbo"),
                "absent",
            )),
        ),
        (
            "cpufreq_boost",
            Json::from(read_trimmed(&format!("{cpu}/cpufreq/boost"), "absent")),
        ),
        (
            "scaling_driver",
            Json::from(read_trimmed(
                &format!("{cpu}/cpu0/cpufreq/scaling_driver"),
                "none",
            )),
        ),
        ("governors", Json::from(governors.join(","))),
        ("clang", Json::from(first_line(&["clang", "--version"]))),
        ("rustc", Json::from(first_line(&["rustc", "--version"]))),
        ("go", Json::from(first_line(&["go", "version"]))),
        ("loadavg", Json::from(read_trimmed("/proc/loadavg", ""))),
    ])
}

fn cargo(dir: &Path, args: &[&str]) -> R<()> {
    let status = Command::new("cargo")
        .args(args)
        .current_dir(dir)
        .env("CARGO_INCREMENTAL", "0")
        .status()
        .map_err(io_err("cargo"))?;
    crate::perf::ensure(
        status.success(),
        format!("cargo {} in {} failed", args.join(" "), dir.display()),
    )
}

/// `ignite-perf quality-host`, the part of a run on the host: the compiler and runtime of BASE
/// and HEAD, `out/host-state.json`, then
/// `quality --timing-only` of each tree in the order of [`schedule`] into
/// `out/<tree>-<pass>.json`, then (unless `--no-bench`) the native benchmark
/// ([`crate::bench::host`]).
pub fn host_main(args: &HostArgs) -> R<()> {
    let root = &args.root;
    let out = root.join("out");
    fs::create_dir_all(&out).map_err(io_err(out.display()))?;
    for tree in ["base", "head"] {
        cargo(
            &root.join(tree),
            &["build", "-q", "--locked", "-p", "ignite"],
        )?;
        cargo(
            &root.join(tree),
            &[
                "build",
                "-q",
                "--locked",
                "-p",
                "ignite-rt",
                "--features",
                "runner",
                "--target-dir",
                "target/ignite-test/rt",
            ],
        )?;
    }
    let state = host_state();
    println!("host state: {}", state.dumps(false));
    crate::perf::dump(&out.join("host-state.json"), &state)?;
    for (tree, pass) in schedule(args.passes) {
        let repo = root.join(tree);
        println!("measure {tree} {pass}: load {:.2}", crate::perf::loadavg()?);
        crate::quality::measure(&crate::quality::QualityArgs {
            cli: Some(repo.join("target/debug/ignite")),
            runtime: repo.join("target/ignite-test/rt/debug/libignite_rt.a"),
            work: root.join("work").join(tree),
            out: out.join(format!("{tree}-{pass}.json")),
            check_runs: args.check_runs,
            build_runs: args.build_runs,
            timing_only: true,
            repo,
        })?;
    }
    if !args.no_bench {
        crate::bench::host(root, &out, &args.bench)?;
    }
    Ok(())
}

/// The zones of `prices` cheapest first, ties by name, then the zones without a price.
fn zone_order(prices: &BTreeMap<String, f64>, offered: &[String]) -> Vec<String> {
    let mut priced: Vec<(&String, f64)> = prices.iter().map(|(z, p)| (z, *p)).collect();
    priced.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(b.0)));
    let mut order: Vec<String> = priced.into_iter().map(|(z, _)| z.clone()).collect();
    for zone in offered {
        if !order.contains(zone) {
            order.push(zone.clone());
        }
    }
    order
}

fn tag_list(run_id: &str) -> String {
    format!(
        "{{Key={RUN_TAG},Value={run_id}}},{{Key=purpose,Value={PURPOSE}}},{{Key=Name,Value=ignite-quality-{run_id}}}"
    )
}

/// One EC2 call: read-only, or one that changes something (made with `--dry-run` in a dry
/// run when `dry_flag`, else only printed).
enum Kind {
    Read,
    Change { dry_flag: bool },
}

pub struct Remote<'a> {
    s: Settings,
    shell: &'a mut dyn Shell,
    planned: Vec<Vec<String>>,
    vpc: String,
    subnets: BTreeMap<String, String>,
    zones: Vec<String>,
    spot_prices: BTreeMap<String, f64>,
    ami: String,
    root_device: String,
    my_ip: String,
    group: Option<String>,
    key_created: bool,
    instance: Option<String>,
    address: String,
    launched_at: Option<f64>,
    ended_at: Option<f64>,
    info: Json,
}

impl<'a> Remote<'a> {
    pub fn new(settings: Settings, shell: &'a mut dyn Shell) -> Remote<'a> {
        Remote {
            s: settings,
            shell,
            planned: Vec::new(),
            vpc: String::new(),
            subnets: BTreeMap::new(),
            zones: Vec::new(),
            spot_prices: BTreeMap::new(),
            ami: String::new(),
            root_device: String::new(),
            my_ip: String::new(),
            group: None,
            key_created: false,
            instance: None,
            address: String::new(),
            launched_at: None,
            ended_at: None,
            info: obj(Vec::<(&str, Json)>::new()),
        }
    }

    /// Every call that creates, changes or deletes, made or (in a dry run) only planned.
    pub fn planned(&self) -> &[Vec<String>] {
        &self.planned
    }

    /// The run's facts: market, zone price, the host state, instance-hours and cost.
    pub fn info(&self) -> &Json {
        &self.info
    }

    fn log(&mut self, line: &str) {
        self.shell.log(line);
    }

    fn key_name(&self) -> String {
        format!("ignite-quality-{}", self.s.run_id)
    }

    fn key_path(&self) -> PathBuf {
        self.s.state.join("key")
    }

    fn check_interrupt(&self) -> R<()> {
        if interrupted() {
            Err("interrupted".to_string())
        } else {
            Ok(())
        }
    }

    fn argv(&self, args: &[String]) -> Vec<String> {
        let mut argv = words(&["aws", "--profile", &self.s.profile]);
        if let Some(region) = &self.s.region {
            argv.extend(words(&["--region", region]));
        }
        argv.extend(words(&["--output", "json"]));
        argv.extend(args.iter().cloned());
        argv
    }

    fn aws(&mut self, args: &[String], kind: Kind) -> R<Option<Json>> {
        let mut argv = self.argv(args);
        if let Kind::Change { dry_flag } = kind {
            self.planned.push(argv.clone());
            if self.s.dry_run {
                if !dry_flag {
                    self.log(&format!("dry run, not made: {}", argv.join(" ")));
                    return Ok(None);
                }
                argv.push("--dry-run".to_string());
                self.log(&format!("dry run: {}", argv.join(" ")));
                let ran = self.shell.run(&argv);
                if !ran.stderr.contains("DryRunOperation") {
                    return Err(aws_error(&argv, &ran));
                }
                return Ok(None);
            }
        }
        self.log(&argv.join(" "));
        let ran = self.shell.run(&argv);
        if ran.code != 0 {
            return Err(aws_error(&argv, &ran));
        }
        if ran.stdout.trim().is_empty() {
            return Ok(None);
        }
        Json::parse(&ran.stdout)
            .map(Some)
            .map_err(|e| format!("{}: {e}", argv.join(" ")))
    }

    fn read(&mut self, args: &[&str]) -> R<Json> {
        self.aws(&words(args), Kind::Read)?
            .ok_or_else(|| format!("aws {}: no output", args.join(" ")))
    }

    fn preflight(&mut self) -> R<()> {
        let identity = self.read(&["sts", "get-caller-identity"])?;
        let arn = identity.field("Arn").as_str().unwrap_or("?").to_string();
        self.log(&format!("run {} as {arn}", self.s.run_id));
        let ip = self
            .shell
            .run(&words(&["curl", "-sSf", "https://checkip.amazonaws.com"]));
        if ip.code != 0 {
            return Err(format!("reading this host's address: {}", ip.stderr.trim()));
        }
        self.my_ip = ip.stdout.trim().to_string();
        let vpcs = self.read(&[
            "ec2",
            "describe-vpcs",
            "--filters",
            "Name=is-default,Values=true",
        ])?;
        self.vpc = vpcs
            .field("Vpcs")
            .item(0)
            .field("VpcId")
            .as_str()
            .ok_or("no default VPC")?
            .to_string();
        let vpc_filter = format!("Name=vpc-id,Values={}", self.vpc);
        let subnets = self.read(&[
            "ec2",
            "describe-subnets",
            "--filters",
            &vpc_filter,
            "Name=default-for-az,Values=true",
        ])?;
        for subnet in subnets.field("Subnets").as_arr() {
            if let (Some(zone), Some(id)) = (
                subnet.field("AvailabilityZone").as_str(),
                subnet.field("SubnetId").as_str(),
            ) {
                self.subnets.insert(zone.to_string(), id.to_string());
            }
        }
        let type_filter = format!("Name=instance-type,Values={}", self.s.instance_type);
        let offered = self.read(&[
            "ec2",
            "describe-instance-type-offerings",
            "--location-type",
            "availability-zone",
            "--filters",
            &type_filter,
        ])?;
        let offered: Vec<String> = offered
            .field("InstanceTypeOfferings")
            .as_arr()
            .iter()
            .filter_map(|o| o.field("Location").as_str().map(str::to_string))
            .collect();
        let now = iso_now(self.shell.now());
        let instance_type = self.s.instance_type.clone();
        let history = self.read(&[
            "ec2",
            "describe-spot-price-history",
            "--instance-types",
            &instance_type,
            "--product-descriptions",
            "Linux/UNIX",
            "--start-time",
            &now,
        ])?;
        for entry in history.field("SpotPriceHistory").as_arr() {
            let zone = entry.field("AvailabilityZone").as_str().unwrap_or("");
            let price = entry
                .field("SpotPrice")
                .as_str()
                .and_then(|p| p.parse().ok());
            if let Some(price) = price
                && offered.iter().any(|z| z == zone)
            {
                self.spot_prices.entry(zone.to_string()).or_insert(price);
            }
        }
        self.zones = zone_order(&self.spot_prices, &offered)
            .into_iter()
            .filter(|z| self.subnets.contains_key(z))
            .collect();
        if self.zones.is_empty() {
            return Err(format!(
                "{instance_type} is offered in no zone with a default subnet"
            ));
        }
        self.log(&format!("spot prices USD/h: {:?}", self.spot_prices));
        self.ami = self
            .read(&["ssm", "get-parameter", "--name", AMI_PARAMETER])?
            .field("Parameter")
            .field("Value")
            .as_str()
            .ok_or("no AMI id")?
            .to_string();
        let ami = self.ami.clone();
        let image = self.read(&["ec2", "describe-images", "--image-ids", &ami])?;
        self.root_device = image
            .field("Images")
            .item(0)
            .field("RootDeviceName")
            .as_str()
            .ok_or("no root device")?
            .to_string();
        Ok(())
    }

    fn security_group(&mut self) -> R<()> {
        let name = format!("ignite-quality-{}", self.s.run_id);
        let tags = format!(
            "ResourceType=security-group,Tags=[{}]",
            tag_list(&self.s.run_id)
        );
        let created = self.aws(
            &words(&[
                "ec2",
                "create-security-group",
                "--group-name",
                &name,
                "--vpc-id",
                &self.vpc,
                "--description",
                "ignite quality timing run: SSH from the operator address only",
                "--tag-specifications",
                &tags,
            ]),
            Kind::Change { dry_flag: true },
        )?;
        self.group = created.and_then(|c| c.field("GroupId").as_str().map(str::to_string));
        let group = self
            .group
            .clone()
            .unwrap_or_else(|| "sg-dry-run".to_string());
        let cidr = format!("{}/32", self.my_ip);
        self.aws(
            &words(&[
                "ec2",
                "authorize-security-group-ingress",
                "--group-id",
                &group,
                "--protocol",
                "tcp",
                "--port",
                "22",
                "--cidr",
                &cidr,
            ]),
            Kind::Change {
                dry_flag: self.group.is_some(),
            },
        )?;
        Ok(())
    }

    fn key_pair(&mut self) -> R<()> {
        let tags = format!("ResourceType=key-pair,Tags=[{}]", tag_list(&self.s.run_id));
        let name = self.key_name();
        let created = self.aws(
            &words(&[
                "ec2",
                "create-key-pair",
                "--key-name",
                &name,
                "--key-type",
                "ed25519",
                "--tag-specifications",
                &tags,
            ]),
            Kind::Change { dry_flag: true },
        )?;
        let Some(created) = created else {
            return Ok(());
        };
        self.key_created = true;
        let material = created
            .field("KeyMaterial")
            .as_str()
            .ok_or("create-key-pair: no key material")?
            .to_string();
        use std::os::unix::fs::OpenOptionsExt as _;
        let path = self.key_path();
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .map_err(io_err(path.display()))?;
        file.write_all(material.as_bytes())
            .map_err(io_err(path.display()))?;
        Ok(())
    }

    fn run_instances_args(&self, zone: &str, spot: bool, user_data: &Path) -> Vec<String> {
        let tags = tag_list(&self.s.run_id);
        let mut args = words(&[
            "ec2",
            "run-instances",
            "--image-id",
            &self.ami,
            "--instance-type",
            &self.s.instance_type,
            "--count",
            "1",
        ]);
        if self.key_created {
            args.extend(words(&["--key-name", &self.key_name()]));
        }
        if let Some(group) = &self.group {
            args.extend(words(&["--security-group-ids", group]));
        }
        let devices = format!(
            r#"[{{"DeviceName":"{}","Ebs":{{"VolumeSize":100,"VolumeType":"gp3","Iops":6000,"Throughput":500,"DeleteOnTermination":true}}}}]"#,
            self.root_device
        );
        args.extend(words(&[
            "--subnet-id",
            &self.subnets[zone],
            "--instance-initiated-shutdown-behavior",
            "terminate",
            "--metadata-options",
            "HttpTokens=required",
            "--block-device-mappings",
            &devices,
            "--tag-specifications",
            &format!("ResourceType=instance,Tags=[{tags}]"),
            &format!("ResourceType=volume,Tags=[{tags}]"),
            "--user-data",
            &format!("file://{}", path_str(user_data)),
        ]));
        if spot {
            args.extend(words(&["--instance-market-options", SPOT]));
        }
        args
    }

    fn launch(&mut self) -> R<()> {
        let path = self.s.state.join("user-data.sh");
        fs::write(&path, user_data((self.s.max_hours * 60.0).round() as u64))
            .map_err(io_err(path.display()))?;
        let mut failures = Vec::new();
        let attempts: Vec<(String, bool)> = [true, false]
            .into_iter()
            .flat_map(|spot| self.zones.iter().map(move |z| (z.clone(), spot)))
            .collect();
        for (zone, spot) in attempts {
            let args = self.run_instances_args(&zone, spot, &path);
            let market = if spot { "spot" } else { "on-demand" };
            let launched = match self.aws(&args, Kind::Change { dry_flag: true }) {
                Ok(launched) => launched,
                Err(error) if CAPACITY_ERRORS.iter().any(|c| error.contains(c)) => {
                    let tail: String = error.chars().rev().take(200).collect();
                    let tail: String = tail.chars().rev().collect();
                    failures.push(format!("{zone} {market}: {tail}"));
                    self.log(&format!("no capacity: {zone} {market}"));
                    continue;
                }
                Err(error) => return Err(error),
            };
            self.info
                .set("instance_type", self.s.instance_type.as_str().into());
            self.info.set("market", market.into());
            if spot {
                self.info.set(
                    "usd_per_hour",
                    format!("{:.4}", self.spot_prices.get(&zone).copied().unwrap_or(0.0)).into(),
                );
                self.info.set(
                    "price_source",
                    "describe-spot-price-history at launch".into(),
                );
            }
            self.info.set("zone", zone.as_str().into());
            self.info.set(
                "launch_failures",
                Json::Arr(failures.iter().map(|f| f.as_str().into()).collect()),
            );
            let Some(launched) = launched else {
                return Ok(());
            };
            let id = launched
                .field("Instances")
                .item(0)
                .field("InstanceId")
                .as_str()
                .ok_or("run-instances: no instance id")?
                .to_string();
            self.launched_at = Some(self.shell.now());
            self.instance = Some(id.clone());
            self.log(&format!(
                "launched {id}: {market} {} in {zone}",
                self.s.instance_type
            ));
            return Ok(());
        }
        Err(format!(
            "no capacity in any zone, spot or on-demand: {}",
            failures.join("; ")
        ))
    }

    fn states(&mut self, ids: &[String]) -> R<BTreeMap<String, Json>> {
        let mut args = words(&["ec2", "describe-instances", "--instance-ids"]);
        args.extend(ids.iter().cloned());
        let described = self
            .aws(&args, Kind::Read)?
            .ok_or("describe-instances: no output")?;
        let mut out = BTreeMap::new();
        for reservation in described.field("Reservations").as_arr() {
            for instance in reservation.field("Instances").as_arr() {
                if let Some(id) = instance.field("InstanceId").as_str() {
                    out.insert(id.to_string(), instance.clone());
                }
            }
        }
        Ok(out)
    }

    fn wait_state(&mut self, ids: &[String], wanted: &str, minutes: u64, cleanup: bool) -> R<()> {
        let deadline = self.shell.now() + (minutes * 60) as f64;
        loop {
            if !cleanup {
                self.check_interrupt()?;
            }
            let states = self.states(ids)?;
            let names: Vec<&str> = ids
                .iter()
                .map(|id| {
                    states
                        .get(id)
                        .and_then(|i| i.field("State").field("Name").as_str())
                        .unwrap_or("?")
                })
                .collect();
            if names.iter().all(|n| *n == wanted) {
                return Ok(());
            }
            if wanted == "running"
                && names
                    .iter()
                    .any(|n| *n == "shutting-down" || *n == "terminated")
            {
                return Err(format!("{ids:?} ended before running: {names:?}"));
            }
            if self.shell.now() > deadline {
                return Err(format!(
                    "{ids:?} not {wanted} after {minutes} min: {names:?}"
                ));
            }
            self.shell.sleep(15);
        }
    }

    fn wait_running(&mut self) -> R<()> {
        let id = self.instance.clone().ok_or("no instance")?;
        self.wait_state(std::slice::from_ref(&id), "running", 30, false)?;
        let states = self.states(std::slice::from_ref(&id))?;
        self.address = states
            .get(&id)
            .and_then(|i| i.field("PublicIpAddress").as_str())
            .ok_or("the instance has no public address")?
            .to_string();
        self.log(&format!("{id} running at {}", self.address));
        Ok(())
    }

    fn ssh_argv(&self) -> Vec<String> {
        words(&[
            "ssh",
            "-i",
            &path_str(&self.key_path()),
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-o",
            &format!(
                "UserKnownHostsFile={}",
                path_str(&self.s.state.join("known_hosts"))
            ),
            "-o",
            "ServerAliveInterval=30",
            "-o",
            "ConnectTimeout=10",
            "-o",
            "BatchMode=yes",
            &format!("{USER}@{}", self.address),
        ])
    }

    fn ssh(&mut self, command: &str) -> Ran {
        let mut argv = self.ssh_argv();
        argv.push(command.to_string());
        self.shell.run(&argv)
    }

    fn ssh_ok(&mut self, command: &str) -> R<String> {
        let ran = self.ssh(command);
        if ran.code != 0 {
            return Err(format!(
                "ssh {command:?} exited {}: {}",
                ran.code,
                tail(&ran.stderr)
            ));
        }
        Ok(ran.stdout)
    }

    /// `pipeline` through bash with pipefail, the ssh command quoted in it as `{ssh}`.
    fn piped(&mut self, pipeline: &str) -> R<()> {
        let ssh: Vec<String> = self.ssh_argv().iter().map(|w| quote(w)).collect();
        let script = format!(
            "set -o pipefail; {}",
            pipeline.replace("{ssh}", &ssh.join(" "))
        );
        let ran = self.shell.run(&words(&["bash", "-c", &script]));
        if ran.code != 0 {
            return Err(format!(
                "{pipeline}: exited {}: {}",
                ran.code,
                tail(&ran.stderr)
            ));
        }
        Ok(())
    }

    fn wait_ssh(&mut self) -> R<()> {
        let deadline = self.shell.now() + 25.0 * 60.0;
        loop {
            self.check_interrupt()?;
            if self.ssh("true").code == 0 {
                self.log("ssh answers");
                return Ok(());
            }
            if self.shell.now() > deadline {
                return Err(format!(
                    "ssh to {} did not answer within 25 min",
                    self.address
                ));
            }
            self.shell.sleep(15);
        }
    }

    fn wait_setup(&mut self) -> R<()> {
        let deadline = self.shell.now() + 40.0 * 60.0;
        loop {
            self.check_interrupt()?;
            let ran = self.ssh(
                "if test -f /var/lib/ignite-quality/ready; then echo ready; \
                 elif cloud-init status 2>/dev/null | grep -q 'status: error'; then echo failed; \
                 else echo waiting; fi",
            );
            let status = ran.stdout.trim().to_string();
            if status == "ready" {
                self.log("host setup done");
                return Ok(());
            }
            if status == "failed" || self.shell.now() > deadline {
                let log = self
                    .ssh("sudo tail -n 60 /var/log/cloud-init-output.log")
                    .stdout;
                return Err(format!("host setup (user-data) {status}:\n{log}"));
            }
            self.shell.sleep(20);
        }
    }

    fn copy_tree(&mut self, commit: &str, name: &str) -> R<()> {
        let repo = quote(&path_str(&self.s.repo));
        let remote = format!("rm -rf q/{name} && mkdir -p q/{name} && tar -x -C q/{name}");
        self.piped(&format!(
            "git -C {repo} archive --format=tar {} | {{ssh}} {}",
            quote(commit),
            quote(&remote)
        ))?;
        self.log(&format!("copied {commit} to q/{name}"));
        Ok(())
    }

    fn measure(&mut self) -> R<i32> {
        let command = host_command(
            self.s.passes,
            self.s.check_runs,
            self.s.build_runs,
            self.s.bench_passes,
        );
        self.ssh_ok(&command)?;
        self.log("measuring on the host");
        let deadline = self.shell.now() + self.s.max_hours * 3600.0;
        let mut shown = 0usize;
        loop {
            self.shell.sleep(30);
            self.check_interrupt()?;
            let ran = self.ssh(&format!(
                "cd q && tail -n +{} run.log; echo; cat run.exit 2>/dev/null || echo running",
                shown + 1
            ));
            if ran.code != 0 {
                if self.shell.now() > deadline {
                    return Err(format!("lost the host: {}", tail(&ran.stderr)));
                }
                continue;
            }
            let lines: Vec<&str> = ran.stdout.trim_end_matches('\n').split('\n').collect();
            let status = lines
                .last()
                .copied()
                .unwrap_or("running")
                .trim()
                .to_string();
            let body = &lines[..lines.len().saturating_sub(2)];
            for line in body {
                self.log(&format!("host: {line}"));
            }
            shown += body.len();
            if status != "running" {
                self.log(&format!("host run exited {status}"));
                return status
                    .parse()
                    .map_err(|_| format!("run.exit holds {status:?}"));
            }
            if self.shell.now() > deadline {
                return Err(format!(
                    "the host run did not finish within {} h",
                    self.s.max_hours
                ));
            }
        }
    }

    fn fetch(&mut self) -> R<()> {
        let into = self.s.state.join("fetched");
        fs::create_dir_all(&into).map_err(io_err(into.display()))?;
        self.piped(&format!(
            "{{ssh}} 'tar -c -C q out run.log' | tar -x -C {}",
            quote(&path_str(&into))
        ))?;
        if let Ok(state) = crate::perf::read_json(&into.join("out/host-state.json")) {
            self.log(&format!("host state: {}", state.dumps(false)));
            self.info.set("host_state", state);
        }
        Ok(())
    }

    fn body(&mut self) -> R<i32> {
        self.security_group()?;
        self.key_pair()?;
        self.launch()?;
        if self.s.dry_run {
            self.log(&format!(
                "dry run: no instance launched; the run would copy {} and {}, and measure {} passes",
                self.s.base, self.s.head, self.s.passes
            ));
            return Ok(0);
        }
        self.check_interrupt()?;
        self.wait_running()?;
        self.wait_ssh()?;
        self.wait_setup()?;
        let (base, head) = (self.s.base.clone(), self.s.head.clone());
        self.copy_tree(&base, "base")?;
        self.copy_tree(&head, "head")?;
        let harness = self.s.harness.clone();
        self.copy_tree(&harness, "harness")?;
        self.check_interrupt()?;
        let status = self.measure()?;
        self.fetch()?;
        Ok(status)
    }

    fn tagged(&mut self, what: &str) -> R<Json> {
        let filter = format!("Name=tag:{RUN_TAG},Values={}", self.s.run_id);
        self.read(&["ec2", what, "--filters", &filter])
    }

    fn tagged_instances(&mut self) -> R<Vec<(String, String)>> {
        let described = self.tagged("describe-instances")?;
        let mut out = Vec::new();
        for reservation in described.field("Reservations").as_arr() {
            for instance in reservation.field("Instances").as_arr() {
                out.push((
                    instance
                        .field("InstanceId")
                        .as_str()
                        .unwrap_or("?")
                        .to_string(),
                    instance
                        .field("State")
                        .field("Name")
                        .as_str()
                        .unwrap_or("?")
                        .to_string(),
                ));
            }
        }
        Ok(out)
    }

    fn cleanup(&mut self) -> R<()> {
        if self.s.dry_run {
            let id = self.instance.clone().unwrap_or_else(|| "i-dry-run".into());
            let name = self.key_name();
            let group = self.group.clone().unwrap_or_else(|| "sg-dry-run".into());
            for args in [
                words(&["ec2", "terminate-instances", "--instance-ids", &id]),
                words(&["ec2", "delete-key-pair", "--key-name", &name]),
                words(&["ec2", "delete-security-group", "--group-id", &group]),
            ] {
                self.aws(&args, Kind::Change { dry_flag: false })?;
            }
            return Ok(());
        }
        let mut problems = Vec::new();
        let terminated = (|| -> R<()> {
            let listed = self.tagged_instances()?;
            let mut live: Vec<String> = listed
                .iter()
                .filter(|(_, state)| state != "terminated")
                .map(|(id, _)| id.clone())
                .collect();
            if let Some(id) = &self.instance
                && !listed.iter().any(|(l, _)| l == id)
            {
                live.push(id.clone());
            }
            if !live.is_empty() {
                let mut args = words(&["ec2", "terminate-instances", "--instance-ids"]);
                args.extend(live.iter().cloned());
                self.aws(&args, Kind::Change { dry_flag: false })?;
                self.wait_state(&live, "terminated", 30, true)?;
            }
            Ok(())
        })();
        self.ended_at = Some(self.shell.now());
        if let Err(error) = terminated {
            problems.push(error);
        }
        if self.key_created {
            let name = self.key_name();
            if let Err(error) = self.aws(
                &words(&["ec2", "delete-key-pair", "--key-name", &name]),
                Kind::Change { dry_flag: false },
            ) {
                problems.push(error);
            }
        }
        if let Some(group) = self.group.clone() {
            // The group stays in use until the instance's interface is gone.
            let mut tries = 0;
            loop {
                match self.aws(
                    &words(&["ec2", "delete-security-group", "--group-id", &group]),
                    Kind::Change { dry_flag: false },
                ) {
                    Ok(_) => break,
                    Err(error) if error.contains("DependencyViolation") && tries < 20 => {
                        tries += 1;
                        self.shell.sleep(15);
                    }
                    Err(error) => {
                        problems.push(error);
                        break;
                    }
                }
            }
        }
        let left: Vec<String> = self
            .tagged_instances()?
            .into_iter()
            .filter(|(_, state)| state != "terminated")
            .map(|(id, state)| format!("{id} {state}"))
            .collect();
        let groups = self.tagged("describe-security-groups")?;
        let groups: Vec<String> = groups
            .field("SecurityGroups")
            .as_arr()
            .iter()
            .map(|g| g.field("GroupId").as_str().unwrap_or("?").to_string())
            .collect();
        let keys = self.tagged("describe-key-pairs")?;
        let keys: Vec<String> = keys
            .field("KeyPairs")
            .as_arr()
            .iter()
            .map(|k| k.field("KeyName").as_str().unwrap_or("?").to_string())
            .collect();
        self.info.set(
            "left",
            obj(vec![
                (
                    "instances",
                    Json::Arr(left.iter().map(|s| s.as_str().into()).collect()),
                ),
                (
                    "security_groups",
                    Json::Arr(groups.iter().map(|s| s.as_str().into()).collect()),
                ),
                (
                    "key_pairs",
                    Json::Arr(keys.iter().map(|s| s.as_str().into()).collect()),
                ),
            ]),
        );
        if !left.is_empty() || !groups.is_empty() || !keys.is_empty() || !problems.is_empty() {
            return Err(format!(
                "run {}: NOT CLEAN: instances {left:?}, security groups {groups:?}, key pairs {keys:?}; errors {problems:?}. \
                 Check: aws ec2 describe-instances --filters Name=tag:{RUN_TAG},Values={}",
                self.s.run_id, self.s.run_id
            ));
        }
        self.log(&format!(
            "confirmed: nothing tagged {RUN_TAG}={} is left (instances, security groups, key pairs)",
            self.s.run_id
        ));
        Ok(())
    }

    fn on_demand_price(&mut self) {
        let Some(location) = self.s.region.as_deref().and_then(location) else {
            return;
        };
        let instance_type = self.s.instance_type.clone();
        let products = self.aws(
            &words(&[
                "pricing",
                "get-products",
                "--region",
                "us-east-1",
                "--service-code",
                "AmazonEC2",
                "--filters",
                &format!("Type=TERM_MATCH,Field=instanceType,Value={instance_type}"),
                &format!("Type=TERM_MATCH,Field=location,Value={location}"),
                "Type=TERM_MATCH,Field=operatingSystem,Value=Linux",
                "Type=TERM_MATCH,Field=tenancy,Value=Shared",
                "Type=TERM_MATCH,Field=preInstalledSw,Value=NA",
                "Type=TERM_MATCH,Field=capacitystatus,Value=Used",
            ]),
            Kind::Read,
        );
        let price = products.ok().flatten().and_then(|p| {
            let product = Json::parse(p.field("PriceList").item(0).as_str()?).ok()?;
            let term = product
                .field("terms")
                .field("OnDemand")
                .as_obj()
                .first()?
                .1
                .clone();
            let dimension = term.field("priceDimensions").as_obj().first()?.1.clone();
            dimension
                .field("pricePerUnit")
                .field("USD")
                .as_str()?
                .parse::<f64>()
                .ok()
        });
        if let Some(price) = price {
            self.info.set("usd_per_hour", format!("{price:.4}").into());
            self.info
                .set("price_source", "pricing get-products, OnDemand".into());
        }
    }

    fn account(&mut self) {
        let (Some(start), Some(end)) = (self.launched_at, self.ended_at) else {
            return;
        };
        if self.info.field("market").as_str() == Some("on-demand") {
            self.on_demand_price();
        }
        let hours = (end - start).max(60.0) / 3600.0;
        self.info
            .set("instance_hours", format!("{hours:.4}").into());
        if let Some(price) = self
            .info
            .field("usd_per_hour")
            .as_str()
            .and_then(|p| p.parse::<f64>().ok())
        {
            self.info
                .set("cost_usd", format!("{:.4}", price * hours).into());
        }
    }

    /// The whole run: preflight, then launch and measure, and the cleanup on every path.
    pub fn run(&mut self) -> R<Outcome> {
        fs::create_dir_all(&self.s.state).map_err(io_err(self.s.state.display()))?;
        self.preflight()?;
        let result = self.body();
        let cleaned = self.cleanup();
        self.account();
        match (result, cleaned) {
            (Ok(status), Ok(())) => Ok(Outcome { status }),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(cleanup)) => Err(cleanup),
            (Err(error), Err(cleanup)) => Err(format!("{error}; {cleanup}")),
        }
    }
}

fn tail(text: &str) -> String {
    let text = text.trim();
    let start = text.char_indices().rev().nth(600).map_or(0, |(i, _)| i);
    text[start..].to_string()
}

fn aws_error(argv: &[String], ran: &Ran) -> String {
    format!("{}: {}", argv.join(" "), tail(&ran.stderr))
}

/// The pricing API's location name of a region.
fn location(region: &str) -> Option<&'static str> {
    Some(match region {
        "eu-central-1" => "EU (Frankfurt)",
        "eu-west-1" => "EU (Ireland)",
        "eu-north-1" => "EU (Stockholm)",
        "us-east-1" => "US East (N. Virginia)",
        "us-east-2" => "US East (Ohio)",
        "us-west-2" => "US West (Oregon)",
        _ => return None,
    })
}

/// `YYYY-MM-DD` (UTC) of `secs` after the epoch.
pub fn civil_date(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn iso_now(secs: f64) -> String {
    let secs = secs as u64;
    format!(
        "{}T{:02}:{:02}:{:02}Z",
        civil_date(secs),
        secs / 3600 % 24,
        secs / 60 % 60,
        secs % 60
    )
}

/// One timing row over the passes: BASE's and HEAD's values, HEAD's loads.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub metric: String,
    pub target: String,
    pub base: Vec<i64>,
    pub head: Vec<i64>,
    pub head_loads: Vec<f64>,
}

fn median(values: &[i64]) -> i64 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    if n == 0 {
        0
    } else if n % 2 == 1 {
        sorted[n / 2]
    } else {
        ((sorted[n / 2 - 1] + sorted[n / 2]) as f64 / 2.0).round() as i64
    }
}

fn median_f(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    if n == 0 {
        0.0
    } else if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

/// The rows of the fetched `out/<tree>-<pass>.json` files, passes 1 to `passes` (pass 0 is
/// the warm-up and left out).
pub fn rows(out: &Path, passes: usize) -> R<Vec<Row>> {
    let mut rows: Vec<Row> = Vec::new();
    for pass in 1..=passes {
        for tree in ["base", "head"] {
            let path = out.join(format!("{tree}-{pass}.json"));
            let json = crate::perf::read_json(&path)?;
            for value in json.field("values").as_arr() {
                let metric = value.field("metric").as_str().unwrap_or("").to_string();
                let target = value.field("target").as_str().unwrap_or("").to_string();
                let number = value
                    .field("value")
                    .as_i64()
                    .ok_or_else(|| format!("{}: a value without a number", path.display()))?;
                let load = value
                    .field("load")
                    .as_str()
                    .and_then(|l| l.parse().ok())
                    .unwrap_or(0.0);
                let index = match rows
                    .iter()
                    .position(|r| r.metric == metric && r.target == target)
                {
                    Some(index) => index,
                    None => {
                        rows.push(Row {
                            metric,
                            target,
                            base: Vec::new(),
                            head: Vec::new(),
                            head_loads: Vec::new(),
                        });
                        rows.len() - 1
                    }
                };
                if tree == "base" {
                    rows[index].base.push(number);
                } else {
                    rows[index].head.push(number);
                    rows[index].head_loads.push(load);
                }
            }
        }
    }
    Ok(rows)
}

/// `value` as one compact line, keys sorted, as `quality/history.jsonl` holds it.
pub fn compact(value: &Json) -> String {
    match value {
        Json::Obj(entries) => {
            let mut sorted: Vec<&(String, Json)> = entries.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            let fields: Vec<String> = sorted
                .iter()
                .map(|(k, v)| format!("{}:{}", Json::Str(k.clone()).dumps(false), compact(v)))
                .collect();
            format!("{{{}}}", fields.join(","))
        }
        Json::Arr(items) => format!(
            "[{}]",
            items.iter().map(compact).collect::<Vec<_>>().join(",")
        ),
        other => other.dumps(false),
    }
}

/// One `quality/history.jsonl` line per row: HEAD's median as the value, BASE's as
/// `base_value`, the host by its type and `machine`.
pub fn history_lines(
    rows: &[Row],
    machine: &Json,
    base: &str,
    head: &str,
    run: &str,
    date: &str,
) -> Vec<String> {
    let instance_type = machine.field("instance_type").as_str().unwrap_or("?");
    rows.iter()
        .map(|row| {
            compact(&obj(vec![
                ("base", Json::from(base)),
                ("base_value", Json::Int(median(&row.base))),
                ("commit", Json::from(head)),
                ("date", Json::from(date)),
                ("host", Json::from(format!("ec2:{instance_type}"))),
                (
                    "load",
                    Json::from(format!("{:.2}", median_f(&row.head_loads))),
                ),
                ("machine", machine.clone()),
                ("metric", Json::from(row.metric.as_str())),
                ("run", Json::from(run)),
                ("target", Json::from(row.target.as_str())),
                ("value", Json::Int(median(&row.head))),
            ]))
        })
        .collect()
}

/// `value` in `unit`: a ratio in permille as `0.873×`, anything else with its unit.
pub fn shown(value: i64, unit: &str) -> String {
    match unit {
        "permille" => format!("{:.3}×", value as f64 / 1000.0),
        unit => format!("{value} {unit}"),
    }
}

/// The refusals of a release: a row whose HEAD median is worse than BASE's beyond the
/// tolerance of its metric in `baseline` (`quality/baseline.json`), or worse by any amount when
/// every HEAD sample is worse than every BASE sample (a row of one sample each: any worse value).
/// The tolerance is a bound on noise only: where the samples overlap the difference is not
/// shown and the tolerance decides; where they do not, the row is worse. No `decision` holds a
/// row for a release, the size row excepted ([`judge`]).
pub fn refusals(baseline: &Json, rows: &[Row]) -> R<Vec<String>> {
    Ok(judge(baseline, rows)?.refused)
}

/// What a release run found: every reason the release is refused, and every grown cell of the
/// size row a decision holds, one line each (the cell, BASE's value, HEAD's, the decision).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Judged {
    pub refused: Vec<String>,
    pub grown: Vec<String>,
}

/// The size row among the rows this run measures: the stripped release binary against plain
/// Rust. It is the only one of them the baseline may mark `"release_growth": "decision"`; the
/// others are time and memory, which no decision holds for a release.
pub const SIZE_ROW: &str = "3.9";

/// The rows of a release run judged against `baseline` ([`refusals`]). A cell of the size row
/// (`"release_growth": "decision"` on its metric; `AGENTS.md` § Releases: a binary may grow when
/// a release adds code) that would be refused is listed in `grown` instead when its bar in the
/// baseline carries a `decision`, which names the code that added the bytes; without one it is
/// refused and the refusal says the decision is missing. Another value of the mark, or the mark
/// on a row other than [`SIZE_ROW`], is an error.
pub fn judge(baseline: &Json, rows: &[Row]) -> R<Judged> {
    let mut out = Judged::default();
    for row in rows {
        let metric = baseline.field("metrics").field(&row.metric);
        let size_row = match metric.get("release_growth") {
            None => false,
            Some(Json::Str(rule)) if rule == "decision" => {
                if row.metric != SIZE_ROW {
                    return Err(format!(
                        "{}: `release_growth` is for the size row {SIZE_ROW} only; no decision holds a timing or benchmark row for a release",
                        row.metric
                    ));
                }
                true
            }
            Some(other) => {
                return Err(format!(
                    "{}: `release_growth` is {}, and \"decision\" is its only value",
                    row.metric,
                    compact(other)
                ));
            }
        };
        let direction = metric
            .field("direction")
            .as_str()
            .ok_or_else(|| format!("{}: no direction in the baseline", row.metric))?;
        let tolerance = metric.field("tolerance");
        let (base, head) = (median(&row.base), median(&row.head));
        let slack = match tolerance {
            Json::Int(n) => *n,
            Json::Str(text) => {
                let percent: i64 = text
                    .strip_suffix('%')
                    .and_then(|p| p.parse().ok())
                    .ok_or_else(|| format!("{}: tolerance {text:?}", row.metric))?;
                base.abs() * percent / 100
            }
            _ => 0,
        };
        let (heads, bases) = (row.head.iter(), row.base.iter());
        let (beyond, apart) = match direction {
            "lower" => (
                head > base + slack,
                head > base && matches!((heads.min(), bases.max()), (Some(h), Some(b)) if h > b),
            ),
            "higher" => (
                head < base - slack,
                head < base && matches!((heads.max(), bases.min()), (Some(h), Some(b)) if h < b),
            ),
            other => return Err(format!("{}: direction {other:?}", row.metric)),
        };
        if beyond || apart {
            let unit = metric.field("unit").as_str().unwrap_or("ms");
            let shown_apart = if beyond {
                ""
            } else {
                "; every sample worse than every sample of the base"
            };
            let values = format!(
                "{} {}: {} at the base, {} now",
                row.metric,
                row.target,
                shown(base, unit),
                shown(head, unit)
            );
            let judged = format!("({direction}, {}{shown_apart})", compact(tolerance));
            if !size_row {
                out.refused.push(format!("{values} {judged}"));
                continue;
            }
            let decision = metric
                .field("bars")
                .field(&row.target)
                .field("decision")
                .as_str()
                .map(str::trim)
                .filter(|decision| !decision.is_empty());
            match decision {
                Some(decision) => out.grown.push(format!("{values}; decision: {decision}")),
                None => out.refused.push(format!(
                    "{values} {judged}; a size row grows at a release only by a `decision` on the bar of the grown cell, and a `decision` naming the added code is missing"
                )),
            }
        }
    }
    Ok(out)
}

/// `machine` of the records: the instance type, what the host reported, and the cost.
pub fn machine(info: &Json) -> Json {
    let state = info.field("host_state");
    let read = |key: &str| state.field(key).as_str().unwrap_or("").to_string();
    let smt = match read("smt_active").as_str() {
        "0" => "off".to_string(),
        "1" => "on".to_string(),
        _ => format!("unknown ({})", read("smt_control")),
    };
    let turbo = match (
        read("intel_pstate_no_turbo").as_str(),
        read("cpufreq_boost").as_str(),
    ) {
        ("1", _) | (_, "0") => "off".to_string(),
        ("0", _) | (_, "1") => "on".to_string(),
        (a, b) => format!("unknown (no_turbo {a}, boost {b})"),
    };
    let mut fields = vec![
        ("instance_type", info.field("instance_type").clone()),
        ("cpu", Json::from(read("cpu"))),
        ("kernel", Json::from(read("kernel"))),
        ("nproc", Json::from(read("nproc"))),
        ("smt", Json::from(smt)),
        ("turbo", Json::from(turbo)),
        ("governor", Json::from(read("governors"))),
        ("clang", Json::from(read("clang"))),
        ("rustc", Json::from(read("rustc"))),
        ("market", info.field("market").clone()),
    ];
    if !read("go").is_empty() {
        fields.push(("go", Json::from(read("go"))));
    }
    for key in ["usd_per_hour", "instance_hours", "cost_usd"] {
        if let Some(value) = info.field(key).as_str() {
            fields.push((key, Json::from(value)));
        }
    }
    obj(fields)
}

fn git_commit(repo: &Path, rev: &str) -> R<String> {
    let out = Command::new("git")
        .args(["-C", &path_str(repo), "rev-parse", "--verify"])
        .arg(format!("{rev}^{{commit}}"))
        .output()
        .map_err(io_err("git"))?;
    if !out.status.success() {
        return Err(format!("{rev}: not a commit of {}", repo.display()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The fewest passes of each tree a release run takes, for the timing rows and the benchmark
/// alike: with five, two equal builds have disjoint samples on a row by chance in 1 of 252 runs.
pub const RELEASE_PASSES: usize = 5;

/// A release run (`--run vX.Y.Z`) on fewer than [`RELEASE_PASSES`] passes is an error before
/// anything starts: `--passes 0` yields no timing row and `--bench-passes 0` no benchmark row,
/// so the release would pass them unjudged. Other runs take any count.
pub fn release_samples(run: &str, passes: usize, bench_passes: usize) -> R<()> {
    if !run.starts_with('v') {
        return Ok(());
    }
    for (flag, count) in [("--passes", passes), ("--bench-passes", bench_passes)] {
        if count < RELEASE_PASSES {
            return Err(format!(
                "release {run}: {flag} {count} is fewer than the {RELEASE_PASSES} passes a release is judged on"
            ));
        }
    }
    Ok(())
}

/// `ignite-perf quality-remote`.
pub fn main(args: &RemoteArgs) -> R<i32> {
    release_samples(&args.run, args.passes, args.bench_passes)?;
    let profile = profile(args, std::env::var("IGNITE_PERF_AWS_PROFILE").ok())?;
    let region = args
        .region
        .clone()
        .or_else(|| std::env::var("IGNITE_PERF_AWS_REGION").ok())
        .filter(|r| !r.is_empty());
    let base = git_commit(&args.repo, &args.base)?;
    let head = git_commit(&args.repo, &args.head)?;
    let harness = git_commit(&args.repo, "HEAD")?;
    let dirty = Command::new("git")
        .args([
            "-C",
            &path_str(&args.repo),
            "status",
            "--porcelain",
            "--untracked-files=no",
        ])
        .output()
        .map_err(io_err("git"))?;
    if !dirty.stdout.is_empty() {
        eprintln!(
            "the checkout has uncommitted changes; the host runs the harness of {harness} only"
        );
    }
    let mut shell = System;
    let now = shell.now();
    let secs = now as u64;
    let run_id = format!(
        "iq-{}t{:02}{:02}{:02}z-{:04x}",
        civil_date(secs).replace('-', ""),
        secs / 3600 % 24,
        secs / 60 % 60,
        secs % 60,
        (now.fract() * 65536.0) as u32 ^ std::process::id() & 0xffff
    );
    let state = args.state.clone().unwrap_or_else(|| {
        let cache = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".cache")
            });
        cache.join("ignite/quality-remote").join(&run_id)
    });
    let settings = Settings {
        run_id: run_id.clone(),
        instance_type: args.instance_type.clone(),
        profile,
        region,
        max_hours: args.max_hours,
        passes: args.passes,
        check_runs: args.check_runs,
        build_runs: args.build_runs,
        bench_passes: args.bench_passes,
        dry_run: args.dry_run,
        state: state.clone(),
        repo: args.repo.clone(),
        base: base.clone(),
        head: head.clone(),
        harness,
    };
    catch_signals()?;
    let mut remote = Remote::new(settings, &mut shell);
    let result = remote.run();
    let info = remote.info().clone();
    let planned = remote.planned().len();
    drop(remote);
    if args.dry_run {
        result?;
        eprintln!(
            "dry run {run_id}: {planned} changing calls planned; state {}",
            state.display()
        );
        return Ok(0);
    }
    fs::write(state.join("run.json"), info.dumps_indent(true, 2) + "\n")
        .map_err(io_err(state.display()))?;
    eprintln!(
        "run {run_id}: {} {}, {} instance-hours at {} USD/h = {} USD",
        info.field("market").as_str().unwrap_or("?"),
        args.instance_type,
        info.field("instance_hours").as_str().unwrap_or("?"),
        info.field("usd_per_hour").as_str().unwrap_or("?"),
        info.field("cost_usd").as_str().unwrap_or("?")
    );
    let outcome = result?;
    if outcome.status != 0 {
        return Err(format!(
            "the host run exited {}; its log: {}",
            outcome.status,
            state.join("fetched/run.log").display()
        ));
    }
    let fetched = state.join("fetched/out");
    let mut rows = rows(&fetched, args.passes)?;
    let bench = fetched.join("bench.jsonl");
    let mut bench_records = Vec::new();
    if bench.exists() {
        let text = fs::read_to_string(&bench).map_err(io_err(bench.display()))?;
        let samples = crate::bench::read_samples(&text)?;
        let sizes = crate::perf::read_json(&fetched.join("bench-sizes.json"))?;
        rows.extend(crate::bench::rows(&samples, &sizes, args.bench_passes)?);
        bench_records = crate::bench::records(&samples, &sizes, args.bench_passes);
    }
    let baseline = crate::perf::read_json(&args.repo.join("quality/baseline.json"))?;
    let Judged { refused, grown } = if args.run.starts_with('v') {
        judge(&baseline, &rows)?
    } else {
        Judged::default()
    };
    let label = if refused.is_empty() {
        args.run.as_str()
    } else {
        "run"
    };
    let lines = history_lines(
        &rows,
        &machine(&info),
        &base,
        &head,
        label,
        &civil_date(secs),
    );
    let history = args.repo.join("quality/history.jsonl");
    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&history)
        .map_err(io_err(history.display()))?;
    for line in &lines {
        writeln!(file, "{line}").map_err(io_err(history.display()))?;
    }
    if !bench_records.is_empty() {
        let host = format!("ec2:{}", args.instance_type);
        let path = args.repo.join("quality/bench.jsonl");
        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&path)
            .map_err(io_err(path.display()))?;
        let date = civil_date(secs);
        for line in crate::bench::record_lines(&bench_records, &base, &head, label, &date, &host) {
            writeln!(file, "{line}").map_err(io_err(path.display()))?;
        }
        println!(
            "appended {} records to {}",
            bench_records.len(),
            path.display()
        );
    }
    for row in &rows {
        let (b, h) = (median(&row.base), median(&row.head));
        let unit = baseline
            .field("metrics")
            .field(&row.metric)
            .field("unit")
            .as_str()
            .unwrap_or("ms");
        println!(
            "{} {}: base {} {:?}, head {} {:?}, ratio {:.3}",
            row.metric,
            row.target,
            shown(b, unit),
            row.base,
            shown(h, unit),
            row.head,
            h as f64 / b.max(1) as f64
        );
    }
    println!("appended {} records to {}", lines.len(), history.display());
    if !grown.is_empty() {
        println!(
            "release {}: the size row above the base, each cell held by the decision on its bar:\n  {}",
            args.run,
            grown.join("\n  ")
        );
    }
    if !refused.is_empty() {
        eprintln!("release {} refused:\n  {}", args.run, refused.join("\n  "));
        return Ok(1);
    }
    Ok(0)
}
