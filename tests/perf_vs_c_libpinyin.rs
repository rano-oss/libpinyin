//! Performance comparison: Rust libpinyin vs system C/C++ libpinyin.
//!
//! Measures:
//! - Candidate latency (µs/op, throughput)
//! - Sentence latency
//! - Process RSS after engine load + warm query
//! - On-disk artifact sizes (shared libs, data dirs, IME binaries)
//!
//! Run (release, single-threaded):
//! ```text
//! cargo test -p libpinyin --release --test perf_vs_c_libpinyin -- --ignored --nocapture --test-threads=1
//! ```

mod common;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{apply_c_compat, data_dir, isolate_userdict, load_engine, CLibPinyin};

const C_USER_DIR: &str = "/tmp/libpinyin_perf_user";

const IS_PINYIN: u32 = 1 << 1;
const USE_TONE: u32 = 1 << 5;
const PINYIN_INCOMPLETE: u32 = 1 << 3;
const PINYIN_AMB_ALL: u32 = 0x3FF << 10;
const PINYIN_CORRECT_ALL: u32 = 0xFF << 21;
const PINYIN_OPTIONS_FULL: u32 =
    IS_PINYIN | USE_TONE | PINYIN_INCOMPLETE | PINYIN_AMB_ALL | PINYIN_CORRECT_ALL;

const WORKLOAD: &[&str] = &[
    "nihao",
    "zhongguo",
    "beijing",
    "shanghai",
    "xiexie",
    "yinggai",
    "chenggong",
    "gongzuo",
    "women",
    "pengyou",
    "woaini",
    "woshinuoweiren",
    "woshizhongguoren",
    "womenyiqiqubeijing",
    "jintiantianqizenmeyang",
];

fn set_full_c_options(c: &CLibPinyin) {
    c.set_options(PINYIN_OPTIONS_FULL);
}

fn rss_kb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kb);
        }
    }
    None
}

fn file_size(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|m| m.len())
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let walk = walkdir_simple(path);
    for p in walk {
        if let Ok(m) = std::fs::metadata(&p) {
            if m.is_file() {
                total += m.len();
            }
        }
    }
    total
}

fn walkdir_simple(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let p = ent.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out
}

fn fmt_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let x = n as f64;
    if x >= MB {
        format!("{:.2} MiB", x / MB)
    } else if x >= KB {
        format!("{:.1} KiB", x / KB)
    } else {
        format!("{n} B")
    }
}

fn fmt_dur(d: Duration) -> String {
    let us = d.as_secs_f64() * 1_000_000.0;
    if us >= 1000.0 {
        format!("{:.2} ms", us / 1000.0)
    } else {
        format!("{us:.1} µs")
    }
}

fn bench_loop<F: FnMut()>(iters: usize, mut f: F) -> Duration {
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    start.elapsed()
}

