extern crate alloc;

use alloc::vec::Vec;
use crate::println;
use crate::print;
use spin::Mutex;

#[derive(Clone)]
pub struct Package {
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
    pub size: &'static str,
    pub depends: &'static [&'static str],
    pub installed: bool,
}

lazy_static::lazy_static! {
    pub static ref PACKAGES: Mutex<Vec<Package>> = Mutex::new(alloc::vec![
        Package {
            name: "neofetch",
            version: "7.1.0-4",
            description: "Fast, highly customizable system info script",
            size: "82 KB",
            depends: &[],
            installed: false,
        },
        Package {
            name: "curl",
            version: "7.88.1-10",
            description: "command line tool for transferring data with URLs",
            size: "198 KB",
            depends: &["libcurl4"],
            installed: false,
        },
        Package {
            name: "libcurl4",
            version: "7.88.1-10",
            description: "easy-to-use client-side URL transfer library",
            size: "290 KB",
            depends: &[],
            installed: false,
        },
        Package {
            name: "git",
            version: "1:2.39.2-1",
            description: "fast, scalable, distributed revision control system",
            size: "3.2 MB",
            depends: &["git-man"],
            installed: false,
        },
        Package {
            name: "git-man",
            version: "1:2.39.2-1",
            description: "manual pages for git revision control system",
            size: "980 KB",
            depends: &[],
            installed: false,
        },
        Package {
            name: "python3",
            version: "3.11.2-1",
            description: "interactive high-level object-oriented language",
            size: "24 KB",
            depends: &["python3-minimal"],
            installed: false,
        },
        Package {
            name: "python3-minimal",
            version: "3.11.2-1",
            description: "minimal subset of the Python language",
            size: "1.2 MB",
            depends: &[],
            installed: false,
        },
        Package {
            name: "tree",
            version: "2.1.0-1",
            description: "displays directory tree recursively in ASCII format",
            size: "52 KB",
            depends: &[],
            installed: false,
        },
        Package {
            name: "ssh",
            version: "1:9.2p1-2",
            description: "secure shell client metapackage",
            size: "320 KB",
            depends: &["openssh-client"],
            installed: false,
        },
        Package {
            name: "openssh-client",
            version: "1:9.2p1-2",
            description: "secure shell (SSH) client, for secure access to remote machines",
            size: "1.0 MB",
            depends: &[],
            installed: false,
        },
        Package {
            name: "sl",
            version: "5.02-1",
            description: "Correct you if you type sl for ls",
            size: "14 KB",
            depends: &[],
            installed: false,
        },
    ]);
}

fn com2_request(path: &str) -> Result<Vec<u8>, &'static str> {
    // 1. Send the request
    {
        let mut serial = crate::serial2::SERIAL2.lock();
        serial.send_string("REQ:GET:");
        serial.send_string(path);
        serial.send_byte(b'\n');
    }

    // 2. Read the response header: RES:OK:<size>\n or RES:ERROR:<err>\n
    let mut header = alloc::string::String::new();
    loop {
        let b = crate::serial2::SERIAL2.lock().read_byte();
        if b == b'\n' {
            break;
        }
        header.push(b as char);
    }

    if header.starts_with("RES:OK:") {
        let size_str = &header[7..];
        let size = size_str.parse::<usize>().map_err(|_| "Invalid response size")?;
        
        let mut data = Vec::with_capacity(size);
        for _ in 0..size {
            let b = crate::serial2::SERIAL2.lock().read_byte();
            data.push(b);
        }
        Ok(data)
    } else {
        Err("Response error from host proxy")
    }
}

fn extract_json_field(obj: &str, field: &str) -> Option<alloc::string::String> {
    let key = alloc::format!("\"{}\":", field);
    if let Some(pos) = obj.find(&key) {
        let sub = &obj[pos + key.len()..];
        let val_start = sub.find('"')? + 1;
        let val_end = sub[val_start..].find('"')? + val_start;
        Some(alloc::string::String::from(&sub[val_start..val_end]))
    } else {
        None
    }
}

