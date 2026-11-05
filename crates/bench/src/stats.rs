//! Small, pure statistics helpers (unit-tested).

pub(crate) fn mean(v: &[f64]) -> Option<f64> {
    if v.is_empty() {
        None
    } else {
        Some(v.iter().sum::<f64>() / v.len() as f64)
    }
}

pub(crate) fn median(v: &[f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len();
    Some(if n % 2 == 1 {
        s[n / 2]
    } else {
        (s[n / 2 - 1] + s[n / 2]) / 2.0
    })
}

pub(crate) fn max(v: &[f64]) -> Option<f64> {
    v.iter().copied().max_by(f64::total_cmp)
}

/// Coefficient of variation in percent (population standard deviation / mean).
pub(crate) fn cv_percent(v: &[f64]) -> Option<f64> {
    let m = mean(v)?;
    if v.len() < 2 || m <= 0.0 {
        return None;
    }
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64;
    Some(var.sqrt() / m * 100.0)
}

/// Change in percent between the mean of the last 10 % and the first 10 % of the points
/// (at least one point each). Negative = slower at the end. `None` with fewer than 2 points.
pub(crate) fn first_last_decile_change_percent(v: &[f64]) -> Option<f64> {
    if v.len() < 2 {
        return None;
    }
    let k = (v.len() / 10).max(1);
    let first = mean(&v[..k])?;
    let last = mean(&v[v.len() - k..])?;
    if first <= 0.0 {
        return None;
    }
    Some((last / first - 1.0) * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_stats() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert_eq!(median(&[]), None);
        assert_eq!(mean(&[1.0, 3.0]), Some(2.0));
        assert_eq!(max(&[1.0, 3.0, 2.0]), Some(3.0));
        assert_eq!(cv_percent(&[5.0, 5.0, 5.0]), Some(0.0));
        let cv = cv_percent(&[9.0, 11.0]).expect("cv");
        assert!((cv - 10.0).abs() < 1e-9);
        assert_eq!(cv_percent(&[1.0]), None);
    }

    #[test]
    fn decile_change() {
        // 20 points: first 2 average 100, last 2 average 80 → −20 %.
        let mut v = vec![100.0; 18];
        v.extend([80.0, 80.0]);
        let c = first_last_decile_change_percent(&v).expect("change");
        assert!((c + 20.0).abs() < 1e-9, "{c}");
        // Short series use one point at each end.
        let c = first_last_decile_change_percent(&[100.0, 110.0]).expect("change");
        assert!((c - 10.0).abs() < 1e-9);
        assert_eq!(first_last_decile_change_percent(&[1.0]), None);
    }
}