#[test]
#[ignore]
fn perf_compare_rust_vs_c() {
    // Ship numbers: `cargo test -p libpinyin --release --test perf_vs_c_libpinyin -- --ignored --nocapture`
    // Debug builds inflate Rust much more than C (String / unoptimized DP).
    isolate_userdict("perf");
    let rss_baseline = rss_kb();

    let Some(c) = CLibPinyin::new(C_USER_DIR) else {
        eprintln!("SKIP: system libpinyin unavailable");
        return;
    };
    set_full_c_options(&c);
    for input in WORKLOAD {
        let _ = c.candidates_count(input, 9);
        let _ = c.sentence_ok(input);
    }
    let rss_after_c = rss_kb();

    let rust_data = data_dir();
    let mut engine = load_engine();
    engine.set_allow_fuzzy(true);
    apply_c_compat(&mut engine);

    // Warm both engines.
    for input in WORKLOAD {
        let _ = c.candidates_count(input, 9);
        let _ = engine.input(input);
        let _ = c.sentence_ok(input);
        let _ = engine.best_sentence(input);
    }
    let rss_after_both = rss_kb();
    engine.clear_cache();

    let rounds = 40; // each round walks full WORKLOAD
    let ops = rounds * WORKLOAD.len();

    // --- Candidate latency ---
    let c_cand = bench_loop(rounds, || {
        for input in WORKLOAD {
            let _ = c.candidates_count(input, 9);
        }
    });
    let r_cand = bench_loop(rounds, || {
        for input in WORKLOAD {
            let _ = engine.input(input);
        }
    });
    engine.clear_cache();
    let r_cand_uncached = bench_loop(rounds, || {
        for input in WORKLOAD {
            engine.clear_cache();
            let _ = engine.input(input);
        }
    });

    // --- Sentence latency ---
    let c_sent = bench_loop(rounds, || {
        for input in WORKLOAD {
            let _ = c.sentence_ok(input);
        }
    });
    engine.clear_cache();
    let r_sent = bench_loop(rounds, || {
        for input in WORKLOAD {
            let _ = engine.best_sentence(input);
        }
    });

    println!("\n=== Performance: Rust libpinyin vs C libpinyin ===");
    println!(
        "workload: {} inputs × {rounds} rounds = {ops} ops",
        WORKLOAD.len()
    );
    println!("rust data: {}", rust_data.display());
    println!();
    println!("## Speed");
    println!("| op | C total | C /op | Rust total | Rust /op | Rust/C |");
    println!("|---|---:|---:|---:|---:|---:|");
    let row = |name: &str, c_d: Duration, r_d: Duration| {
        let c_op = c_d / ops as u32;
        let r_op = r_d / ops as u32;
        let ratio = r_d.as_secs_f64() / c_d.as_secs_f64().max(1e-12);
        println!(
            "| {name} | {} | {} | {} | {} | {ratio:.2}× |",
            fmt_dur(c_d),
            fmt_dur(c_op),
            fmt_dur(r_d),
            fmt_dur(r_op),
        );
    };
    row("candidates×9 (warm cache)", c_cand, r_cand);
    row("candidates×9 (uncached each op)", c_cand, r_cand_uncached);
    row("sentence", c_sent, r_sent);

    // Ship gate (release only): every listed metric must beat or match C.
    #[cfg(not(debug_assertions))]
    {
        let ratio = |r: Duration, c: Duration| r.as_secs_f64() / c.as_secs_f64().max(1e-12);
        let warm = ratio(r_cand, c_cand);
        let cold = ratio(r_cand_uncached, c_cand);
        let sent = ratio(r_sent, c_sent);
        assert!(
            warm <= 1.0 && cold <= 1.0 && sent <= 1.0,
            "release perf regression: warm={warm:.2}× cold={cold:.2}× sentence={sent:.2}× (need ≤1.0×)"
        );
    }

    println!();
    println!("## Memory (VmRSS deltas in this process)");
    let fmt_rss = |label: &str, kb: Option<u64>| match kb {
        Some(k) => println!("{label}: {} ({} KiB)", fmt_bytes(k * 1024), k),
        None => println!("{label}: unavailable"),
    };
    fmt_rss("baseline (before engines)", rss_baseline);
    fmt_rss("after C engine warm", rss_after_c);
    fmt_rss("after C + Rust warm", rss_after_both);
    if let (Some(b), Some(c_kb), Some(both)) = (rss_baseline, rss_after_c, rss_after_both) {
        println!(
            "Δ C ≈ {}, Δ Rust≈ {} (approx; allocator retains pages)",
            fmt_bytes(c_kb.saturating_sub(b) * 1024),
            fmt_bytes(both.saturating_sub(c_kb) * 1024)
        );
    }
    println!();
    println!("## Process-tree note (session architecture)");
    println!("ibus/fcitx are multi-process (daemon + UI + portal + bridge + engine).");
    println!("pinyinwl is typically one process (engine + UI). Compare Σ RSS of the");
    println!("whole IM session, not a single engine binary — e.g. idle ibus* here is");
    println!("often ~100+ MiB before libpinyin is even loaded.");
    print_live_im_session_rss();

    println!();
    println!("## On-disk size (fair: packages + ldd closures)");
    print_size_report(&rust_data);

    // Soft sanity: bench should finish and produce finite timings.
    assert!(c_cand > Duration::ZERO && r_cand > Duration::ZERO);
}

