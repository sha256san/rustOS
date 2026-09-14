use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write, BufRead, BufReader};

fn handle_client(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    if let Ok(_) = stream.read(&mut buffer) {
        let request = String::from_utf8_lossy(&buffer);
        let first_line = request.lines().next().unwrap_or("");
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() >= 2 {
            let method = parts[0];
            let path = parts[1];
            if method == "GET" {
                if path == "/Packages.json" {
                    let metadata = r#"[
                        {"name": "neofetch", "version": "7.1.0-4", "size": "82 KB", "depends": []},
                        {"name": "curl", "version": "7.88.1-10", "size": "198 KB", "depends": ["libcurl4"]},
                        {"name": "libcurl4", "version": "7.88.1-10", "size": "290 KB", "depends": []},
                        {"name": "git", "version": "1:2.39.2-1", "size": "3.2 MB", "depends": ["git-man"]},
                        {"name": "git-man", "version": "1:2.39.2-1", "size": "980 KB", "depends": []},
                        {"name": "python3", "version": "3.11.2-1", "size": "24 KB", "depends": ["python3-minimal"]},
                        {"name": "python3-minimal", "version": "3.11.2-1", "size": "1.2 MB", "depends": []},
                        {"name": "tree", "version": "2.1.0-1", "size": "52 KB", "depends": []},
                        {"name": "ssh", "version": "1:9.2p1-2", "size": "320 KB", "depends": ["openssh-client"]},
                        {"name": "openssh-client", "version": "1:9.2p1-2", "size": "1.0 MB", "depends": []}
                    ]"#;
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        metadata.len(),
                        metadata
                    );
                    let _ = stream.write_all(response.as_bytes());
                } else if path.starts_with("/packages/") {
                    let pkg_name = path.split('/').last().unwrap_or("").replace(".deb", "");
                    let body = format!("MOCK DEB PACKAGE FOR {}", pkg_name);
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(response.as_bytes());
                } else {
                    let body = "Not Found";
                    let response = format!(
                        "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(response.as_bytes());
                }
            }
        }
    }
}

fn handle_serial_stream(stream: &mut TcpStream) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    while reader.read_line(&mut line)? > 0 {
        let request = line.trim();
        if request.starts_with("REQ:GET:") {
            let path = &request[8..];
            println!("Serial Proxy: Received request for path '{}'", path);
            if path == "/Packages.json" {
                let metadata = r#"[
                    {"name": "neofetch", "version": "7.1.0-4", "size": "82 KB", "depends": []},
                    {"name": "curl", "version": "7.88.1-10", "size": "198 KB", "depends": ["libcurl4"]},
                    {"name": "libcurl4", "version": "7.88.1-10", "size": "290 KB", "depends": []},
                    {"name": "git", "version": "1:2.39.2-1", "size": "3.2 MB", "depends": ["git-man"]},
                    {"name": "git-man", "version": "1:2.39.2-1", "size": "980 KB", "depends": []},
                    {"name": "python3", "version": "3.11.2-1", "size": "24 KB", "depends": ["python3-minimal"]},
                    {"name": "python3-minimal", "version": "3.11.2-1", "size": "1.2 MB", "depends": []},
                    {"name": "tree", "version": "2.1.0-1", "size": "52 KB", "depends": []},
                    {"name": "ssh", "version": "1:9.2p1-2", "size": "320 KB", "depends": ["openssh-client"]},
                    {"name": "openssh-client", "version": "1:9.2p1-2", "size": "1.0 MB", "depends": []}
                ]"#;
                let header = format!("RES:OK:{}\n", metadata.len());
                stream.write_all(header.as_bytes())?;
                stream.write_all(metadata.as_bytes())?;
                stream.flush()?;
                println!("Serial Proxy: Sent Packages.json ({} bytes)", metadata.len());
            } else if path.starts_with("/packages/") && path.ends_with(".tar") {
                let pkg_name = path.split('/').last().unwrap_or("").replace(".tar", "");
                println!("Serial Proxy: Dynamically building package tarball for '{}'...", pkg_name);
                
                let exe_dir = std::env::current_dir().unwrap_or_default();
                let script_path = exe_dir.join("fetch_pkg.py");
                
                let output = std::process::Command::new("python3")
                    .arg(script_path)
                    .arg(&pkg_name)
                    .output();
                
                match output {
                    Ok(out) if out.status.success() => {
                        let tar_path_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        match std::fs::read(&tar_path_str) {
                            Ok(tar_bytes) => {
                                let header = format!("RES:OK:{}\n", tar_bytes.len());
                                stream.write_all(header.as_bytes())?;
                                stream.write_all(&tar_bytes)?;
                                stream.flush()?;
                                println!("Serial Proxy: Sent dynamically compiled tarball for '{}' ({} bytes)", pkg_name, tar_bytes.len());
                            }
                            Err(e) => {
                                println!("Serial Proxy: Error reading generated tarball '{}': {:?}", tar_path_str, e);
                                let _ = stream.write_all(b"RES:ERROR:read_error\n");
                            }
                        }
                    }
                    Ok(out) => {
                        let err_msg = String::from_utf8_lossy(&out.stderr);
                        println!("Serial Proxy: Python helper script failed: {}", err_msg);
                        let _ = stream.write_all(b"RES:ERROR:fetch_failed\n");
                    }
                    Err(e) => {
                        println!("Serial Proxy: Failed to execute python3 fetch_pkg.py: {:?}", e);
                        let _ = stream.write_all(b"RES:ERROR:execution_error\n");
                    }
                }
            } else {
                let _ = stream.write_all(b"RES:ERROR:not_found\n");
            }
        }
        line.clear();
    }
    Ok(())
}

fn start_serial_proxy() {
    std::thread::spawn(|| {
        println!("Serial Proxy: Starting COM2 network proxy...");
        loop {
            match TcpStream::connect("127.0.0.1:12345") {
                Ok(mut stream) => {
                    println!("Serial Proxy: Connected to QEMU COM2 TCP socket!");
                    if let Err(e) = handle_serial_stream(&mut stream) {
                        println!("Serial Proxy: Connection error: {:?}", e);
                    }
                    println!("Serial Proxy: Disconnected. Retrying...");
                }
                Err(_) => {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                }
            }
        }
    });
}

fn main() {
    // Start QEMU COM2 network proxy thread
    start_serial_proxy();

    let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
    println!("Rust Package Server running at http://127.0.0.1:8080");
    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            std::thread::spawn(|| handle_client(stream));
        }
    }
}
