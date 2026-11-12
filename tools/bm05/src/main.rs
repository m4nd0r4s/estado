//! BM-05 (spec 10.3): compile time of a synthetic machine with 200 states and
//! 600 edges, "under 3 s of expansion + checking".
//!
//! ```text
//! bm05-generator generate <file>               write the machine as a lib.rs
//! bm05-generator run [--runs N] [--fail-above SECONDS] [--states N --edges M] [--states N --edges M]
//! ```
//!
//! `run` writes the machine and a small baseline machine into throwaway
//! crates under `target/bm05`, warms the dependencies up, times
//! `cargo check` on each (median of N runs) and exits 1 when the big machine
//! takes longer than the threshold.

mod builder;
mod edges;
mod events;
mod probe;
mod random;
mod render;
mod render_handlers;
mod render_types;
mod tree;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use edges::{EdgeForm, Plan};
use probe::{Probe, median};
use tree::{NodeKind, TimerKind, Tree};

/// The spec target (BM-05): over it, the run reports a warning.
const TARGET_SECONDS: f64 = 3.0;
const STATE_BUDGET: usize = 200;
const EDGE_BUDGET: usize = 600;
/// A machine small enough to measure the fixed cost of cargo, rustc and the
/// macro, subtracted to report the cost that grows with the machine.
const BASELINE_STATES: usize = 4;
const BASELINE_EDGES: usize = 6;

fn generate(state_budget: usize, edge_budget: usize) -> (Tree, Plan, String) {
    let tree = builder::build_tree(state_budget);
    let plan = edges::plan_edges(&tree, edge_budget);
    let source = render::render_machine(&tree, &plan);
    (tree, plan, source)
}

fn describe(tree: &Tree, plan: &Plan, source: &str) -> String {
    let form_count = |form: EdgeForm| {
        plan.arms
            .iter()
            .flatten()
            .filter(|arm| arm.edge.is_some_and(|edge| edge.form == form))
            .count()
    };
    let timers = tree
        .nodes
        .iter()
        .filter(|node| node.timer != TimerKind::None)
        .count();
    let count = |kind: NodeKind| tree.nodes.iter().filter(|node| node.kind == kind).count();
    let deepest = tree
        .state_ids()
        .into_iter()
        .map(|id| tree.depth(id))
        .max()
        .unwrap_or(0);
    format!(
        "{} states ({} atomic, {} compound, {} parallel), {} regions, deepest state at depth {deepest}, \
         {} edges ({} history, {} constructor), {timers} timers, {} lines",
        tree.state_ids().len(),
        count(NodeKind::Atomic),
        count(NodeKind::Compound),
        count(NodeKind::Parallel),
        count(NodeKind::Region),
        plan.edge_count(),
        form_count(EdgeForm::History),
        form_count(EdgeForm::Constructor),
        source.lines().count(),
    )
}

fn repository_root() -> PathBuf {
    // tools/bm05 -> repository root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root exists")
}

fn seconds(duration: Duration) -> f64 {
    duration.as_secs_f64()
}