pub fn parse_packages_json(json: &str) -> Vec<Package> {
    use alloc::boxed::Box;
    let mut pkgs = Vec::new();
    for obj in json.split('{') {
        if !obj.contains('}') {
            continue;
        }
        if let Some(name_raw) = extract_json_field(obj, "name") {
            let name = Box::leak(name_raw.into_boxed_str());
            let version = Box::leak(extract_json_field(obj, "version").unwrap_or_default().into_boxed_str());
            let size = Box::leak(extract_json_field(obj, "size").unwrap_or_default().into_boxed_str());
            let description = Box::leak(alloc::format!("Dynamically fetched package {}", name).into_boxed_str());
            
            let mut depends = Vec::new();
            if let Some(dep_start) = obj.find("\"depends\":") {
                let dep_sub = &obj[dep_start..];
                if let Some(arr_start) = dep_sub.find('[') {
                    if let Some(arr_end) = dep_sub.find(']') {
                        let arr_content = &dep_sub[arr_start + 1..arr_end];
                        for d in arr_content.split(',') {
                            let d_trim = d.trim().replace('"', "");
                            if !d_trim.is_empty() {
                                depends.push(Box::leak(d_trim.into_boxed_str()) as &'static str);
                            }
                        }
                    }
                }
            }
            let depends_slice = Box::leak(depends.into_boxed_slice());
            
            pkgs.push(Package {
                name,
                version,
                description,
                size,
                depends: depends_slice,
                installed: false,
            });
        }
    }
    pkgs
}

pub fn apt_update() {
    println!("Get:1 http://153.125.239.34:8081 Packages.json InRelease [256 kB]");
    sleep_delay(2_000_000);
    println!("Hit:2 http://153.125.239.34:8081 packages pool InRelease");
    
    println!("Reading online packages list from mirror (153.125.239.34:8081)...");
    match com2_request("/Packages.json") {
        Ok(json_bytes) => {
            let json_str = alloc::string::String::from_utf8_lossy(&json_bytes);
            let parsed_pkgs = parse_packages_json(&json_str);
            if !parsed_pkgs.is_empty() {
                let mut pkgs = PACKAGES.lock();
                let mut updated_pkgs = parsed_pkgs;
                for new_p in updated_pkgs.iter_mut() {
                    for old_p in pkgs.iter() {
                        if old_p.name == new_p.name {
                            new_p.installed = old_p.installed;
                        }
                    }
                }
                *pkgs = updated_pkgs;
                println!("Reading package lists... Done");
                println!("Building dependency tree... Done");
                println!("Successfully updated package database ({} packages).", pkgs.len());
            } else {
                println!("E: Failed to parse package list.");
            }
        }
        Err(e) => {
            println!("E: Failed to fetch package list over serial network link: {:?}", e);
        }
    }
}

pub fn apt_install(pkg_name: &str) -> Result<(), &'static str> {
    let mut to_install = Vec::new();
    
    // Check dependencies recursively
    {
        let pkgs = PACKAGES.lock();
        if !resolve_dependencies(pkg_name, &*pkgs, &mut to_install) {
            return Err("E: Unable to locate package or resolve dependencies");
        }
    }

    if to_install.is_empty() {
        println!("{} is already the newest version (simulated).", pkg_name);
        return Ok(());
    }

    // Verify or dynamically download packages
    for p in &to_install {
        let staging_path = alloc::format!("/usr/share/apt/unpacked/{}", p.name);
        let needs_download = match crate::vfs::vfs_readdir(&staging_path) {
            Ok(entries) => entries.is_empty(),
            Err(_) => true,
        };

        if needs_download {
            println!("Get:1 http://153.125.239.34:8081/packages/{}.tar [{}]", p.name, p.size);
            println!("Downloading package payload '{}' via serial network link...", p.name);
            let req_path = alloc::format!("/packages/{}.tar", p.name);
            match com2_request(&req_path) {
                Ok(tar_bytes) => {
                    println!("Extracting package payload to staging directory...");
                    let dest_staging = alloc::format!("usr/share/apt/unpacked/{}", p.name);
                    let _ = crate::vfs::vfs_mkdir(&dest_staging, "root", "root", "drwxr-xr-x");
                    if let Err(e) = crate::vfs::parse_tar_to_staging(&tar_bytes, p.name) {
                        println!("E: Failed to unpack staging archive: {}", e);
                        return Err("E: Package unpacking failed");
                    }
                    println!("Download and staging completed ({} bytes).", tar_bytes.len());
                }
                Err(e) => {
                    println!("E: Failed to download package '{}' from host: {:?}", p.name, e);
                    return Err("E: Package download failed");
                }
            }
        }
    }

    println!("Reading package lists... Done");
    println!("Building dependency tree... Done");
    println!("The following additional packages will be installed:");
    for p in &to_install {
        if p.name != pkg_name {
            print!("  {} ", p.name);
        }
    }
    println!();
    
    println!("The following NEW packages will be installed:");
    for p in &to_install {
        print!("  {} ", p.name);
    }
    println!();

    println!("Need to get some archives.");

    println!("Selecting previously unselected packages.");
    for p in &to_install {
        println!("Preparing to unpack .../archives/{}_{}_amd64.deb ...", p.name, p.version);
        println!("Unpacking {} ({}) ...", p.name, p.version);
        sleep_delay(3_000_000);
    }

    println!("Setting up packages...");
    for p in &to_install {
        println!("Setting up {} ({}) ...", p.name, p.version);
        sleep_delay(2_000_000);
        
        // Copy the unpacked package files from staging area to real destination
        if let Err(e) = crate::vfs::vfs_copy_unpacked(p.name) {
            println!("APT Error unpacking package {}: {}", p.name, e);
            return Err("E: Package extraction failed");
        }
    }

    // Mark as installed in DB
    {
        let mut pkgs = PACKAGES.lock();
        for p in &to_install {
            for db_p in pkgs.iter_mut() {
                if db_p.name == p.name {
                    db_p.installed = true;
                    break;
                }
            }
        }
    }

    println!("Processing triggers for libc-bin (simulated) ... Done.");
    Ok(())
}

