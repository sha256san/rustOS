import urllib.request
import re
import os
import tarfile
import shutil
import subprocess
import sys

DEBIAN_POOLS = {
    "neofetch": ("http://ftp.debian.org/debian/pool/main/n/neofetch/", r"^neofetch_7\.[0-9.-]+_all\.deb$"),
    "curl": ("http://ftp.debian.org/debian/pool/main/c/curl/", r"^curl_7\.[0-9.+~a-zA-Z-]+_amd64\.deb$"),
    "libcurl4": ("http://ftp.debian.org/debian/pool/main/c/curl/", r"^libcurl4_7\.[0-9.+~a-zA-Z-]+_amd64\.deb$"),
    "git": ("http://ftp.debian.org/debian/pool/main/g/git/", r"^git_2\.[0-9.+~a-zA-Z-]+_amd64\.deb$"),
    "git-man": ("http://ftp.debian.org/debian/pool/main/g/git/", r"^git-man_2\.[0-9.+~a-zA-Z-]+_all\.deb$"),
    "python3": ("http://ftp.debian.org/debian/pool/main/p/python3-defaults/", r"^python3_3\.[0-9.+~a-zA-Z-]+_amd64\.deb$"),
    "python3-minimal": ("http://ftp.debian.org/debian/pool/main/p/python3-defaults/", r"^python3-minimal_3\.[0-9.+~a-zA-Z-]+_amd64\.deb$"),
    "tree": ("http://ftp.debian.org/debian/pool/main/t/tree/", r"^tree_2\.[0-9.+~a-zA-Z-]+_amd64\.deb$"),
}

def get_pool_info(pkg_name):
    if pkg_name in DEBIAN_POOLS:
        return DEBIAN_POOLS[pkg_name]
    
    if pkg_name in ["ssh", "openssh-client", "openssh-server"]:
        return "http://ftp.debian.org/debian/pool/main/o/openssh/", r"^openssh-client_[0-9.+~a-zA-Z-]+_amd64\.deb$"
    
    first_char = pkg_name[0]
    if pkg_name.startswith("lib"):
        first_char = pkg_name[:4]
    
    pool_url = f"http://ftp.debian.org/debian/pool/main/{first_char}/{pkg_name}/"
    pattern = rf"^{pkg_name}_[0-9.+~a-zA-Z-]+_amd64\.deb$"
    return pool_url, pattern

def find_latest_deb_url(pool_url, prefix_pattern):
    try:
        # Throttle mirror access to avoid rate-limiting
        import time
        time.sleep(0.5)
        req = urllib.request.urlopen(pool_url, timeout=10)
        html = req.read().decode('utf-8', errors='ignore')
        filenames = re.findall(r'href="([^"]+\.deb)"', html)
        matches = [f for f in filenames if re.match(prefix_pattern, f)]
        if not matches:
            all_pattern = prefix_pattern.replace("_amd64.deb", "_all.deb")
            matches = [f for f in filenames if re.match(all_pattern, f)]
        if matches:
            matches.sort()
            return pool_url.rstrip('/') + '/' + matches[-1]
    except Exception as e:
        print(f"Error querying pool {pool_url}: {e}", file=sys.stderr)
    return None

def extract_deb(deb_path, target_dir):
    os.makedirs(target_dir, exist_ok=True)
    with open(deb_path, "rb") as f:
        magic = f.read(8)
        if magic != b"!<arch>\n":
            raise Exception("Not a valid ar archive")
        
        while True:
            header = f.read(60)
            if len(header) < 60:
                break
            name = header[0:16].decode().strip()
            size = int(header[48:58].decode().strip())
            data = f.read(size)
            if size % 2 != 0:
                f.read(1)
            
            if name.startswith("data.tar"):
                ext = "xz" if ".xz" in name else "gz" if ".gz" in name else "zst" if ".zst" in name else ""
                temp_tar = "temp_data.tar"
                with open(temp_tar, "wb") as tf:
                    tf.write(data)
                
                try:
                    with tarfile.open(temp_tar, f"r:{ext}") as tar:
                        members = []
                        for m in tar.getmembers():
                            if m.isdir() or "bin/" in m.name or "sbin/" in m.name:
                                members.append(m)
                        tar.extractall(path=target_dir, members=members)
                except Exception as e:
                    subprocess.run(["tar", "-xf", temp_tar, "-C", target_dir], check=False)
                    for root, dirs, files in os.walk(target_dir, topdown=False):
                        for file in files:
                            p = os.path.join(root, file)
                            if "bin/" not in p and "sbin/" not in p:
                                try: os.remove(p)
                                except: pass
                        for d in dirs:
                            p = os.path.join(root, d)
                            if "bin/" not in p and "sbin/" not in p:
                                try: os.rmdir(p)
                                except: pass
                if os.path.exists(temp_tar):
                    os.remove(temp_tar)

def main():
    if len(sys.argv) < 2:
        print("Usage: fetch_pkg.py <pkg_name>", file=sys.stderr)
        sys.exit(1)
    
    pkg_name = sys.argv[1]
    cache_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), "cache")
    os.makedirs(cache_dir, exist_ok=True)
    
    tar_path = os.path.join(cache_dir, f"{pkg_name}.tar")
    if os.path.exists(tar_path):
        print(tar_path)
        sys.exit(0)
        
    pool_url, pattern = get_pool_info(pkg_name)
    url = find_latest_deb_url(pool_url, pattern)
    if not url:
        print(f"Error: package {pkg_name} not found on mirrors", file=sys.stderr)
        sys.exit(1)
        
    deb_path = os.path.join(cache_dir, f"{pkg_name}.deb")
    print(f"Downloading {url} to {deb_path}", file=sys.stderr)
    try:
        req = urllib.request.urlopen(url, timeout=15)
        with open(deb_path, "wb") as f:
            f.write(req.read())
    except Exception as e:
        print(f"Download failed: {e}", file=sys.stderr)
        sys.exit(1)
        
    staging_dir = os.path.join(cache_dir, f"staging_{pkg_name}")
    if os.path.exists(staging_dir):
        shutil.rmtree(staging_dir)
    os.makedirs(staging_dir)
    
    print(f"Extracting {deb_path}", file=sys.stderr)
    try:
        extract_deb(deb_path, staging_dir)
    except Exception as e:
        print(f"Extraction failed: {e}", file=sys.stderr)
        sys.exit(1)
        
    print(f"Creating uncompressed tar at {tar_path}", file=sys.stderr)
    with tarfile.open(tar_path, "w") as tar:
        for item in os.listdir(staging_dir):
            item_path = os.path.join(staging_dir, item)
            tar.add(item_path, arcname=item)
            
    shutil.rmtree(staging_dir)
    if os.path.exists(deb_path):
        os.remove(deb_path)
        
    print(tar_path)

if __name__ == "__main__":
    main()
