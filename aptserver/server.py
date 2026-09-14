#!/usr/bin/env python3
"""
Standalone Integrated APT Repository & Proxy Server for rust_os
Supports both HTTP (Port 8080) and COM2 Serial TCP Proxy (Port 12345).
"""

import os
import sys
import json
import time
import socket
import threading
import subprocess
from http.server import HTTPServer, BaseHTTPRequestHandler

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
CACHE_DIR = os.path.join(SCRIPT_DIR, "cache")

HTTP_PORT = int(os.environ.get("HTTP_PORT", 8080))
SERIAL_PORT = int(os.environ.get("SERIAL_PORT", 12345))

DEFAULT_PACKAGES = [
    {
        "name": "neofetch",
        "version": "7.1.0-4",
        "size": "82 KB",
        "depends": []
    },
    {
        "name": "curl",
        "version": "7.88.1-10",
        "size": "198 KB",
        "depends": ["libcurl4"]
    },
    {
        "name": "libcurl4",
        "version": "7.88.1-10",
        "size": "290 KB",
        "depends": []
    },
    {
        "name": "git",
        "version": "2.39.2-1",
        "size": "3.2 MB",
        "depends": ["git-man"]
    },
    {
        "name": "git-man",
        "version": "2.39.2-1",
        "size": "980 KB",
        "depends": []
    },
    {
        "name": "python3",
        "version": "3.11.2-1",
        "size": "24 KB",
        "depends": ["python3-minimal"]
    },
    {
        "name": "python3-minimal",
        "version": "3.11.2-1",
        "size": "1.2 MB",
        "depends": []
    },
    {
        "name": "tree",
        "version": "2.1.0-1",
        "size": "52 KB",
        "depends": []
    },
    {
        "name": "ssh",
        "version": "1:9.2p1-2",
        "size": "320 KB",
        "depends": ["openssh-client"]
    },
    {
        "name": "openssh-client",
        "version": "1:9.2p1-2",
        "size": "1.0 MB",
        "depends": []
    },
    {
        "name": "sl",
        "version": "5.02-1",
        "size": "14 KB",
        "depends": []
    }
]

def ensure_cache():
    os.makedirs(CACHE_DIR, exist_ok=True)
    pkgs_json_path = os.path.join(CACHE_DIR, "Packages.json")
    if not os.path.exists(pkgs_json_path):
        with open(pkgs_json_path, "w", encoding="utf-8") as f:
            json.dump(DEFAULT_PACKAGES, f, indent=2)
        print(f"[Cache] Initialized default Packages.json at {pkgs_json_path}")

def get_file_content(path):
    ensure_cache()
    clean_path = path.lstrip("/")
    
    if clean_path in ["Packages.json", ""]:
        file_path = os.path.join(CACHE_DIR, "Packages.json")
    elif clean_path.startswith("packages/") and clean_path.endswith(".tar"):
        pkg_name = clean_path[len("packages/"):-len(".tar")]
        file_path = os.path.join(CACHE_DIR, f"{pkg_name}.tar")
        if not os.path.exists(file_path):
            print(f"[Cache] On-demand fetching package tar for '{pkg_name}'...")
            fetch_script = os.path.join(SCRIPT_DIR, "fetch_pkg.py")
            res = subprocess.run([sys.executable, fetch_script, pkg_name], capture_output=True, text=True)
            if res.returncode != 0:
                print(f"[Cache] Error fetching {pkg_name}: {res.stderr}")
                return None
            file_path = res.stdout.strip()
    else:
        file_path = os.path.join(CACHE_DIR, clean_path)

    if os.path.exists(file_path) and os.path.isfile(file_path):
        with open(file_path, "rb") as f:
            return f.read()
    return None

class AptHTTPRequestHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        content = get_file_content(self.path)
        if content is not None:
            self.send_response(200)
            if self.path.endswith(".json") or self.path in ["/Packages.json", "/"]:
                self.send_header("Content-Type", "application/json")
            else:
                self.send_header("Content-Type", "application/x-tar")
            self.send_header("Content-Length", str(len(content)))
            self.end_headers()
            self.wfile.write(content)
        else:
            self.send_response(404)
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            self.wfile.write(b"404 Not Found")

    def log_message(self, format, *args):
        print(f"[HTTP 8080] {self.address_string()} - {format%args}")

def run_http_server():
    server_address = ("0.0.0.0", HTTP_PORT)
    httpd = HTTPServer(server_address, AptHTTPRequestHandler)
    print(f"========================================================")
    print(f" APT HTTP Server listening on http://0.0.0.0:{HTTP_PORT}")
    print(f"========================================================")
    httpd.serve_forever()

def handle_serial_client(conn, addr):
    print(f"[Serial TCP 12345] Client connected from {addr}")
    try:
        buffer = ""
        while True:
            data = conn.recv(1024)
            if not data:
                break
            buffer += data.decode("utf-8", errors="ignore")
            while "\n" in buffer:
                line, buffer = buffer.split("\n", 1)
                line = line.strip()
                if line.startswith("REQ:GET:"):
                    req_path = line[len("REQ:GET:"):]
                    print(f"[Serial TCP 12345] Request path: {req_path}")
                    content = get_file_content(req_path)
                    if content is not None:
                        header = f"RES:OK:{len(content)}\n".encode("utf-8")
                        conn.sendall(header + content)
                        print(f"[Serial TCP 12345] Sent RES:OK ({len(content)} bytes)")
                    else:
                        conn.sendall(b"RES:ERROR:File Not Found\n")
                        print(f"[Serial TCP 12345] Sent RES:ERROR for {req_path}")
    except Exception as e:
        print(f"[Serial TCP 12345] Connection error with {addr}: {e}")
    finally:
        conn.close()

def run_serial_server():
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    s.bind(("0.0.0.0", SERIAL_PORT))
    s.listen(5)
    print(f"========================================================")
    print(f" APT Serial TCP Proxy listening on 0.0.0.0:{SERIAL_PORT}")
    print(f"========================================================")
    while True:
        conn, addr = s.accept()
        t = threading.Thread(target=handle_serial_client, args=(conn, addr), daemon=True)
        t.start()

def main():
    ensure_cache()
    
    http_thread = threading.Thread(target=run_http_server, daemon=True)
    serial_thread = threading.Thread(target=run_serial_server, daemon=True)

    http_thread.start()
    serial_thread.start()

    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("\nShutting down APT server...")
        sys.exit(0)

if __name__ == "__main__":
    main()
