pub fn format_threshold_value(value: f64) -> String {
    if (value - value.round()).abs() < f64::EPSILON {
        format!("{:.0}", value)
    } else {
        value.to_string()
    }
}
