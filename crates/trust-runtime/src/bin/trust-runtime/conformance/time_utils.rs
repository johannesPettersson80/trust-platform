fn elapsed_ms(duration: std::time::Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn is_valid_case_id(id: &str, category: &str) -> bool {
    let prefix = format!("cfm_{category}_");
    let Some(body) = id.strip_prefix(&prefix) else {
        return false;
    };
    let mut tokens = body.split('_').collect::<Vec<_>>();
    let Some(sequence) = tokens.pop() else {
        return false;
    };
    !tokens.is_empty()
        && tokens.iter().all(|token| {
            !token.is_empty()
                && token
                    .chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
        })
        && sequence.len() == 3
        && sequence.chars().all(|ch| ch.is_ascii_digit())
}

struct UtcParts {
    rfc3339: String,
    compact: String,
}

fn now_utc_parts() -> UtcParts {
    let unix_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let (year, month, day, hour, minute, second) = split_unix_utc(unix_secs);
    UtcParts {
        rfc3339: format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"),
        compact: format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}Z"),
    }
}

fn split_unix_utc(unix_secs: i64) -> (i64, i64, i64, i64, i64, i64) {
    let days = unix_secs.div_euclid(86_400);
    let seconds_in_day = unix_secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_in_day / 3600;
    let minute = (seconds_in_day % 3600) / 60;
    let second = seconds_in_day % 60;
    (year, month, day, hour, minute, second)
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}
