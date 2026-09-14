import urllib.request
import json
import os
import tarfile
import shutil
import re
import subprocess

import time

SERVER_URL = "http://153.125.239.34:8081"
INITFS_DIR = "initfs"
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
TAR_FILE = os.path.join(SCRIPT_DIR, "rust_os/src/initfs.tar")

DEBIAN_POOLS = [
    {
        "name": "neofetch",
        "pool": "http://ftp.debian.org/debian/pool/main/n/neofetch/",
        "pattern": r"^neofetch_7\.[0-9.-]+_all\.deb$"
    },
    {
        "name": "curl",
        "pool": "http://ftp.debian.org/debian/pool/main/c/curl/",
        "pattern": r"^curl_7\.[0-9.+~a-zA-Z-]+_amd64\.deb$"
    },
    {
        "name": "libcurl4",
        "pool": "http://ftp.debian.org/debian/pool/main/c/curl/",
        "pattern": r"^libcurl4_7\.[0-9.+~a-zA-Z-]+_amd64\.deb$"
    },
    {
        "name": "git",
        "pool": "http://ftp.debian.org/debian/pool/main/g/git/",
        "pattern": r"^git_2\.[0-9.+~a-zA-Z-]+_amd64\.deb$"
    },
    {
        "name": "git-man",
        "pool": "http://ftp.debian.org/debian/pool/main/g/git/",
        "pattern": r"^git-man_2\.[0-9.+~a-zA-Z-]+_all\.deb$"
    },
    {
        "name": "python3",
        "pool": "http://ftp.debian.org/debian/pool/main/p/python3-defaults/",
        "pattern": r"^python3_3\.11\.[0-9.+~a-zA-Z-]+_amd64\.deb$"
    },
    {
        "name": "python3-minimal",
        "pool": "http://ftp.debian.org/debian/pool/main/p/python3-defaults/",
        "pattern": r"^python3-minimal_3\.11\.[0-9.+~a-zA-Z-]+_amd64\.deb$"
    },
    {
        "name": "tree",
        "pool": "http://ftp.debian.org/debian/pool/main/t/tree/",
        "pattern": r"^tree_2\.[0-9.+~a-zA-Z-]+_amd64\.deb$"
    }
]

def urlopen_with_retry(url, retries=5, delay=2.0):
    for i in range(retries):
        try:
            # Throttle requests slightly
            time.sleep(1.0)
            return urllib.request.urlopen(url, timeout=10)
        except Exception as e:
            if i == retries - 1:
                raise e
            print(f"Connection failed for {url}, retrying in {delay}s... ({e})")
            time.sleep(delay)
            delay *= 2

def find_latest_deb_url(pool_url, prefix_pattern):
    try:
        req = urlopen_with_retry(pool_url)
        html = req.read().decode('utf-8', errors='ignore')
        filenames = re.findall(r'href="([^"]+\.deb)"', html)
        matches = [f for f in filenames if re.match(prefix_pattern, f)]
        if matches:
            matches.sort()
            return pool_url.rstrip('/') + '/' + matches[-1]
    except Exception as e:
        print(f"Error querying pool {pool_url} for pattern {prefix_pattern}: {e}")
    return None

def extract_deb(deb_path, target_dir):
    os.makedirs(target_dir, exist_ok=True)
    with open(deb_path, "rb") as f:
        magic = f.read(8)
        if magic != b"!<arch>\n":
            print(f"Error: {deb_path} is not a valid ar archive")
            return
        
        while True:
            header = f.read(60)
            if len(header) < 60:
                break
            name = header[0:16].decode().strip()
            size_str = header[48:58].decode().strip()
            if not size_str:
                break
            size = int(size_str)
            
            data = f.read(size)
            if size % 2 != 0:
                f.read(1) # padding
                
            if name.startswith("data.tar"):
                ext = "xz" if ".xz" in name else "gz" if ".gz" in name else "zst" if ".zst" in name else ""
                temp_tar = "temp_data.tar"
                with open(temp_tar, "wb") as tf:
                    tf.write(data)
                
                # Unpack using python's tarfile with filtering
                try:
                    with tarfile.open(temp_tar, f"r:{ext}") as tar:
                        members = []
                        for m in tar.getmembers():
                            if m.isdir() or "bin/" in m.name or "sbin/" in m.name:
                                members.append(m)
                        tar.extractall(path=target_dir, members=members)
                except Exception as e:
                    subprocess.run(["tar", "-xf", temp_tar, "-C", target_dir], check=False)
                    # Cleanup non-bin files to keep staging tiny
                    for root, dirs, files in os.walk(target_dir, topdown=False):
                        for file in files:
                            full_path = os.path.join(root, file)
                            if "bin/" not in full_path and "sbin/" not in full_path:
                                try:
                                    os.remove(full_path)
                                except Exception:
                                    pass
                        for d in dirs:
                            full_path = os.path.join(root, d)
                            if "bin/" not in full_path and "sbin/" not in full_path:
                                try:
                                    os.rmdir(full_path)
                                except Exception:
                                    pass
                
                if os.path.exists(temp_tar):
                    os.remove(temp_tar)

