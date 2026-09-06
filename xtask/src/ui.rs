use anyhow::Result;

pub fn print_test_start(msg: &str) {
    let width = msg.chars().count() + 2;
    let border = "━".repeat(width);
    println!("\n┏{border}┓");
    println!("┃ {msg} ┃");
    println!("┗{border}┛");
}

pub fn print_test_end(msg: &str, res: Result<()>) -> Result<()> {
    let (icon, status) = if res.is_ok() {
        ("✔", "SUCCESS")
    } else {
        ("✖", "FAILED")
    };
    let end_msg = format!("{icon} {status}: {msg}");

    let width = end_msg.chars().count() + 2;
    let border = "━".repeat(width);
    // Pad the end message with spaces so the box width perfectly matches the header
    let padding = width.saturating_sub(end_msg.chars().count() + 2);
    let spaces = " ".repeat(padding);

    println!("┏{border}┓");
    println!("┃ {end_msg}{spaces} ┃");
    println!("┗{border}┛\n");

    res
}
