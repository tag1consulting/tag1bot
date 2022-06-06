use std::time::{SystemTime, UNIX_EPOCH};

// Get the time since the unix epoch.
pub fn timestamp_now() -> u64 {
    let start = SystemTime::now();
    start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs()
}

// How long has elapsed since the timestamp.
pub fn elapsed(timestamp: u64) -> u64 {
    timestamp_now() - timestamp
}

// Display "X time ago" style text.
pub fn time_ago(timestamp: u64, precision: bool) -> String {
    let mut seconds: u64 = elapsed(timestamp);
    let days: u64 = seconds / 86400;
    let remainder_string;

    match days {
        0 => match seconds {
            0..=9 => "just now".to_string(),
            10..=59 => seconds.to_string() + " seconds ago",
            60..=119 => "a minute ago".to_string(),
            120..=3599 => {
                let time_string = (seconds / 60).to_string()
                    + " minutes "
                    + match precision {
                        true => {
                            let remainder = seconds % 60;
                            match remainder {
                                0 => "ago",
                                1 => "1 second ago",
                                _ => {
                                    remainder_string = format!("{} seconds ago", remainder);
                                    remainder_string.as_str()
                                }
                            }
                        }
                        false => "ago",
                    };
                time_string
            }
            3600..=7199 => "an hour ago".to_string(),
            _ => {
                let time_string = format!("{} hours ", seconds / 3600)
                    + match precision {
                        true => {
                            let remainder: u64 = (seconds % 3600) / 60;
                            match remainder {
                                0 => "ago",
                                1 => "1 minute ago",
                                _ => {
                                    remainder_string = format!("{} minutes ago", remainder);
                                    remainder_string.as_str()
                                }
                            }
                        }
                        false => "ago",
                    };
                time_string
            }
        },
        1 => {
            let time_string = "1 day ".to_string()
                + match precision {
                    true => {
                        seconds -= 86400;
                        match seconds {
                            0..=119 => "ago",
                            120..=3599 => {
                                remainder_string = format!("{} minutes ago", seconds / 60);
                                remainder_string.as_str()
                            }
                            3600..=7199 => "1 hour ago",
                            _ => {
                                remainder_string = format!("{} hours ago", seconds / 3600);
                                remainder_string.as_str()
                            }
                        }
                    }
                    false => "ago",
                };
            time_string
        }
        2..=6 => {
            let time_string = format!("{} days ", days)
                + match precision {
                    true => {
                        seconds -= 86400 * days;
                        match seconds {
                            0..=7199 => "ago",
                            _ => {
                                remainder_string = format!("{} hours ago", seconds / 3600);
                                remainder_string.as_str()
                            }
                        }
                    }
                    false => "ago",
                };
            time_string
        }
        7 => {
            let time_string = "1 week ".to_string()
                + match precision {
                    true => {
                        let remainder: u64 = (days % 7) / 60;
                        match remainder {
                            0 => "ago",
                            1 => "1 day ago",
                            _ => {
                                remainder_string = format!("{} days ago", remainder);
                                remainder_string.as_str()
                            }
                        }
                    }
                    false => "ago",
                };
            time_string
        }
        8..=30 => {
            let time_string = format!("{} weeks ", (days / 7) as u64)
                + match precision {
                    true => {
                        let remainder: u64 = (days % 7) / 60;
                        match remainder {
                            0 => "ago",
                            1 => "1 day ago",
                            _ => {
                                remainder_string = format!("{} days ago", remainder);
                                remainder_string.as_str()
                            }
                        }
                    }
                    false => "ago",
                };
            time_string
        }
        31..=364 => {
            let time_string = format!("{} months ", (days / 30) as u64)
                + match precision {
                    true => {
                        let day_remainder: u64 = days % 30;
                        match day_remainder {
                            0 => "ago",
                            1 => "1 day ago",
                            2..=6 => {
                                remainder_string = format!("{} days ago", day_remainder);
                                remainder_string.as_str()
                            }
                            _ => {
                                let week_remainder: u64 = day_remainder / 7;
                                match week_remainder {
                                    1 => "1 week ago",
                                    _ => {
                                        remainder_string = format!("{} weeks ago", week_remainder);
                                        remainder_string.as_str()
                                    }
                                }
                            }
                        }
                    }
                    false => "ago",
                };
            time_string
        }
        _ => {
            let time_string = format!("{} years ", days / 365)
                + match precision {
                    true => {
                        let day_remainder = days % 365;
                        match day_remainder {
                            0 => "ago",
                            1 => "1 day ago",
                            2..=6 => {
                                remainder_string = format!("{} days ago", day_remainder);
                                remainder_string.as_str()
                            }
                            _ => {
                                let week_remainder = days % 7;
                                match week_remainder {
                                    0 => "ago",
                                    1 => "1 week ago",
                                    2..=4 => {
                                        remainder_string = format!("{} weeks ago", week_remainder);
                                        remainder_string.as_str()
                                    }
                                    _ => {
                                        let month_remainder = days % 12;
                                        match month_remainder {
                                            0 => "ago",
                                            1 => "1 month ago",
                                            _ => {
                                                remainder_string =
                                                    format!("{} months ago", month_remainder);
                                                remainder_string.as_str()
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    false => "ago",
                };
            time_string
        }
    }
}