fn time_probe(probe: &Probe, source: &str, runs: usize) -> Result<Duration, String> {
    probe.check()?; // warm-up: builds the dependencies and the probe once
    let mut samples = Vec::new();
    for _ in 0..runs {
        probe.touch(source);
        samples.push(probe.check()?);
    }
    println!(
        "  samples: {}",
        samples
            .iter()
            .map(|sample| format!("{:.2}s", seconds(*sample)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(median(samples))
}

fn run(
    runs: usize,
    fail_above: f64,
    state_budget: usize,
    edge_budget: usize,
) -> Result<bool, String> {
    let repository = repository_root();
    let work_directory = repository.join("target/bm05");
    let (tree, plan, source) = generate(state_budget, edge_budget);
    let (baseline_tree, baseline_plan, baseline_source) = generate(BASELINE_STATES, BASELINE_EDGES);
    println!("machine: {}", describe(&tree, &plan, &source));

    let baseline_probe = Probe::write(
        &work_directory,
        &repository,
        "bm05_baseline",
        &baseline_source,
    );
    let machine_probe = Probe::write(&work_directory, &repository, "bm05_machine", &source);

    println!(
        "baseline ({}):",
        describe(&baseline_tree, &baseline_plan, &baseline_source)
    );
    let baseline = time_probe(&baseline_probe, &baseline_source, runs)?;
    println!("BM-05 machine:");
    let machine = time_probe(&machine_probe, &source, runs)?;

    let net = machine.saturating_sub(baseline);
    println!(
        "cargo check, median of {runs}: baseline {:.2}s, machine {:.2}s, machine minus baseline {:.2}s",
        seconds(baseline),
        seconds(machine),
        seconds(net)
    );
    publish_summary(runs, baseline, machine, fail_above);
    if seconds(machine) > TARGET_SECONDS {
        println!("warning: above the BM-05 target of {TARGET_SECONDS:.1}s on this machine");
        if std::env::var_os("GITHUB_ACTIONS").is_some() {
            println!(
                "::warning title=BM-05::cargo check of the 200-state machine took {:.2}s, above the {TARGET_SECONDS:.1}s target",
                seconds(machine)
            );
        }
    }
    if seconds(machine) > fail_above {
        println!(
            "FAIL: {:.2}s is above the threshold of {fail_above:.1}s",
            seconds(machine)
        );
        return Ok(false);
    }
    println!("PASS: under the threshold of {fail_above:.1}s");
    Ok(true)
}

/// Appends the result to the job summary when running in GitHub Actions.
fn publish_summary(runs: usize, baseline: Duration, machine: Duration, fail_above: f64) {
    use std::io::Write;
    let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") else {
        return;
    };
    let Ok(mut file) = std::fs::OpenOptions::new().append(true).open(path) else {
        return;
    };
    let _ = writeln!(
        file,
        "### BM-05: 200 states, 600 edges\n\n`cargo check`, median of {runs}: **{:.2} s** \
         (baseline crate {:.2} s; target {TARGET_SECONDS:.1} s; CI threshold {fail_above:.1} s)\n",
        seconds(machine),
        seconds(baseline),
    );
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let value_of = |flag: &str| {
        arguments
            .iter()
            .position(|argument| argument == flag)
            .and_then(|index| arguments.get(index + 1))
    };
    match arguments.first().map(String::as_str) {
        Some("generate") => {
            let Some(directory) = arguments.get(1) else {
                eprintln!("usage: bm05-generator generate <file>");
                return ExitCode::from(2);
            };
            let (tree, plan, source) = generate(STATE_BUDGET, EDGE_BUDGET);
            std::fs::write(directory, &source).expect("write the machine");
            println!("{}", describe(&tree, &plan, &source));
            ExitCode::SUCCESS
        }
        Some("run") => {
            let runs = value_of("--runs")
                .and_then(|text| text.parse().ok())
                .unwrap_or(3);
            let fail_above = value_of("--fail-above")
                .and_then(|text| text.parse().ok())
                .unwrap_or(TARGET_SECONDS);
            let states = value_of("--states")
                .and_then(|text| text.parse().ok())
                .unwrap_or(STATE_BUDGET);
            let edges = value_of("--edges")
                .and_then(|text| text.parse().ok())
                .unwrap_or(EDGE_BUDGET);
            match run(runs, fail_above, states, edges) {
                Ok(true) => ExitCode::SUCCESS,
                Ok(false) => ExitCode::from(1),
                Err(message) => {
                    eprintln!("the machine does not compile:\n{message}");
                    ExitCode::from(2)
                }
            }
        }
        _ => {
            eprintln!(
                "usage: bm05-generator generate <file> | run [--runs N] [--fail-above SECONDS] [--states N --edges M]"
            );
            ExitCode::from(2)
        }
    }
}
