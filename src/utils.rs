use std::fmt::Write;

pub fn fmt_game_message(raw: &str) -> String {
    let mut buf = "<span class='m-white'>".to_string();
    let mut expecting_control = false;
    let mut color_class;
    for char in raw.chars() {
        match char {
            '$' => expecting_control = true,
            c if expecting_control => {
                expecting_control = false;
                match c {
                    'a' => color_class = "m-aquamarine",
                    'b' => color_class = "m-blue",
                    'c' => color_class = "m-concrete",
                    'g' => color_class = "m-grey",
                    'o' => color_class = "m-gold",
                    'p' => color_class = "m-pink",
                    'r' => color_class = "m-red",
                    '.' => color_class = "m-white",
                    _ => continue,
                }
                write!(&mut buf, "</span><span class='{}'>", color_class).unwrap();
            },
            c => buf.push(c),
        }
    }
    buf
}

pub fn fmt_bool(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

pub fn fmt_size(size: u32) -> String {
    let (prec, fac, suffix) = match size {
        0..1000 => (0, 1.0, ""),
        1000..1_000_000 => (1, 1000.0, " KB"),
        1_000_000..1_000_000_000 => (2, 1_000_000.0, " MB"),
        1_000_000_000..=u32::MAX => (3, 1_000_000_000.0, " GB"),
    };
    format!("{:.*}{suffix}", prec, (size as f32) / fac)
}

pub fn fmt_duration(seconds: i64) -> String {
    let times = [(1, 60, "s"), (60, 60, "m"), (60 * 60, 24, "h"),
        (24 * 60 * 60, 7, "d"), (7 * 24 * 60 * 60, 1, "w")];

    let mut parts = vec![];
    for (d, m, s) in &times {
        if seconds / d % m != 0 {
            parts.push(format!("{}{s}", seconds / d % m));
        }
    }

    parts.reverse();
    parts.truncate(3); // Prevent "3w 5d 12h 8m 34s", "3w 5d 12h" is good enough!
    parts.join(" ")
}
