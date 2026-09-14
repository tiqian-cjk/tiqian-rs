use std::hint::black_box;
use std::time::{Duration, Instant};

use tiqian::common::HashSet;

struct Options {
    length: usize,
    intervals: usize,
    iterations: usize,
    warmup: usize,
    calls_per_sample: usize,
}

impl Options {
    fn parse() -> Result<Option<Self>, String> {
        let mut options = Self {
            length: 8_192,
            intervals: 32,
            iterations: 200,
            warmup: 20,
            calls_per_sample: 100,
        };
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            if flag == "--help" || flag == "-h" {
                println!(
                    "boundary-prefix-index-bench [--length 8192] [--intervals 32] [--iterations 200] [--warmup 20] [--calls-per-sample 100]"
                );
                return Ok(None);
            }
            if !matches!(
                flag.as_str(),
                "--length" | "--intervals" | "--iterations" | "--warmup" | "--calls-per-sample"
            ) {
                return Err(format!("unknown option: {flag}"));
            }
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for {flag}"))?;
            match flag.as_str() {
                "--length" => options.length = value.parse().map_err(|_| "invalid length")?,
                "--intervals" => {
                    options.intervals = value.parse().map_err(|_| "invalid intervals")?
                }
                "--iterations" => {
                    options.iterations = value.parse().map_err(|_| "invalid iterations")?
                }
                "--warmup" => options.warmup = value.parse().map_err(|_| "invalid warmup")?,
                "--calls-per-sample" => {
                    options.calls_per_sample = value
                        .parse()
                        .map_err(|_| "invalid calls-per-sample")?
                }
                _ => unreachable!(),
            }
        }
        if options.length < 2
            || options.intervals == 0
            || options.iterations == 0
            || options.calls_per_sample == 0
        {
            return Err("length must be at least 2; intervals, iterations and calls-per-sample must be positive".to_owned());
        }
        Ok(Some(options))
    }
}

struct Fixture {
    length: usize,
    cjk: HashSet<i32>,
    sino: HashSet<i32>,
    intervals: Vec<(usize, usize)>,
}

impl Fixture {
    fn new(length: usize, interval_count: usize, span: usize) -> Self {
        let cjk = (1..length as i32).step_by(2).collect();
        let sino = (1..length as i32).step_by(3).collect();
        let intervals = (0..interval_count)
            .map(|index| {
                let start = (index * span * 3) % (length - span);
                (start, start + span)
            })
            .collect();
        Self {
            length,
            cjk,
            sino,
            intervals,
        }
    }

    fn current_gap(&self) -> i32 {
        let mut gap = self.cjk.clone();
        gap.extend(&self.sino);
        self.intervals.iter().fold(0, |total, &(start, end)| {
            total
                + (start..end)
                    .filter(|index| gap.contains(&(*index as i32)))
                    .count() as i32
        })
    }

    fn gap_prefix(&self) -> i32 {
        let mut prefix = vec![0; self.length + 1];
        for index in 0..self.length {
            let boundary = index as i32;
            prefix[index + 1] = prefix[index]
                + if self.cjk.contains(&boundary) || self.sino.contains(&boundary) {
                    1
                } else {
                    0
                };
        }
        self.intervals.iter().fold(0, |total, &(start, end)| {
            total + prefix[end] - prefix[start]
        })
    }

    fn all_prefixes(&self) -> i32 {
        let mut gap_prefix = vec![0; self.length + 1];
        let mut sino_prefix = vec![0; self.length + 1];
        let mut cjk_prefix = vec![0; self.length + 1];
        for index in 0..self.length {
            let boundary = index as i32;
            gap_prefix[index + 1] = gap_prefix[index]
                + if self.cjk.contains(&boundary) || self.sino.contains(&boundary) {
                    1
                } else {
                    0
                };
            sino_prefix[index + 1] = sino_prefix[index]
                + if self.sino.contains(&boundary) { 1 } else { 0 };
            cjk_prefix[index + 1] = cjk_prefix[index]
                + if self.cjk.contains(&boundary) { 1 } else { 0 };
        }
        self.intervals.iter().fold(0, |total, &(start, end)| {
            total
                + gap_prefix[end] - gap_prefix[start]
                + sino_prefix[end]
                - sino_prefix[start]
                + cjk_prefix[end]
                - cjk_prefix[start]
        })
    }
}

fn print_stats(label: &str, samples: &[Duration], calls_per_sample: usize) {
    let mut values: Vec<_> = samples
        .iter()
        .map(|duration| duration.as_secs_f64() * 1_000_000.0 / calls_per_sample as f64)
        .collect();
    values.sort_by(f64::total_cmp);
    let n = values.len();
    let median = if n % 2 == 0 {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    } else {
        values[n / 2]
    };
    let p95 = values[(n * 95).div_ceil(100) - 1];
    println!(
        "{label} us/call n={n} min={:.3} median={median:.3} p95={p95:.3} mean={:.3} max={:.3}",
        values[0],
        values.iter().sum::<f64>() / n as f64,
        values[n - 1]
    );
}

fn measure(
    label: &str,
    expected: i32,
    options: &Options,
    mut operation: impl FnMut() -> i32,
) {
    for _ in 0..options.warmup {
        for _ in 0..options.calls_per_sample {
            black_box(operation());
        }
    }
    let samples: Vec<_> = (0..options.iterations)
        .map(|_| {
            let start = Instant::now();
            for _ in 0..options.calls_per_sample {
                assert_eq!(black_box(operation()), expected);
            }
            start.elapsed()
        })
        .collect();
    print_stats(label, &samples, options.calls_per_sample);
}

fn main() -> Result<(), String> {
    let Some(options) = Options::parse()? else {
        return Ok(());
    };
    if cfg!(debug_assertions) {
        eprintln!("Warning: debug build; use --release for performance comparisons.");
    }
    let scenarios = [
        ("greedy-like", Fixture::new(options.length, options.intervals, 64)),
        (
            "lookahead-like",
            Fixture::new(options.length, options.intervals * 8, 64),
        ),
    ];
    println!(
        "length={} intervals={} warmup={} iterations={} calls_per_sample={} profile={} arch={}",
        options.length,
        options.intervals,
        options.warmup,
        options.iterations,
        options.calls_per_sample,
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        std::env::consts::ARCH
    );
    for (label, fixture) in scenarios {
        let expected_gap = fixture.current_gap();
        let expected_all = fixture.intervals.iter().fold(0, |total, &(start, end)| {
            total
                + (start..end)
                    .filter(|index| {
                        fixture.cjk.contains(&(*index as i32))
                            || fixture.sino.contains(&(*index as i32))
                    })
                    .count() as i32
                + (start..end)
                    .filter(|index| fixture.sino.contains(&(*index as i32)))
                    .count() as i32
                + (start..end)
                    .filter(|index| fixture.cjk.contains(&(*index as i32)))
                    .count() as i32
        });
                assert_eq!(expected_all, fixture.all_prefixes());
                assert_eq!(expected_gap, fixture.gap_prefix());
        assert_eq!(expected_all, fixture.all_prefixes());
        measure(
            &format!("current[{label}]"),
            expected_gap,
            &options,
            || fixture.current_gap(),
        );
        measure(
            &format!("gap-prefix[{label}]"),
            expected_gap,
            &options,
            || fixture.gap_prefix(),
        );
        measure(
            &format!("all-prefixes[{label}]"),
            expected_all,
            &options,
            || fixture.all_prefixes(),
        );
    }
    println!("Each call includes boundary index construction.");
    Ok(())
}
