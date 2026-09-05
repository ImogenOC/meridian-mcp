fn main() {
    use std::io::{BufRead, Write};
    if let Ok(path) = std::env::var("COLLECTOR_READY") {
        std::fs::write(path, std::process::id().to_string()).unwrap();
    }
    let mode = std::env::var("COLLECTOR_MODE").unwrap_or_default();
    if mode == "respond" {
        for line in std::io::stdin().lock().lines() {
            let line = line.unwrap();
            let id = line
                .split("\"id\":")
                .nth(1)
                .unwrap()
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            println!("{{\"schema_version\":2,\"id\":{id},\"ok\":true,\"result\":{{\"state\":\"stopped\"}}}}");
            std::io::stdout().flush().unwrap();
        }
        std::process::exit(37);
    }
    if mode == "stderr" {
        let mut stderr = std::io::stderr().lock();
        write!(stderr, "{}€{}\n", "x".repeat(4095), "x".repeat(10000)).unwrap();
        for index in 0..70 {
            writeln!(stderr, "line{index}").unwrap();
        }
        stderr.flush().unwrap();
    }
    std::thread::sleep(std::time::Duration::from_secs(30));
}