pub fn apt_remove(pkg_name: &str) -> Result<(), &'static str> {
    let mut pkgs = PACKAGES.lock();
    let mut found = false;
    for p in pkgs.iter_mut() {
        if p.name == pkg_name {
            if !p.installed {
                println!("Package '{}' is not installed, so not removed", pkg_name);
                return Ok(());
            }
            p.installed = false;
            found = true;
            break;
        }
    }

    if found {
        println!("Reading package lists... Done");
        println!("Building dependency tree... Done");
        println!("The following packages will be REMOVED:");
        println!("  {}", pkg_name);
        println!("Removing {} ({}) ...", pkg_name, "simulated");
        sleep_delay(3_000_000);
        
        // Remove VFS file
        let bin_path = alloc::format!("/usr/bin/{}", pkg_name);
        let _ = crate::vfs::vfs_remove(&bin_path);
        println!("Processing triggers... Done");
        Ok(())
    } else {
        Err("E: Package not found")
    }
}

pub fn apt_show(pkg_name: &str) {
    let pkgs = PACKAGES.lock();
    for p in pkgs.iter() {
        if p.name == pkg_name {
            println!("Package: {}", p.name);
            println!("Version: {}", p.version);
            println!("Priority: optional");
            println!("Section: utils");
            println!("Installed-Size: {}", p.size);
            if !p.depends.is_empty() {
                print!("Depends: ");
                for (i, dep) in p.depends.iter().enumerate() {
                    if i > 0 { print!(", "); }
                    print!("{}", dep);
                }
                println!();
            }
            println!("Download-Size: {}", p.size);
            println!("Description: {}", p.description);
            println!("Status: {}", if p.installed { "installed" } else { "not-installed" });
            return;
        }
    }
    println!("N: Unable to locate package {}", pkg_name);
}

pub fn apt_list(only_installed: bool) {
    let pkgs = PACKAGES.lock();
    for p in pkgs.iter() {
        if only_installed && !p.installed {
            continue;
        }
        let inst_str = if p.installed { " [installed]" } else { "" };
        println!("{}/{}/noble {} amd64{}", p.name, "noble", p.version, inst_str);
    }
}

fn resolve_dependencies(name: &str, db: &[Package], list: &mut Vec<Package>) -> bool {
    // Find in DB
    let mut pkg_opt = None;
    for p in db {
        if p.name == name {
            pkg_opt = Some(p.clone());
            break;
        }
    }

    let pkg = match pkg_opt {
        Some(p) => p,
        None => return false,
    };

    if pkg.installed {
        return true; // Already installed, no need to add to list
    }

    // Check if already in list
    if list.iter().any(|p| p.name == name) {
        return true;
    }

    // Resolve dependencies first
    for dep in pkg.depends {
        if !resolve_dependencies(dep, db, list) {
            return false;
        }
    }

    // Add self to list
    list.push(pkg);
    true
}

fn sleep_delay(count: usize) {
    for _ in 0..count {
        x86_64::instructions::nop();
    }
}
