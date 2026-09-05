use std::io::{BufRead, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

fn main() {
    let arguments: Vec<_> = std::env::args_os().collect();
    if arguments.iter().any(|argument| argument == "--leaf") {
        std::thread::sleep(Duration::from_secs(30));
        return;
    }
    if arguments.iter().any(|argument| argument == "--branch") {
        let mut leaf = Command::new(std::env::current_exe().unwrap())
            .arg("--leaf").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
            .spawn().unwrap();
        println!("{}", leaf.id());
        std::io::stdout().flush().unwrap();
        let _ = leaf.wait();
        return;
    }
    let marker = arguments
        .windows(2)
        .find(|pair| pair[0] == "--marker")
        .unwrap()[1]
        .clone();
    let mut leaf = Command::new(std::env::current_exe().unwrap())
        .arg("--branch")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut grandchild = String::new();
    std::io::BufReader::new(leaf.stdout.take().unwrap()).read_line(&mut grandchild).unwrap();
    let grandchild: u32 = grandchild.trim().parse().unwrap();
    std::fs::write(marker, format!("{} {} {grandchild}", std::process::id(), leaf.id())).unwrap();
    println!("RUNTIME_TREE_READY");
    std::io::stdout().flush().unwrap();
    let _ = leaf.wait();
}