fn ldd_resolved_libs(bin: &Path) -> Vec<PathBuf> {
    let Ok(out) = std::process::Command::new("ldd").arg(bin).output() else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut libs = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.split("=>").nth(1) {
            let path = rest.split_whitespace().next().unwrap_or("");
            if path.starts_with('/') {
                libs.push(PathBuf::from(path));
            }
        }
    }
    libs
}

fn uniq_file_size_sum(paths: impl IntoIterator<Item = PathBuf>) -> (u64, Vec<(u64, PathBuf)>) {
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    let mut total = 0u64;
    for p in paths {
        let Ok(meta) = std::fs::metadata(&p) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let Ok(canon) = std::fs::canonicalize(&p) else {
            continue;
        };
        if !seen.insert(canon.clone()) {
            continue;
        }
        total += meta.len();
        items.push((meta.len(), canon));
    }
    items.sort_by_key(|a| std::cmp::Reverse(a.0));
    (total, items)
}

fn rpm_size(pkg: &str) -> Option<u64> {
    let out = std::process::Command::new("rpm")
        .args(["-q", "--qf", "%{size}", pkg])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()?.trim().parse().ok()
}

fn rpm_ql(pkg: &str) -> Vec<PathBuf> {
    let out = std::process::Command::new("rpm")
        .args(["-ql", pkg])
        .output();
    let Ok(out) = out else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .collect()
}

fn print_live_im_session_rss() {
    let mut rows: Vec<(String, u64)> = Vec::new();
    let Ok(proc) = std::fs::read_dir("/proc") else {
        println!("(no /proc)");
        return;
    };
    for ent in proc.flatten() {
        let name = ent.file_name();
        let name = name.to_string_lossy();
        if !name.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let comm_path = ent.path().join("comm");
        let Ok(comm) = std::fs::read_to_string(&comm_path) else {
            continue;
        };
        let comm = comm.trim();
        let keep =
            comm.starts_with("ibus") || comm.starts_with("fcitx") || comm.starts_with("pinyinwl");
        if !keep {
            continue;
        }
        let rss = rss_kb_of(&name).unwrap_or(0);
        rows.push((comm.to_string(), rss));
    }
    rows.sort_by_key(|a| std::cmp::Reverse(a.1));
    println!("Live session Σ VmRSS (this machine, right now):");
    for prefix in ["ibus", "fcitx", "pinyinwl"] {
        let procs: Vec<_> = rows.iter().filter(|(c, _)| c.starts_with(prefix)).collect();
        let sum: u64 = procs.iter().map(|(_, r)| *r).sum();
        if procs.is_empty() {
            println!("  {prefix}*: (not running)");
        } else {
            println!(
                "  {prefix}*: {} procs, Σ {}",
                procs.len(),
                fmt_bytes(sum * 1024)
            );
        }
    }
    for (comm, rss) in rows.iter().take(10) {
        println!("    {:>8}  {comm}", fmt_bytes(rss * 1024));
    }
}

fn rss_kb_of(pid: &str) -> Option<u64> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest.split_whitespace().next()?.parse().ok();
        }
    }
    None
}

