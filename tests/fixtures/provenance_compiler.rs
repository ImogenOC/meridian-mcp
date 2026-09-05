use std::io::{Read, Write};

fn main() {
    let dme = std::path::PathBuf::from(std::env::args().next_back().unwrap());
    let root = dme.parent().unwrap_or(std::path::Path::new("."));
    let consumed_source = std::fs::read(root.join("source.dm")).unwrap();
    let address = std::fs::read_to_string(root.join("compiler.address")).unwrap();
    let mut stream = std::net::TcpStream::connect(address.trim()).unwrap();
    stream.write_all(b"started").unwrap();
    let mut release = [0];
    stream.read_exact(&mut release).unwrap();
    std::fs::write(dme.with_extension("dmb"), consumed_source).unwrap();
}