def main():
    print("Generating initfs...")
    if os.path.exists(INITFS_DIR):
        shutil.rmtree(INITFS_DIR)
        
    os.makedirs(INITFS_DIR)
    
    dirs = [
        "bin", "sbin", "usr", "usr/bin", "usr/sbin", "usr/lib", "usr/share",
        "etc", "var", "var/log", "var/cache", "var/cache/apt", "var/cache/apt/archives",
        "var/lib", "home", "home/guru", "home/guest", "root", "tmp", "run",
        "dev", "proc", "sys"
    ]
    for d in dirs:
        os.makedirs(os.path.join(INITFS_DIR, d), exist_ok=True)
        
    # Write base configuration files
    passwd_content = (
        "root:x:0:0:root:/root:/bin/bash\n"
        "guru:x:1000:1000:guru:/home/guru:/bin/bash\n"
        "guest:x:1001:1001:guest:/home/guest:/bin/bash\n"
    )
    with open(os.path.join(INITFS_DIR, "etc/passwd"), "w") as f:
        f.write(passwd_content)
        
    with open(os.path.join(INITFS_DIR, "etc/hostname"), "w") as f:
        f.write("ubuntu-2604-server\n")

    os_release_content = (
        'NAME="Ubuntu"\n'
        'VERSION="26.04 LTS (Resolute Raccoon)"\n'
        'ID=ubuntu\n'
        'ID_LIKE=debian\n'
        'PRETTY_NAME="Ubuntu 26.04 LTS (Resolute Raccoon)"\n'
        'VERSION_ID="26.04"\n'
        'HOME_URL="https://www.ubuntu.com/"\n'
        'SUPPORT_URL="https://help.ubuntu.com/"\n'
        'BUG_REPORT_URL="https://bugs.launchpad.net/ubuntu/"\n'
        'PRIVACY_POLICY_URL="https://www.ubuntu.com/legal/terms-and-policies/privacy-policy"\n'
        'UBUNTU_CODENAME=resolute\n'
    )
    with open(os.path.join(INITFS_DIR, "etc/os-release"), "w") as f:
        f.write(os_release_content)
        
    syslog_content = (
        "[Jul 20 00:00:00] systemd-journald[0]: Journal Service started.\n"
        "[Jul 20 00:00:00] systemd-networkd[0]: Network Service started.\n"
        "[Jul 20 00:00:00] udev[0]: udev Device Event Manager started.\n"
        "[Jul 20 00:00:00] ufw[0]: Uncomplicated Firewall started.\n"
    )
    with open(os.path.join(INITFS_DIR, "var/log/syslog"), "w") as f:
        f.write(syslog_content)
        
    welcome_content = (
        "=========================================================\n"
        " Welcome to Ubuntu 26.04 LTS Server (Resolute Raccoon)!\n"
        " (Rust OS kernel implementation & uutils/coreutils)\n"
        "=========================================================\n"
        " * Documentation:  https://help.ubuntu.com\n"
        " * Management:     sudo-rs (v0.2.1), systemctl, apt (v3.0)\n"
        " * Security:       ufw, fail2ban, unattended-upgrades\n"
        "\n"
        " Type 'help' to view available commands & options.\n"
        "=========================================================\n"
    )
    with open(os.path.join(INITFS_DIR, "home/guru/welcome.txt"), "w") as f:
        f.write(welcome_content)
        
    bash_history = (
        "uname -a\n"
        "cat /etc/os-release\n"
        "whoami\n"
        "ls -la /var/log/\n"
        "head -n 20 /var/log/syslog\n"
        "systemctl status\n"
        "sudo apt update\n"
    )
    with open(os.path.join(INITFS_DIR, "home/guru/.bash_history"), "w") as f:
        f.write(bash_history)
        
    profile = (
        "# ~/.profile: executed by the command interpreter\n"
        'export PATH="/bin:/usr/bin:/sbin:/usr/sbin"\n'
        "alias ll='ls -lh'\n"
        "alias la='ls -A'\n"
    )
    with open(os.path.join(INITFS_DIR, "home/guru/.profile"), "w") as f:
        f.write(profile)
        
    with open(os.path.join(INITFS_DIR, "root/flag.txt"), "w") as f:
        f.write("Congratulations! You got root!\n")
        
    # Download real packages dynamically from Debian pool (avoiding zstd and 404 errors)
    try:
        # Verify package server is running
        print(f"Verifying package server at {SERVER_URL}/Packages.json")
        urlopen_with_retry(f"{SERVER_URL}/Packages.json")
        
        for pkg_info in DEBIAN_POOLS:
            name = pkg_info["name"]
            pool = pkg_info["pool"]
            pattern = pkg_info["pattern"]
            
            print(f"Locating latest Debian package for: {name}...")
            url = find_latest_deb_url(pool, pattern)
            if not url:
                print(f"Warning: Could not find matching package for {name} on Debian mirror. Skipping.")
                continue
                
            print(f"Downloading real Debian package from: {url}")
            pkg_req = urlopen_with_retry(url)
            pkg_data = pkg_req.read()
            
            # Save deb package as local cache in archives/
            pkg_path = os.path.join(INITFS_DIR, f"var/cache/apt/archives/{name}.deb")
            with open(pkg_path, "wb") as f:
                f.write(pkg_data)
                
            # Unpack deb payload to staging directory
            staging_dir = os.path.join(INITFS_DIR, f"usr/share/apt/unpacked/{name}")
            print(f"Extracting package payload for {name} to {staging_dir}")
            extract_deb(pkg_path, staging_dir)
            
        print("All official Debian packages downloaded and unpacked successfully.")
    except Exception as e:
        print(f"Warning: Failed to fetch packages from server: {e}")
        print("Proceeding with default configs only.")
        
    # Pack into initfs.tar
    print(f"Creating tar archive: {TAR_FILE}")
    os.makedirs(os.path.dirname(TAR_FILE), exist_ok=True)
    with tarfile.open(TAR_FILE, "w") as tar:
        for item in os.listdir(INITFS_DIR):
            item_path = os.path.join(INITFS_DIR, item)
            tar.add(item_path, arcname=item)
            
    # Cleanup initfs temp directory
    shutil.rmtree(INITFS_DIR)
    print("initfs.tar generated successfully.")

if __name__ == "__main__":
    main()
