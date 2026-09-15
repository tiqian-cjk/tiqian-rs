use std::hint::black_box;
use std::time::{Duration, Instant};

use tiqian::common::HashSet;
use tiqian::core::font_face::FontFaceId;
use tiqian::core::geometry::text_range;
use tiqian::core::layout_model::Cluster;
use tiqian::core::text::Text;
use tiqian::layout::progressive_break_decisions::decide_hyphen_break;

struct Options {
    length: usize,
    iterations: usize,
    warmup: usize,
    calls_per_sample: usize,
}

impl Options {
    fn parse() -> Result<Option<Self>, String> {
        let mut options = Self {
            length: 8_192,
            iterations: 200,
            warmup: 20,
            calls_per_sample: 100,
        };
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            if flag == "--help" || flag == "-h" {
                println!(
                    "hyphen-break-bench [--length 8192] [--iterations 200] [--warmup 20] [--calls-per-sample 100]"
                );
                return Ok(None);
            }
            if !matches!(
                flag.as_str(),
                "--length" | "--iterations" | "--warmup" | "--calls-per-sample"
            ) {
                return Err(format!("unknown option: {flag}"));
            }
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for {flag}"))?;
            match flag.as_str() {
                "--length" => options.length = value.parse().map_err(|_| "invalid length")?,
                "--iterations" => {
                    options.iterations = value.parse().map_err(|_| "invalid iterations")?
                }
                "--warmup" => options.warmup = value.parse().map_err(|_| "invalid warmup")?,
                "--calls-per-sample" => {
                    options.calls_per_sample =
                        value.parse().map_err(|_| "invalid calls-per-sample")?
                }
                _ => unreachable!(),
            }
        }
        if options.length < 2 || options.iterations == 0 || options.calls_per_sample == 0 {
            return Err(
                "length must be at least 2; iterations and calls-per-sample must be positive"
                    .to_owned(),
            );
        }
        Ok(Some(options))
    }
}

struct Fixture {
    clusters: Vec<Cluster>,
    hyphen_break_clusters: HashSet<i32>,
    cjk_inter_char_boundaries: HashSet<i32>,
    sino_western_boundaries: HashSet<i32>,
}

impl Fixture {
    fn new(length: usize) -> Self {
        let clusters = (0..length)
            .map(|index| {
                Cluster::new(
                    text_range(index as i32, index as i32 + 1),
                    Text::from("x"),
                    FontFaceId::with_resource_id("benchmark"),
                    1.0,
                )
            })
            .collect();
        let hyphen_break_clusters = HashSet::from([length as i32]);
        let cjk_inter_char_boundaries = (1..length as i32 - 1).collect();
        let sino_western_boundaries = (1..length as i32 - 1).step_by(2).collect();
        Self {
            clusters,
            hyphen_break_clusters,
            cjk_inter_char_boundaries,
            sino_western_boundaries,
        }
    }

    fn decide(&self, line_limit: f32) -> i32 {
        decide_hyphen_break(
            0,
            self.clusters.len() as i32,
            &self.clusters,
            line_limit,
            &self.hyphen_break_clusters,
            &self.cjk_inter_char_boundaries,
            8.0,
            &self.sino_western_boundaries,
            1.0,
        )
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

fn main() -> Result<(), String> {
    let Some(options) = Options::parse()? else {
        return Ok(());
    };
    if cfg!(debug_assertions) {
        eprintln!("Warning: debug build; use --release for performance comparisons.");
    }
    let fixture = Fixture::new(options.length);
    let expected = fixture.clusters.len() as i32 - 1;
    let scenarios = [
        ("sino-fills-deficit", options.length as f32 + 1.0),
        ("cjk-fills-residual", options.length as f32 * 1.5),
    ];
    for (_, line_limit) in scenarios {
        assert_eq!(fixture.decide(line_limit), expected);
    }
    println!(
        "length={} warmup={} iterations={} calls_per_sample={} profile={} arch={}",
        options.length,
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
    for (label, line_limit) in scenarios {
        for _ in 0..options.warmup {
            for _ in 0..options.calls_per_sample {
                black_box(fixture.decide(line_limit));
            }
        }
        let samples: Vec<_> = (0..options.iterations)
            .map(|_| {
                let start = Instant::now();
                for _ in 0..options.calls_per_sample {
                    assert_eq!(black_box(fixture.decide(line_limit)), expected);
                }
                start.elapsed()
            })
            .collect();
        print_stats(
            &format!("decide_hyphen_break[{label}]"),
            &samples,
            options.calls_per_sample,
        );
    }
    println!(
        "Measured calls: {}",
        options.iterations * options.calls_per_sample * scenarios.len()
    );
    Ok(())
}
