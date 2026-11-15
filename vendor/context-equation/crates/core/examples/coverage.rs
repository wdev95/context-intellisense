//! Converts every formula of `target/coverage/formulas.jsonl` (written by
//! `tools/coverage/extract.py`) and writes one result per line to `target/coverage/results.jsonl`.
//!
//! `cargo run --release -p contextEquation --example coverage`
//!
//! Each formula is converted in tolerant mode (the default) and gets one status:
//! `ok` (no warning), `warn` (converted with warnings: something was not understood and was
//! worked around, usually drawn in red), `error` (refused), `panic` (contextEquation crashed) or `timeout`
//! (no answer in `TIMEOUT`). A panic or a timeout is caught, recorded, and the run goes on.

use std::io::{BufRead, BufWriter, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use context_equation_core::{convert, Options};

const TIMEOUT: Duration = Duration::from_secs(10);

/// What a worker is converting (the job's index) and since when, for the watchdog.
type Busy = Mutex<Option<(usize, Instant)>>;

struct Job {
    id: u64,
    tex: String,
    display: bool,
}

fn main() {
    let dir = format!("{}/../../target/coverage", env!("CARGO_MANIFEST_DIR"));
    let input = std::fs::File::open(format!("{dir}/formulas.jsonl")).expect("run tools/coverage/extract.py first");
    let jobs: Vec<Job> = std::io::BufReader::new(input)
        .lines()
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(&l.unwrap()).unwrap();
            Job { id: v["id"].as_u64().unwrap(), tex: v["tex"].as_str().unwrap().to_string(), display: v["display"].as_bool().unwrap() }
        })
        .collect();
    let total = jobs.len();
    let jobs = Arc::new(jobs);
    let next = Arc::new(AtomicUsize::new(0));
    // what each worker is converting and since when, for the watchdog
    let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let busy: Arc<Vec<Busy>> = Arc::new((0..workers).map(|_| Mutex::new(None)).collect());
    let (tx, rx) = mpsc::channel::<serde_json::Value>();

    // a panic is a result here, not noise on stderr
    std::panic::set_hook(Box::new(|_| {}));

    for w in 0..workers {
        let (jobs, next, busy, tx) = (jobs.clone(), next.clone(), busy.clone(), tx.clone());
        std::thread::spawn(move || loop {
            let k = next.fetch_add(1, Ordering::Relaxed);
            let Some(job) = jobs.get(k) else { break };
            *busy[w].lock().unwrap() = Some((k, Instant::now()));
            let options = Options { display: job.display, ..Options::default() };
            let start = Instant::now();
            let outcome = catch_unwind(AssertUnwindSafe(|| convert(&job.tex, &options)));
            let micros = start.elapsed().as_micros() as u64;
            *busy[w].lock().unwrap() = None;
            let result = match outcome {
                Ok(Ok(out)) if out.warnings.is_empty() => serde_json::json!({ "id": job.id, "status": "ok", "us": micros }),
                Ok(Ok(out)) => serde_json::json!({
                    "id": job.id, "status": "warn", "us": micros,
                    "warnings": out.warnings.iter().map(|a| serde_json::json!([a.rule, a.description])).collect::<Vec<_>>(),
                }),
                Ok(Err(e)) => serde_json::json!({ "id": job.id, "status": "error", "us": micros, "message": e.to_string() }),
                Err(p) => {
                    let msg = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
                    serde_json::json!({ "id": job.id, "status": "panic", "us": micros, "message": msg })
                }
            };
            if tx.send(result).is_err() {
                break;
            }
        });
    }
    drop(tx);

    let out = std::fs::File::create(format!("{dir}/results.jsonl")).unwrap();
    let mut out = BufWriter::new(out);
    let mut done = 0usize;
    let mut timed_out = std::collections::HashSet::new();
    let started = Instant::now();
    while done + timed_out.len() < total {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(v) => {
                writeln!(out, "{v}").unwrap();
                done += 1;
                if done.is_multiple_of(20_000) {
                    eprintln!("{done}/{total} in {:.0?}", started.elapsed());
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        // a worker stuck past TIMEOUT stays stuck; its formula is recorded and the others go on
        for slot in busy.iter() {
            if let Some((k, since)) = *slot.lock().unwrap() {
                if since.elapsed() > TIMEOUT && timed_out.insert(k) {
                    writeln!(out, "{}", serde_json::json!({ "id": jobs[k].id, "status": "timeout", "us": since.elapsed().as_micros() as u64 })).unwrap();
                    eprintln!("timeout: formula {}", jobs[k].id);
                }
            }
        }
    }
    out.flush().unwrap();
    eprintln!("{done} converted, {} timed out, in {:.1?}", timed_out.len(), started.elapsed());
    // threads stuck in a timeout would keep the process alive
    std::process::exit(0);
}