fn print_size_report(rust_data: &Path) {
    use std::os::unix::fs::PermissionsExt;

    // Package payload sizes (installed file totals from RPM).
    let pkgs = ["ibus-libpinyin", "libpinyin", "libpinyin-data"];
    println!("| RPM package | installed size |");
    println!("|---|---:|");
    let mut rpm_sum = 0u64;
    for pkg in pkgs {
        match rpm_size(pkg) {
            Some(n) => {
                rpm_sum += n;
                println!("| {pkg} | {} |", fmt_bytes(n));
            }
            None => println!("| {pkg} | (not installed) |"),
        }
    }
    println!("| **sum (ibus stack + data)** | {} |", fmt_bytes(rpm_sum));
    println!(
        "| Rust data (simplified) | {} |",
        fmt_bytes(dir_size(rust_data))
    );

    // Binary + linked .so closure (unique realpaths).
    let mut ibus_paths: Vec<PathBuf> = Vec::new();
    for f in rpm_ql("ibus-libpinyin") {
        let Ok(meta) = std::fs::metadata(&f) else {
            continue;
        };
        let mode = meta.permissions().mode();
        let is_exec = mode & 0o111 != 0;
        let is_so = f
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains(".so"));
        if is_exec || is_so {
            ibus_paths.push(f.clone());
            if is_exec {
                ibus_paths.extend(ldd_resolved_libs(&f));
            }
        }
    }
    if let Some(n) = file_size(Path::new("/usr/lib64/libpinyin.so.15.0.0")) {
        let _ = n;
        ibus_paths.push(PathBuf::from("/usr/lib64/libpinyin.so.15.0.0"));
    }

    let pinyinwl =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../pinyinwl/target/release/pinyinwl");
    let mut wl_paths = vec![pinyinwl.clone()];
    wl_paths.extend(ldd_resolved_libs(&pinyinwl));

    let (ibus_closure, ibus_top) = uniq_file_size_sum(ibus_paths);
    let (wl_closure, wl_top) = uniq_file_size_sum(wl_paths);

    println!();
    println!("| runtime closure (bins + ldd .so, unique) | size |");
    println!("|---|---:|");
    println!(
        "| ibus-libpinyin executables + linked libs | {} |",
        fmt_bytes(ibus_closure)
    );
    println!("| pinyinwl + linked libs | {} |", fmt_bytes(wl_closure));

    println!();
    println!("Top linked libs (ibus-libpinyin closure):");
    for (sz, p) in ibus_top.iter().take(8) {
        println!("  {:>10}  {}", fmt_bytes(*sz), p.display());
    }
    println!("Top linked libs (pinyinwl closure):");
    for (sz, p) in wl_top.iter().take(8) {
        println!("  {:>10}  {}", fmt_bytes(*sz), p.display());
    }

    let rlib_core =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/release/liblibpinyin.rlib");
    let rlib_py =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/release/liblibpinyin.rlib");
    println!();
    println!("| Rust static libs (rlib, not shipped as-is) | size |");
    println!("|---|---:|");
    if let Some(n) = file_size(&rlib_py) {
        println!("| liblibpinyin.rlib | {} |", fmt_bytes(n));
    }
    if let Some(n) = file_size(&rlib_core) {
        println!("| liblibpinyin.rlib | {} |", fmt_bytes(n));
    }
}

#[test]
#[ignore]
fn perf_breakdown_cold() {
    isolate_userdict("perf");
    let mut engine = load_engine();
    engine.set_allow_fuzzy(true);
    apply_c_compat(&mut engine);
    let parser = libpinyin::Parser::with_syllables(libpinyin::PINYIN_SYLLABLES);
    let rounds = 50usize;
    let mut seg_t = std::time::Duration::ZERO;
    let mut full_t = std::time::Duration::ZERO;
    let mut best_t = std::time::Duration::ZERO;
    for input in WORKLOAD {
        let t0 = Instant::now();
        for _ in 0..rounds {
            let _ = parser.segment_best(input, true);
        }
        best_t += t0.elapsed();
        let t1 = Instant::now();
        for _ in 0..rounds {
            let _ = parser.segment_top_k(input, 1, true);
        }
        seg_t += t1.elapsed();
        let t2 = Instant::now();
        for _ in 0..rounds {
            engine.clear_cache();
            let _ = engine.input(input);
        }
        full_t += t2.elapsed();
    }
    let ops = rounds * WORKLOAD.len();
    println!("seg_best/op {}", fmt_dur(best_t / ops as u32));
    println!("seg_topk1/op {}", fmt_dur(seg_t / ops as u32));
    println!("input_cold/op {}", fmt_dur(full_t / ops as u32));
    println!(
        "nonseg ≈ {}",
        fmt_dur((full_t.saturating_sub(seg_t)) / ops as u32)
    );
}
