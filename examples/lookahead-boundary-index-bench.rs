use std::hint::black_box;
use std::time::{Duration, Instant};

use tiqian::core::font_face::FontFaceId;
use tiqian::core::geometry::text_range;
use tiqian::core::layout_model::Cluster;
use tiqian::core::text::Text;
use tiqian::layout::line_breaker::{LineBreaker, LineBreakerConfig, LookaheadLineBreaker};

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
            iterations: 100,
            warmup: 10,
            calls_per_sample: 10,
        };
        let mut args = std::env::args().skip(1);
        while let Some(flag) = args.next() {
            if flag == "--help" || flag == "-h" {
                println!(
                    "lookahead-boundary-index-bench [--length 8192] [--iterations 100] [--warmup 10] [--calls-per-sample 10]"
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
        if options.length < 65 || options.iterations == 0 || options.calls_per_sample == 0 {
            return Err(
                "length must be at least 65; iterations and calls-per-sample must be positive"
                    .to_owned(),
            );
        }
        Ok(Some(options))
    }
}

struct Fixture {
    clusters: Vec<Cluster>,
    config: LineBreakerConfig,
}

impl Fixture {
    fn new(length: usize) -> Self {
        let clusters = (0..length)
            .map(|index| {
                Cluster::new(
                    text_range(index as i32, index as i32 + 1),
                    Text::from("中"),
                    FontFaceId::with_resource_id("benchmark"),
                    1.0,
                )
            })
            .collect();
        let mut config = LineBreakerConfig::default();
        config.cjk_inter_char_boundaries = (1..length as i32).collect();
        config.sino_western_boundaries = (1..length as i32).step_by(3).collect();
        config.max_cjk_stretch_per_gap = 8.0;
        config.sino_western_stretch_cap = 1.0;
        Self { clusters, config }
    }

    fn break_lines(&self) -> usize {
        LookaheadLineBreaker::default()
            .break_lines(&self.clusters, &self.clusters, 64.0, &self.config)
            .lines
            .len()
    }
}

fn print_stats(samples: &[Duration], calls_per_sample: usize) {
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
        "lookahead_break_lines us/call n={n} min={:.3} median={median:.3} p95={p95:.3} mean={:.3} max={:.3}",
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
    let expected = fixture.break_lines();
    println!(
        "length={} expected_lines={} warmup={} iterations={} calls_per_sample={} profile={} arch={}",
        options.length,
        expected,
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
    for _ in 0..options.warmup {
        for _ in 0..options.calls_per_sample {
            black_box(fixture.break_lines());
        }
    }
    let samples: Vec<_> = (0..options.iterations)
        .map(|_| {
            let start = Instant::now();
            for _ in 0..options.calls_per_sample {
                assert_eq!(black_box(fixture.break_lines()), expected);
            }
            start.elapsed()
        })
        .collect();
    print_stats(&samples, options.calls_per_sample);
    Ok(())
}
