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
}

#[derive(Debug, PartialEq)]
pub enum Miss {
    OverLimit { limit: f64 },
    Regressed { baseline: f64, ceiling: f64 },
}

/// `None` is a pass.
pub fn judge(value: f64, budget: &Budget, os: &str) -> Option<Miss> {
    if let Some(limit) = budget.limit
        && value > limit
    {
        return Some(Miss::OverLimit { limit });
    }

    let baseline = *budget.baseline.get(os)?;
    let ceiling = baseline * (1.0 + REGRESSION) + budget.noise;

    (value > ceiling).then_some(Miss::Regressed { baseline, ceiling })
}

/// Every metric that misses its budget, by name. A budget with no measured metric is an error: a gate that silently stops measuring is not a gate.
pub fn misses(result: &Report, budgets: &Budgets) -> Result<Vec<(String, Miss)>, String> {
    if let Some(name) = budgets
        .keys()
        .find(|name| !result.metrics.contains_key(*name))
    {
        return Err(format!("budget {name:?} has no measurement"));
    }

    Ok(result
        .metrics
        .iter()
        .filter_map(|(name, value)| {
            let miss = judge(*value, budgets.get(name)?, &result.os)?;

            Some((name.clone(), miss))
        })
        .collect())
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
