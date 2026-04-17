use rust_decimal::Decimal;

#[allow(dead_code)]
pub type Money = Decimal;

/// Convert a Decimal dollar amount to integer cents.
#[allow(dead_code)]
pub fn to_cents(amount: Money) -> i64 {
    (amount * Decimal::from(100))
        .round()
        .to_string()
        .parse::<i64>()
        .unwrap_or(0)
}

/// Convert integer cents to a Decimal dollar amount.
#[allow(dead_code)]
pub fn from_cents(cents: i64) -> Money {
    Decimal::from(cents) / Decimal::from(100)
}

/// Parse a cents string (e.g. "1099") to i64.
#[allow(dead_code)]
pub fn parse_cents(s: &str) -> Option<i64> {
    s.trim().parse::<i64>().ok()
}

/// Format cents as a display string (e.g. "$10.99").
#[allow(dead_code)]
pub fn format_dollars(cents: i64) -> String {
    format!("${:.2}", from_cents(cents))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn cents_round_trip() {
        let amount = Decimal::from_str("10.99").unwrap();
        assert_eq!(to_cents(amount), 1099);
        assert_eq!(from_cents(1099), Decimal::from_str("10.99").unwrap());
    }
}
