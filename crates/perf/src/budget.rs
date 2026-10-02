use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Rule 7: a regression above this share of the baseline fails.
pub const REGRESSION: f64 = 0.10;

/// What one metric may measure. A metric with neither a limit nor a baseline for this OS is reported, never failed.
#[derive(Debug, Deserialize)]
pub struct Budget {
    /// Absolute ceiling from rule 7.
    pub limit: Option<f64>,
    /// Absolute slack added to the regression ceiling, so a metric near the clock's resolution does not fail on 10% of nothing.
    #[serde(default)]
    pub noise: f64,
    /// The last accepted median per OS (`std::env::consts::OS`): perf numbers do not transfer between platforms.
    #[serde(default)]
    pub baseline: BTreeMap<String, f64>,
    /// Load per cpu (R13) above which this metric's regression test is skipped for this OS. Absent (e.g. `rss_extra_mb`) means always compared.
    #[serde(default)]
    pub max_load_per_cpu: BTreeMap<String, f64>,
}

pub type Budgets = BTreeMap<String, Budget>;

/// What a run wrote: the median of every metric over `runs`, and each run's value so the spread is visible.
#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub os: String,
    pub cpus: usize,
    pub runs: usize,
    /// 1-minute load average at the start of each run.
    pub loads: Vec<f64>,
    pub metrics: BTreeMap<String, f64>,
    pub per_run: BTreeMap<String, Vec<f64>>,
    /// Regression tests this run did not compare because the machine was busier than the metric's threshold (R13).
    pub skipped: Vec<Skipped>,
}

#[derive(Debug, PartialEq)]
pub enum Miss {
    OverLimit { limit: f64 },
    Regressed { baseline: f64, ceiling: f64 },
}

/// One metric whose regression test this run did not run (R13): the load that caused it, so the output can say why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Skipped {
    pub metric: String,
    pub load_per_cpu: f64,
    pub max_load_per_cpu: f64,
}

/// Every metric's result: what missed (fails the run) and what the regression test skipped (R13; still reported).
#[derive(Debug, PartialEq)]
pub struct Outcome {
    pub misses: Vec<(String, Miss)>,
    pub skipped: Vec<Skipped>,
}

fn over_limit(value: f64, budget: &Budget) -> Option<Miss> {
    budget
        .limit
        .filter(|&limit| value > limit)
        .map(|limit| Miss::OverLimit { limit })
}

/// `None` is a pass.
pub fn judge(value: f64, budget: &Budget, os: &str) -> Option<Miss> {
    if let Some(miss) = over_limit(value, budget) {
        return Some(miss);
    }

    let baseline = *budget.baseline.get(os)?;
    let ceiling = baseline * (1.0 + REGRESSION) + budget.noise;

    (value > ceiling).then_some(Miss::Regressed { baseline, ceiling })
}

/// The run's load per cpu (R13): the median of each run's starting 1-minute load, divided by the cpu count.
pub fn load_per_cpu(loads: &[f64], cpus: usize) -> f64 {
    median(loads) / cpus as f64
}

/// Every metric judged against its budget. A budget naming a metric the run never measured is an error, not a pass: a gate that silently stops measuring is not a gate.
pub fn judge_all(result: &Report, budgets: &Budgets) -> Result<Outcome, String> {
    if result.loads.is_empty() {
        return Err("no load figures recorded".to_string());
    }

    if let Some(name) = budgets
        .keys()
        .find(|name| !result.metrics.contains_key(*name))
    {
        return Err(format!("budget {name:?} has no measurement"));
    }

    let per_cpu = load_per_cpu(&result.loads, result.cpus);
    let mut misses = Vec::new();
    let mut skipped = Vec::new();

    for (name, value) in &result.metrics {
        let Some(budget) = budgets.get(name) else {
            continue;
        };

        match budget.max_load_per_cpu.get(&result.os) {
            Some(&max) if per_cpu > max => {
                skipped.push(Skipped {
                    metric: name.clone(),
                    load_per_cpu: per_cpu,
                    max_load_per_cpu: max,
                });

                if let Some(miss) = over_limit(*value, budget) {
                    misses.push((name.clone(), miss));
                }
            }
            _ => {
                if let Some(miss) = judge(*value, budget, &result.os) {
                    misses.push((name.clone(), miss));
                }
            }
        }
    }

    Ok(Outcome { misses, skipped })
}

/// The middle value; the mean of the two middle values for an even count.
pub fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();

    sorted.sort_by(f64::total_cmp);

    let mid = sorted.len() / 2;

    if sorted.len() % 2 == 1 {
        sorted[mid]
    } else {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    }
}
