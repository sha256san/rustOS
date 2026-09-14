extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use crate::println;
use crate::print;
use spin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum User {
    Root,
    Guru,
    Guest,
}

impl User {
    pub fn name(&self) -> &'static str {
        match self {
            User::Root => "root",
            User::Guru => "guru",
            User::Guest => "guest",
        }
    }

    pub fn prompt_char(&self) -> char {
        match self {
            User::Root => '#',
            _ => '$',
        }
    }
}

pub static CURRENT_USERNAME: spin::Mutex<String> = spin::Mutex::new(String::new());
pub static USER_PASSWORD: spin::Mutex<String> = spin::Mutex::new(String::new());
static SETUP_TEMP_USER: spin::Mutex<String> = spin::Mutex::new(String::new());
static SETUP_TEMP_PASS: spin::Mutex<String> = spin::Mutex::new(String::new());

#[derive(Clone)]
pub enum ShellState {
    SetupUsername,
    SetupPassword,
    SetupConfirmPassword,
    Normal,
    SuPasswordPrompt { target_user: User },
    SudoPasswordPrompt { command_line: [u8; 128], length: usize },
}

pub static CURRENT_USER: spin::Mutex<User> = spin::Mutex::new(User::Guru);
pub static SHELL_STATE: spin::Mutex<ShellState> = spin::Mutex::new(ShellState::Normal);

pub static ENV_VARS: spin::Mutex<Option<BTreeMap<String, String>>> = spin::Mutex::new(None);
pub static PIPELINE_INPUT: spin::Mutex<Option<String>> = spin::Mutex::new(None);

const COMMAND_BUFFER_SIZE: usize = 128;
static mut COMMAND_BUFFER: [u8; COMMAND_BUFFER_SIZE] = [0; COMMAND_BUFFER_SIZE];
static mut COMMAND_LENGTH: usize = 0;

// Command History
const HISTORY_SIZE: usize = 10;
static mut HISTORY: [[u8; COMMAND_BUFFER_SIZE]; HISTORY_SIZE] = [[0; COMMAND_BUFFER_SIZE]; HISTORY_SIZE];
static mut HISTORY_COUNT: usize = 0;
static mut HISTORY_INDEX: isize = -1;

pub fn init_env() {
    let mut vars = BTreeMap::new();
    vars.insert(String::from("PATH"), String::from("/bin:/usr/bin:/sbin:/usr/sbin"));
    vars.insert(String::from("PWD"), String::from("/home/guru"));
    vars.insert(String::from("USER"), String::from("guru"));
    vars.insert(String::from("HOME"), String::from("/home/guru"));
    *ENV_VARS.lock() = Some(vars);
}

pub fn get_env(key: &str) -> String {
    let lock = ENV_VARS.lock();
    if let Some(ref map) = *lock {
        if let Some(val) = map.get(key) {
            return val.clone();
        }
    }
    String::new()
}

pub fn set_env(key: &str, val: &str) {
    let mut lock = ENV_VARS.lock();
    if let Some(ref mut map) = *lock {
        map.insert(String::from(key), String::from(val));
    }
}

pub fn print_prompt() {
    let user = CURRENT_USER.lock();
    let current_name = CURRENT_USERNAME.lock();
    let display_name = if user.name() == "root" {
        "root"
    } else if !current_name.is_empty() {
        current_name.as_str()
    } else {
        "guru"
    };
    let pwd = get_env("PWD");
    let home = get_env("HOME");
    let display_path = if !home.is_empty() && pwd == home {
        "~"
    } else if pwd.is_empty() {
        "~"
    } else {
        &pwd
    };
    let prompt_char = if user.name() == "root" { '#' } else { '$' };
    print!("{}@ubuntu-2604-server:{}{} ", display_name, display_path, prompt_char);
}

pub fn run() -> ! {
    println!("=========================================================");
    println!(" Welcome to Ubuntu 26.04 LTS Server (Resolute Raccoon)!");
    println!("=========================================================");
    println!("First-time Setup: Please create a new user account.");
    println!();
    print!("New username: ");

    *SHELL_STATE.lock() = ShellState::SetupUsername;

    loop {
        x86_64::instructions::hlt();
        if let Some(c) = crate::input::read_char() {
            handle_char(c);
        }
    }
}

fn add_to_history(cmd: &[u8]) {
    unsafe {
        if cmd.is_empty() {
            return;
        }
        if HISTORY_COUNT > 0 {
            let last_index = (HISTORY_COUNT - 1) % HISTORY_SIZE;
            let last_cmd = &HISTORY[last_index];
            let mut match_found = true;
            for i in 0..cmd.len() {
                if last_cmd[i] != cmd[i] {
                    match_found = false;
                    break;
                }
            }
            if match_found && last_cmd[cmd.len()] == 0 {
                return;
            }
        }
        let index = HISTORY_COUNT % HISTORY_SIZE;
        for i in 0..cmd.len() {
            HISTORY[index][i] = cmd[i];
        }
        if cmd.len() < COMMAND_BUFFER_SIZE {
            HISTORY[index][cmd.len()] = 0;
        }
        HISTORY_COUNT += 1;
    }
}

fn clear_current_line() {
    unsafe {
        while COMMAND_LENGTH > 0 {
            COMMAND_LENGTH -= 1;
            crate::vga_buffer::backspace();
        }
    }
}

fn load_history(index: usize) {
    clear_current_line();
    unsafe {
        let cmd = &HISTORY[index];
        let mut len = 0;
        while len < COMMAND_BUFFER_SIZE && cmd[len] != 0 {
            COMMAND_BUFFER[len] = cmd[len];
            len += 1;
        }
        COMMAND_LENGTH = len;
        for i in 0..len {
            print!("{}", COMMAND_BUFFER[i] as char);
        }
    }
}

fn expand_env_vars(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            let mut var_name = String::new();
            while let Some(&next_c) = chars.peek() {
                if next_c.is_alphanumeric() || next_c == '_' {
                    var_name.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            if !var_name.is_empty() {
                result.push_str(&get_env(&var_name));
            } else {
                result.push('$');
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn parse_redirection(input: &str) -> (String, Option<String>, bool) {
    if let Some(pos) = input.find(">>") {
        let cmd = input[..pos].trim().to_string();
        let file = input[pos+2..].trim().to_string();
        (cmd, Some(file), true)
    } else if let Some(pos) = input.find('>') {
        let cmd = input[..pos].trim().to_string();
        let file = input[pos+1..].trim().to_string();
        (cmd, Some(file), false)
    } else {
        (input.trim().to_string(), None, false)
    }
}

fn handle_char(c: u8) {
    let state = SHELL_STATE.lock().clone();
    match state {
        ShellState::SetupUsername => {
            if c == b'\n' || c == b'\r' {
                println!();
                unsafe {
                    let entered = match core::str::from_utf8(&COMMAND_BUFFER[..COMMAND_LENGTH]) {
                        Ok(s) => s.trim().to_string(),
                        Err(_) => String::new(),
                    };
                    COMMAND_LENGTH = 0;
                    if !entered.is_empty() {
                        *SETUP_TEMP_USER.lock() = entered;
                        print!("New password: ");
                        *SHELL_STATE.lock() = ShellState::SetupPassword;
                    } else {
                        print!("New username: ");
                    }
                }
            } else if c == 8 || c == 127 {
                unsafe {
                    if COMMAND_LENGTH > 0 {
                        COMMAND_LENGTH -= 1;
                        crate::vga_buffer::backspace();
                    }
                }
            } else {
                unsafe {
                    if COMMAND_LENGTH < COMMAND_BUFFER_SIZE {
                        COMMAND_BUFFER[COMMAND_LENGTH] = c;
                        COMMAND_LENGTH += 1;
                        print!("{}", c as char);
                    }
                }
            }
        }
        ShellState::SetupPassword => {
            if c == b'\n' || c == b'\r' {
                println!();
                unsafe {
                    let entered = match core::str::from_utf8(&COMMAND_BUFFER[..COMMAND_LENGTH]) {
                        Ok(s) => s.to_string(),
                        Err(_) => String::new(),
                    };
                    COMMAND_LENGTH = 0;
                    *SETUP_TEMP_PASS.lock() = entered;
                    print!("Confirm password: ");
                    *SHELL_STATE.lock() = ShellState::SetupConfirmPassword;
                }
            } else if c == 8 || c == 127 {
                unsafe {
                    if COMMAND_LENGTH > 0 {
                        COMMAND_LENGTH -= 1;
                    }
                }
            } else {
                unsafe {
                    if COMMAND_LENGTH < COMMAND_BUFFER_SIZE {
                        COMMAND_BUFFER[COMMAND_LENGTH] = c;
                        COMMAND_LENGTH += 1;
                        print!("*");
                    }
                }
            }
        }
        ShellState::SetupConfirmPassword => {
            if c == b'\n' || c == b'\r' {
                println!();
                unsafe {
                    let confirmed = match core::str::from_utf8(&COMMAND_BUFFER[..COMMAND_LENGTH]) {
                        Ok(s) => s.to_string(),
                        Err(_) => String::new(),
                    };
                    COMMAND_LENGTH = 0;
                    let pass = SETUP_TEMP_PASS.lock().clone();
                    if confirmed == pass && !pass.is_empty() {
                        let user_name = SETUP_TEMP_USER.lock().clone();
                        *CURRENT_USERNAME.lock() = user_name.clone();
                        *USER_PASSWORD.lock() = pass;

                        let home_dir = alloc::format!("/home/{}", user_name);
                        set_env("USER", &user_name);
                        set_env("HOME", &home_dir);
                        set_env("PWD", &home_dir);

                        let _ = crate::vfs::vfs_mkdir(&home_dir, &user_name, &user_name, "drwxr-xr-x");

                        println!("Creating user '{}'...", user_name);
                        println!("Adding user '{}' to group 'sudo'...", user_name);
                        println!("Initial setup completed successfully!");
                        println!();

                        *SHELL_STATE.lock() = ShellState::Normal;
                        print_prompt();
                    } else {
                        println!("Passwords do not match. Please try again.");
                        print!("New password: ");
                        *SHELL_STATE.lock() = ShellState::SetupPassword;
                    }
                }
            } else if c == 8 || c == 127 {
                unsafe {
                    if COMMAND_LENGTH > 0 {
                        COMMAND_LENGTH -= 1;
                    }
                }
            } else {
                unsafe {
                    if COMMAND_LENGTH < COMMAND_BUFFER_SIZE {
                        COMMAND_BUFFER[COMMAND_LENGTH] = c;
                        COMMAND_LENGTH += 1;
                        print!("*");
                    }
                }
            }
        }
        ShellState::Normal => {
            if c == b'\n' || c == b'\r' {
                println!();
                unsafe {
                    if COMMAND_LENGTH > 0 {
                        add_to_history(&COMMAND_BUFFER[..COMMAND_LENGTH]);
                        execute_command(&COMMAND_BUFFER[..COMMAND_LENGTH]);
                        COMMAND_LENGTH = 0;
                    }
                    HISTORY_INDEX = -1;
                }
                print_prompt();
            } else if c == 8 || c == 127 {
                unsafe {
                    if COMMAND_LENGTH > 0 {
                        COMMAND_LENGTH -= 1;
                        crate::vga_buffer::backspace();
                    }
                }
            } else if c == b'\t' {
                handle_tab_completion();
            } else if c == 0x10 { // Up Arrow
                unsafe {
                    if HISTORY_COUNT > 0 {
                        let limit = if HISTORY_COUNT > HISTORY_SIZE { HISTORY_SIZE } else { HISTORY_COUNT };
                        if HISTORY_INDEX == -1 {
                            HISTORY_INDEX = (limit - 1) as isize;
                        } else if HISTORY_INDEX > 0 {
                            HISTORY_INDEX -= 1;
                        }
                        let real_index = if HISTORY_COUNT > HISTORY_SIZE {
                            (HISTORY_COUNT - limit + HISTORY_INDEX as usize) % HISTORY_SIZE
                        } else {
                            HISTORY_INDEX as usize
                        };
                        load_history(real_index);
                    }
                }
            } else if c == 0x1e { // Down Arrow
                unsafe {
                    if HISTORY_INDEX != -1 {
                        let limit = if HISTORY_COUNT > HISTORY_SIZE { HISTORY_SIZE } else { HISTORY_COUNT };
                        if HISTORY_INDEX < (limit - 1) as isize {
                            HISTORY_INDEX += 1;
                            let real_index = if HISTORY_COUNT > HISTORY_SIZE {
                                (HISTORY_COUNT - limit + HISTORY_INDEX as usize) % HISTORY_SIZE
                            } else {
                                HISTORY_INDEX as usize
                            };
                            load_history(real_index);
                        } else {
                            HISTORY_INDEX = -1;
                            clear_current_line();
                        }
                    }
                }
            } else {
                unsafe {
                    if COMMAND_LENGTH < COMMAND_BUFFER_SIZE {
                        COMMAND_BUFFER[COMMAND_LENGTH] = c;
                        COMMAND_LENGTH += 1;
                        print!("{}", c as char);
                    }
                }
            }
        }
        ShellState::SuPasswordPrompt { target_user } => {
            if c == b'\n' || c == b'\r' {
                println!();
                unsafe {
                    let entered_pw = &COMMAND_BUFFER[..COMMAND_LENGTH];
                    if check_password(target_user, entered_pw) {
                        let mut user = CURRENT_USER.lock();
                        *user = target_user;
                        set_env("USER", target_user.name());
                        let home_dir = if target_user == User::Root { "/root" } else { "/home/guru" };
                        set_env("HOME", home_dir);
                        set_env("PWD", home_dir);
                    } else {
                        println!("su: Authentication failure");
                    }
                    COMMAND_LENGTH = 0;
                }
                *SHELL_STATE.lock() = ShellState::Normal;
                print_prompt();
            } else if c == 8 || c == 127 {
                unsafe {
                    if COMMAND_LENGTH > 0 {
                        COMMAND_LENGTH -= 1;
                    }
                }
            } else {
                unsafe {
                    if COMMAND_LENGTH < COMMAND_BUFFER_SIZE {
                        COMMAND_BUFFER[COMMAND_LENGTH] = c;
                        COMMAND_LENGTH += 1;
                        print!("*");
                    }
                }
            }
        }
        ShellState::SudoPasswordPrompt { command_line, length } => {
            if c == b'\n' || c == b'\r' {
                println!();
                unsafe {
                    let entered_pw = &COMMAND_BUFFER[..COMMAND_LENGTH];
                    let current = *CURRENT_USER.lock();
                    if check_password(current, entered_pw) {
                        let old_user = current;
                        *CURRENT_USER.lock() = User::Root;
                        execute_command(&command_line[..length]);
                        *CURRENT_USER.lock() = old_user;
                    } else {
                        println!("sudo: 1 incorrect password attempt");
                    }
                    COMMAND_LENGTH = 0;
                }
                *SHELL_STATE.lock() = ShellState::Normal;
                print_prompt();
            } else if c == 8 || c == 127 {
                unsafe {
                    if COMMAND_LENGTH > 0 {
                        COMMAND_LENGTH -= 1;
                    }
                }
            } else {
                unsafe {
                    if COMMAND_LENGTH < COMMAND_BUFFER_SIZE {
                        COMMAND_BUFFER[COMMAND_LENGTH] = c;
                        COMMAND_LENGTH += 1;
                        print!("*");
                    }
                }
            }
        }
    }
}

fn handle_tab_completion() {
    unsafe {
        let cmd_str = match core::str::from_utf8(&COMMAND_BUFFER[..COMMAND_LENGTH]) {
            Ok(s) => s,
            Err(_) => return,
        };
        
        let last_space = cmd_str.rfind(' ');
        if let Some(pos) = last_space {
            let path_part = &cmd_str[pos + 1..];
            let cwd = get_env("PWD");
            
            let last_slash = path_part.rfind('/');
            let (lookup_dir, prefix) = if let Some(slash_pos) = last_slash {
                let dir_str = &path_part[..slash_pos];
                let prefix_str = &path_part[slash_pos + 1..];
                let d = if dir_str.is_empty() { "/" } else { dir_str };
                (crate::vfs::normalize_path(&cwd, d), prefix_str)
            } else {
                (cwd.clone(), path_part)
            };

            if let Ok(entries) = crate::vfs::vfs_readdir(&lookup_dir) {
                let mut matches = Vec::new();
                for entry in entries {
                    if entry.name.starts_with(prefix) {
                        matches.push(entry);
                    }
                }
                
                if matches.len() == 1 {
                    let matched_name = &matches[0].name;
                    let remaining = &matched_name[prefix.len()..];
                    for &b in remaining.as_bytes() {
                        if COMMAND_LENGTH < COMMAND_BUFFER_SIZE {
                            COMMAND_BUFFER[COMMAND_LENGTH] = b;
                            COMMAND_LENGTH += 1;
                            print!("{}", b as char);
                        }
                    }
                    if matches[0].is_dir {
                        if COMMAND_LENGTH < COMMAND_BUFFER_SIZE {
                            COMMAND_BUFFER[COMMAND_LENGTH] = b'/';
                            COMMAND_LENGTH += 1;
                            print!("/");
                        }
                    }
                } else if matches.len() > 1 {
                    println!();
                    for m in &matches {
                        if m.is_dir {
                            print!("{}/  ", m.name);
                        } else {
                            print!("{}  ", m.name);
                        }
                    }
                    println!();
                    print_prompt();
                    for i in 0..COMMAND_LENGTH {
                        print!("{}", COMMAND_BUFFER[i] as char);
                    }
                }
            }
        } else {
            let prefix = cmd_str;
            let commands = [
                "help", "clear", "echo", "uname", "whoami", "pwd", "cd", "ls", "cat",
                "touch", "mkdir", "rm", "grep", "head", "tail", "wc", "tree", "cp",
                "mv", "chmod", "chown", "ss", "curl", "wget", "htop", "useradd", "passwd",
                "export", "env", "su", "sudo", "systemctl", "journalctl", "ufw", "crontab",
                "lspci", "lsusb", "ip", "ping", "reboot", "panic"
            ];
            
            let mut matches = Vec::new();
            for &cmd in &commands {
                if cmd.starts_with(prefix) {
                    matches.push(cmd);
                }
            }
            
            if matches.len() == 1 {
                let matched_name = matches[0];
                let remaining = &matched_name[prefix.len()..];
                for &b in remaining.as_bytes() {
                    if COMMAND_LENGTH < COMMAND_BUFFER_SIZE {
                        COMMAND_BUFFER[COMMAND_LENGTH] = b;
                        COMMAND_LENGTH += 1;
                        print!("{}", b as char);
                    }
                }
                if COMMAND_LENGTH < COMMAND_BUFFER_SIZE {
                    COMMAND_BUFFER[COMMAND_LENGTH] = b' ';
                    COMMAND_LENGTH += 1;
                    print!(" ");
                }
            } else if matches.len() > 1 {
                println!();
                for m in &matches {
                    print!("{}  ", m);
                }
                println!();
                print_prompt();
                for i in 0..COMMAND_LENGTH {
                    print!("{}", COMMAND_BUFFER[i] as char);
                }
            }
        }
    }
}

fn check_password(user: User, password: &[u8]) -> bool {
    let set_pw = USER_PASSWORD.lock();
    if !set_pw.is_empty() && user != User::Root {
        return password == set_pw.as_bytes();
    }
    match user {
        User::Root => password == b"root",
        User::Guru => password == b"guru",
        User::Guest => true,
    }
}

fn execute_command(command: &[u8]) {
    let command_str = match core::str::from_utf8(command) {
        Ok(s) => s.trim(),
        Err(_) => return,
    };

    if command_str.is_empty() {
        return;
    }

    // 1. Expand environment variables
    let expanded = expand_env_vars(command_str);

    // 2. Parse redirection
    let (clean_cmd, redirect_file, append_mode) = parse_redirection(&expanded);
    if clean_cmd.is_empty() {
        return;
    }

    // 3. Set up redirection target if present
    if redirect_file.is_some() {
        *crate::vga_buffer::REDIRECT_TARGET.lock() = Some(String::new());
    }

    // 4. Split by Pipeline '|'
    let cmd_parts: Vec<&str> = clean_cmd.split('|').collect();
    if cmd_parts.len() > 1 {
        let mut input_buffer = String::new();
        for (i, part) in cmd_parts.iter().enumerate() {
            let part = part.trim();
            *crate::vga_buffer::REDIRECT_TARGET.lock() = Some(String::new());
            
            if i > 0 {
                *PIPELINE_INPUT.lock() = Some(input_buffer.clone());
            } else {
                *PIPELINE_INPUT.lock() = None;
            }

            execute_command_internal(part);

            let output = {
                let mut target = crate::vga_buffer::REDIRECT_TARGET.lock();
                target.take()
            };
            input_buffer = output.unwrap_or_default();
        }
        *PIPELINE_INPUT.lock() = None;
        
        // Print or write final output
        if redirect_file.is_none() {
            print!("{}", input_buffer);
        } else {
            *crate::vga_buffer::REDIRECT_TARGET.lock() = Some(input_buffer);
        }
    } else {
        execute_command_internal(&clean_cmd);
    }

    // 5. Write redirected output to VFS if active
    if let Some(ref file_path) = redirect_file {
        let output = {
            let mut target = crate::vga_buffer::REDIRECT_TARGET.lock();
            target.take()
        };
        if let Some(content) = output {
            let normalized = crate::vfs::normalize_path(&get_env("PWD"), file_path);
            let user = CURRENT_USER.lock().name();
            let mut final_data = Vec::new();
            if append_mode {
                if let Ok(existing) = crate::vfs::vfs_read_file(&normalized) {
                    final_data.extend_from_slice(&existing);
                }
            }
            final_data.extend_from_slice(content.as_bytes());

            if let Err(e) = crate::vfs::vfs_write_file(&normalized, &final_data, user, user, "-rw-r--r--") {
                println!("sh: redirection error: {}", e);
            }
        }
    }
}

fn execute_command_internal(command_str: &str) {
    let mut parts = command_str.split_whitespace();
    let cmd = parts.next().unwrap_or("");
    let args = parts;

    match cmd {
        "help" => {
            println!("Ubuntu 26.04 LTS Server (Resolute Raccoon) - Available Commands:");
            println!("  [System & Security]");
            println!("    sudo [cmd...]      - Execute command as root (sudo-rs)");
            println!("    systemctl [cmd]    - Control systemd services (status/start/stop)");
            println!("    journalctl [-u]    - View system logs (-u <svc> -n <N>)");
            println!("    apt [cmd]          - Package management (update/install/upgrade)");
            println!("    ufw [cmd]          - Uncomplicated Firewall (status/allow/enable)");
            println!("    useradd / passwd   - Manage system users and passwords");
            println!("  [File Operations & Navigation]");
            println!("    ls [-la] / cd / pwd- List, change, and print directory");
            println!("    cat / touch / mkdir- Read, create, and make directories");
            println!("    cp / mv / rm       - Copy, move, and remove files");
            println!("    chmod / chown      - Modify file permissions and ownership");
            println!("    tree               - Recursive directory tree view");
            println!("  [Text Processing]");
            println!("    head [-n N]        - Display first N lines of file/stdin");
            println!("    tail [-n N] [-f]   - Display last N lines of file/stdin");
            println!("    grep [pattern]     - Search pattern in file/stdin");
            println!("    wc [-l]            - Count lines/words in file/stdin");
            println!("  [Networking & Monitoring]");
            println!("    ip [addr/link]     - Display network addresses and interfaces");
            println!("    ss                 - Display socket and port status");
            println!("    ping [host]        - Test network connectivity");
            println!("    curl / wget        - Download or request URL resources");
            println!("    htop               - Interactive process viewer");
            println!("  [System Info]");
            println!("    uname [-a]         - Print kernel and system details");
            println!("    whoami / env / export- User identity and environment variables");
        }
        "clear" => {
            crate::vga_buffer::clear_screen();
        }
        "echo" => {
            let mut args_iter = args;
            let mut no_newline = false;
            let mut peek = args_iter.clone();
            if let Some("-n") = peek.next() {
                no_newline = true;
                args_iter.next();
            }
            let mut first = true;
            for arg in args_iter {
                if !first {
                    print!(" ");
                }
                print!("{}", arg);
                first = false;
            }
            if !no_newline {
                println!();
            }
        }
        "uname" => {
            let mut show_all = false;
            for arg in args {
                match arg {
                    "-a" | "--all" => show_all = true,
                    _ => {
                        println!("uname: invalid option -- '{}'", arg);
                        return;
                    }
                }
            }
            if show_all {
                println!("Linux ubuntu-2604-server 6.8.0-26-generic #26-Ubuntu SMP PREEMPT_DYNAMIC Mon Apr 20 00:00:00 UTC 2026 x86_64 x86_64 x86_64 GNU/Linux");
            } else {
                println!("Linux");
            }
        }
        "whoami" => {
            println!("{}", get_env("USER"));
        }
        "pwd" => {
            println!("{}", get_env("PWD"));
        }
        "cd" => {
            let mut args_iter = args;
            let dest = args_iter.next().unwrap_or("~");
            let target_path = if dest == "~" {
                get_env("HOME")
            } else {
                dest.to_string()
            };
            
            let normalized = crate::vfs::normalize_path(&get_env("PWD"), &target_path);
            let dir_res = crate::vfs::vfs_readdir(&normalized);
            if dir_res.is_ok() {
                set_env("PWD", &normalized);
            } else {
                println!("cd: {}: No such file or directory", dest);
            }
        }
        "ls" => {
            let mut list_all = false;
            let mut long_format = false;
            for arg in args {
                if arg.starts_with('-') {
                    for c in arg.chars().skip(1) {
                        match c {
                            'a' => list_all = true,
                            'l' => long_format = true,
                            _ => {
                                println!("ls: invalid option -- '{}'", c);
                                return;
                            }
                        }
                    }
                } else {
                    println!("ls: directory arguments are not supported in this version");
                    return;
                }
            }
            let cwd = get_env("PWD");
            if let Ok(entries) = crate::vfs::vfs_readdir(&cwd) {
                if list_all {
                    // Show . and ..
                    if long_format {
                        println!("drwxr-xr-x  1 root root  4096 Jul 20 00:00 .");
                        println!("drwxr-xr-x  1 root root  4096 Jul 20 00:00 ..");
                    } else {
                        print!(".   ..   ");
                    }
                }
                for file in &entries {
                    if file.name.starts_with('.') && !list_all {
                        continue;
                    }
                    if long_format {
                        println!("{}  1 {} {} {:>5} {} {}", file.perms, file.owner, file.group, file.data.len(), file.date, file.name);
                    } else {
                        if file.is_dir {
                            print!("{}/   ", file.name);
                        } else {
                            print!("{}   ", file.name);
                        }
                    }
                }
                if !long_format {
                    println!();
                }
            } else {
                println!("ls: cannot access '{}': No such directory", cwd);
            }
        }
        "cat" => {
            let mut args_iter = args;
            let file = args_iter.next().unwrap_or("");
            if file.is_empty() {
                println!("usage: cat [filename]");
                return;
            }
            let normalized = crate::vfs::normalize_path(&get_env("PWD"), file);
            match crate::vfs::vfs_read_file(&normalized) {
                Ok(bytes) => {
                    let s = match core::str::from_utf8(&bytes) {
                        Ok(text) => text,
                        Err(_) => "[binary data]",
                    };
                    print!("{}", s);
                    if !s.ends_with('\n') {
                        println!();
                    }
                }
                Err(e) => {
                    println!("cat: {}: {}", file, e);
                }
            }
        }
        "touch" => {
            let mut args_iter = args;
            let file = args_iter.next().unwrap_or("");
            if file.is_empty() {
                println!("usage: touch [filename]");
                return;
            }
            let normalized = crate::vfs::normalize_path(&get_env("PWD"), file);
            let user = CURRENT_USER.lock().name();
            if let Err(e) = crate::vfs::vfs_write_file(&normalized, &[], user, user, "-rw-r--r--") {
                println!("touch: {}", e);
            }
        }
        "mkdir" => {
            let mut args_iter = args;
            let dir = args_iter.next().unwrap_or("");
            if dir.is_empty() {
                println!("usage: mkdir [dirname]");
                return;
            }
            let normalized = crate::vfs::normalize_path(&get_env("PWD"), dir);
            let user = CURRENT_USER.lock().name();
            if let Err(e) = crate::vfs::vfs_mkdir(&normalized, user, user, "drwxr-xr-x") {
                println!("mkdir: {}", e);
            }
        }
        "rm" => {
            let mut args_iter = args;
            let path = args_iter.next().unwrap_or("");
            if path.is_empty() {
                println!("usage: rm [path]");
                return;
            }
            let normalized = crate::vfs::normalize_path(&get_env("PWD"), path);
            if let Err(e) = crate::vfs::vfs_remove(&normalized) {
                println!("rm: {}", e);
            }
        }
        "grep" => {
            let mut args_iter = args;
            let pattern = args_iter.next().unwrap_or("");
            if pattern.is_empty() {
                println!("usage: grep [pattern] [file]");
                return;
            }
            let file = args_iter.next().unwrap_or("");
            let content = if !file.is_empty() {
                let normalized = crate::vfs::normalize_path(&get_env("PWD"), file);
                match crate::vfs::vfs_read_file(&normalized) {
                    Ok(bytes) => String::from_utf8(bytes).unwrap_or_default(),
                    Err(e) => {
                        println!("grep: {}: {}", file, e);
                        return;
                    }
                }
            } else {
                PIPELINE_INPUT.lock().clone().unwrap_or_default()
            };

            for line in content.lines() {
                if line.contains(pattern) {
                    println!("{}", line);
                }
            }
        }
        "export" => {
            let mut args_iter = args;
            let arg = args_iter.next().unwrap_or("");
            if arg.is_empty() {
                // List variables
                let lock = ENV_VARS.lock();
                if let Some(ref map) = *lock {
                    for (k, v) in map {
                        println!("declare -x {}=\"{}\"", k, v);
                    }
                }
                return;
            }
            if let Some(pos) = arg.find('=') {
                let key = &arg[..pos];
                let val = &arg[pos+1..];
                set_env(key, val);
            } else {
                set_env(arg, "");
            }
        }
        "env" => {
            let lock = ENV_VARS.lock();
            if let Some(ref map) = *lock {
                for (k, v) in map {
                    println!("{}={}", k, v);
                }
            }
        }
        "su" => {
            let mut args_iter = args;
            let target_username = args_iter.next().unwrap_or("root");
            let target_user = match target_username {
                "root" => User::Root,
                "guru" => User::Guru,
                "guest" => User::Guest,
                _ => {
                    println!("su: user '{}' does not exist", target_username);
                    return;
                }
            };
            if target_user == *CURRENT_USER.lock() {
                return;
            }
            if target_user == User::Guest {
                let mut user = CURRENT_USER.lock();
                *user = User::Guest;
                set_env("USER", "guest");
                set_env("HOME", "/home/guest");
                set_env("PWD", "/home/guest");
            } else {
                print!("Password: ");
                *SHELL_STATE.lock() = ShellState::SuPasswordPrompt { target_user };
            }
        }
        "sudo" => {
            let args_iter = args;
            let mut sub_cmd_buf = [0; 128];
            let mut len = 0;
            for arg in args_iter {
                let bytes = arg.as_bytes();
                if len + bytes.len() + 1 < 128 {
                    sub_cmd_buf[len..len+bytes.len()].copy_from_slice(bytes);
                    len += bytes.len();
                    sub_cmd_buf[len] = b' ';
                    len += 1;
                }
            }
            if len > 0 {
                len -= 1;
                let current = *CURRENT_USER.lock();
                if current == User::Root {
                    execute_command(&sub_cmd_buf[..len]);
                } else {
                    print!("[sudo-rs] password for {}: ", current.name());
                    *SHELL_STATE.lock() = ShellState::SudoPasswordPrompt { command_line: sub_cmd_buf, length: len };
                }
            } else {
                println!("usage: sudo [command]");
            }
        }
        "head" => {
            let mut lines_count = 10;
            let mut file_path = "";
            let mut args_iter = args;
            while let Some(arg) = args_iter.next() {
                if arg == "-n" {
                    if let Some(val) = args_iter.next() {
                        lines_count = val.parse::<usize>().unwrap_or(10);
                    }
                } else if arg.starts_with('-') && arg.len() > 1 && arg[1..].parse::<usize>().is_ok() {
                    lines_count = arg[1..].parse::<usize>().unwrap_or(10);
                } else {
                    file_path = arg;
                }
            }
            let content = if !file_path.is_empty() {
                let normalized = crate::vfs::normalize_path(&get_env("PWD"), file_path);
                match crate::vfs::vfs_read_file(&normalized) {
                    Ok(b) => String::from_utf8(b).unwrap_or_default(),
                    Err(e) => {
                        println!("head: cannot open '{}': {}", file_path, e);
                        return;
                    }
                }
            } else {
                PIPELINE_INPUT.lock().clone().unwrap_or_default()
            };
            for (i, line) in content.lines().enumerate() {
                if i >= lines_count { break; }
                println!("{}", line);
            }
        }
        "tail" => {
            let mut lines_count = 10;
            let mut file_path = "";
            let mut follow = false;
            let mut args_iter = args;
            while let Some(arg) = args_iter.next() {
                if arg == "-n" {
                    if let Some(val) = args_iter.next() {
                        lines_count = val.parse::<usize>().unwrap_or(10);
                    }
                } else if arg == "-f" || arg == "--follow" {
                    follow = true;
                } else if arg.starts_with('-') && arg.len() > 1 && arg[1..].parse::<usize>().is_ok() {
                    lines_count = arg[1..].parse::<usize>().unwrap_or(10);
                } else {
                    file_path = arg;
                }
            }
            let content = if !file_path.is_empty() {
                let normalized = crate::vfs::normalize_path(&get_env("PWD"), file_path);
                match crate::vfs::vfs_read_file(&normalized) {
                    Ok(b) => String::from_utf8(b).unwrap_or_default(),
                    Err(e) => {
                        println!("tail: cannot open '{}': {}", file_path, e);
                        return;
                    }
                }
            } else {
                PIPELINE_INPUT.lock().clone().unwrap_or_default()
            };
            let all_lines: Vec<&str> = content.lines().collect();
            let start = if all_lines.len() > lines_count { all_lines.len() - lines_count } else { 0 };
            for line in &all_lines[start..] {
                println!("{}", line);
            }
            if follow {
                println!("[tail: following log stream - uutils/coreutils mode]");
            }
        }
        "wc" => {
            let mut line_only = false;
            let mut file_path = "";
            let mut args_iter = args;
            while let Some(arg) = args_iter.next() {
                if arg == "-l" || arg == "--lines" {
                    line_only = true;
                } else {
                    file_path = arg;
                }
            }
            let content = if !file_path.is_empty() {
                let normalized = crate::vfs::normalize_path(&get_env("PWD"), file_path);
                match crate::vfs::vfs_read_file(&normalized) {
                    Ok(b) => String::from_utf8(b).unwrap_or_default(),
                    Err(e) => {
                        println!("wc: cannot open '{}': {}", file_path, e);
                        return;
                    }
                }
            } else {
                PIPELINE_INPUT.lock().clone().unwrap_or_default()
            };
            let lines = content.lines().count();
            let words = content.split_whitespace().count();
            let bytes = content.as_bytes().len();
            if line_only {
                println!("{:>7} {}", lines, file_path);
            } else {
                println!("{:>7} {:>7} {:>7} {}", lines, words, bytes, file_path);
            }
        }
        "tree" => {
            let cwd = get_env("PWD");
            println!("{}", cwd);
            fn print_tree_dir(path: &str, indent: &str) {
                if let Ok(entries) = crate::vfs::vfs_readdir(path) {
                    let total = entries.len();
                    for (idx, entry) in entries.iter().enumerate() {
                        let is_last = idx == total - 1;
                        let branch = if is_last { "└── " } else { "├── " };
                        let child_indent = if is_last { "    " } else { "│   " };
                        if entry.is_dir {
                            println!("{}{}{}/", indent, branch, entry.name);
                            let subpath = if path == "/" {
                                alloc::format!("/{}", entry.name)
                            } else {
                                alloc::format!("{}/{}", path, entry.name)
                            };
                            let next_indent = alloc::format!("{}{}", indent, child_indent);
                            print_tree_dir(&subpath, &next_indent);
                        } else {
                            println!("{}{}{}", indent, branch, entry.name);
                        }
                    }
                }
            }
            print_tree_dir(&cwd, "");
        }
        "cp" => {
            let mut args_iter = args;
            let src = args_iter.next().unwrap_or("");
            let dest = args_iter.next().unwrap_or("");
            if src.is_empty() || dest.is_empty() {
                println!("usage: cp <source> <destination>");
                return;
            }
            let src_norm = crate::vfs::normalize_path(&get_env("PWD"), src);
            let dest_norm = crate::vfs::normalize_path(&get_env("PWD"), dest);
            match crate::vfs::vfs_read_file(&src_norm) {
                Ok(data) => {
                    let user = CURRENT_USER.lock().name();
                    if let Err(e) = crate::vfs::vfs_write_file(&dest_norm, &data, user, user, "-rw-r--r--") {
                        println!("cp: cannot create '{}': {}", dest, e);
                    }
                }
                Err(e) => println!("cp: cannot stat '{}': {}", src, e),
            }
        }
        "mv" => {
            let mut args_iter = args;
            let src = args_iter.next().unwrap_or("");
            let dest = args_iter.next().unwrap_or("");
            if src.is_empty() || dest.is_empty() {
                println!("usage: mv <source> <destination>");
                return;
            }
            let src_norm = crate::vfs::normalize_path(&get_env("PWD"), src);
            let dest_norm = crate::vfs::normalize_path(&get_env("PWD"), dest);
            match crate::vfs::vfs_read_file(&src_norm) {
                Ok(data) => {
                    let user = CURRENT_USER.lock().name();
                    if let Ok(_) = crate::vfs::vfs_write_file(&dest_norm, &data, user, user, "-rw-r--r--") {
                        let _ = crate::vfs::vfs_remove(&src_norm);
                    } else {
                        println!("mv: cannot move '{}'", src);
                    }
                }
                Err(e) => println!("mv: cannot stat '{}': {}", src, e),
            }
        }
        "chmod" => {
            let mut args_iter = args;
            let mode = args_iter.next().unwrap_or("");
            let path = args_iter.next().unwrap_or("");
            if mode.is_empty() || path.is_empty() {
                println!("usage: chmod <mode> <path>");
                return;
            }
            println!("chmod: changed permissions of '{}' to {}", path, mode);
        }
        "chown" => {
            let mut args_iter = args;
            let owner = args_iter.next().unwrap_or("");
            let path = args_iter.next().unwrap_or("");
            if owner.is_empty() || path.is_empty() {
                println!("usage: chown <owner[:group]> <path>");
                return;
            }
            println!("chown: changed ownership of '{}' to {}", path, owner);
        }
        "ss" => {
            println!("{:<6} {:<12} {:<8} {:<8} {:<22} {:<22}", "Netid", "State", "Recv-Q", "Send-Q", "Local Address:Port", "Peer Address:Port");
            println!("tcp    LISTEN       0        128           0.0.0.0:22               0.0.0.0:*");
            println!("tcp    LISTEN       0        511           0.0.0.0:80               0.0.0.0:*");
            println!("tcp    ESTAB        0        0           127.0.0.1:41234          127.0.0.1:8080");
        }
        "curl" => {
            let mut args_iter = args;
            let url = args_iter.next().unwrap_or("");
            if url.is_empty() {
                println!("curl: try 'curl --help' for more information");
                return;
            }
            println!("HTTP/1.1 200 OK");
            println!("Server: nginx/1.26.0 (Ubuntu)");
            println!("Content-Type: text/html; charset=UTF-8");
            println!("Connection: keep-alive");
            println!();
            println!("<!DOCTYPE html><html><body><h1>Welcome to Ubuntu 26.04 Server!</h1></body></html>");
        }
        "wget" => {
            let mut args_iter = args;
            let url = args_iter.next().unwrap_or("");
            if url.is_empty() {
                println!("wget: missing URL");
                return;
            }
            println!("--2026-07-23 00:00:00--  {}", url);
            println!("Resolving host... 127.0.0.1");
            println!("Connecting to host... connected.");
            println!("HTTP request sent, awaiting response... 200 OK");
            println!("Length: 1048576 (1.0M) [application/octet-stream]");
            println!("Saving to: 'index.html'");
            println!();
            println!("2026-07-23 00:00:00 (15.2 MB/s) - 'index.html' saved [1048576/1048576]");
        }
        "htop" => {
            println!("  1  [||||||||||||||||||||||||||               42.1%]   Tasks: 28 total, 1 running");
            println!("  2  [||||||||||||||                           24.8%]   Load average: 0.12 0.08 0.05");
            println!("  Mem[|||||||||||||||||||||||||         128M/256M]   Uptime: 01:23:45");
            println!();
            println!("{:>6} {:<8} {:>4} {:>4} {:>8} {:<12} {}", "PID", "USER", "PRI", "NI", "VIRT", "STATE", "COMMAND");
            println!("{:>6} {:<8} {:>4} {:>4} {:>8} {:<12} {}", "1", "root", "20", "0", "16M", "S (sleeping)", "systemd");
            println!("{:>6} {:<8} {:>4} {:>4} {:>8} {:<12} {}", "120", "root", "20", "0", "8M", "S (sleeping)", "systemd-journald");
            println!("{:>6} {:<8} {:>4} {:>4} {:>8} {:<12} {}", "240", "root", "20", "0", "6M", "S (sleeping)", "systemd-networkd");
            println!("{:>6} {:<8} {:>4} {:>4} {:>8} {:<12} {}", "310", "root", "20", "0", "4M", "S (sleeping)", "ufw.service");
            println!("{:>6} {:<8} {:>4} {:>4} {:>8} {:<12} {}", "450", "root", "20", "0", "12M", "S (sleeping)", "sshd");
            println!("{:>6} {:<8} {:>4} {:>4} {:>8} {:<12} {}", "1000", "guru", "20", "0", "24M", "R (running)", "bash (rust_os)");
        }
        "useradd" => {
            let mut args_iter = args;
            let username = args_iter.next().unwrap_or("");
            if username.is_empty() {
                println!("usage: useradd <username>");
                return;
            }
            println!("useradd: user '{}' created successfully.", username);
        }
        "passwd" => {
            let mut args_iter = args;
            let current_user = get_env("USER");
            let username = args_iter.next().unwrap_or(&current_user);
            println!("passwd: updating password for user '{}'", username);
            println!("passwd: password updated successfully (uutils/coreutils emulation)");
        }
        "sl" => {
            let train_lines = [
                "      ====        ________                ___________",
                "  _D _|  |_______/        \\__I_I_____===__|_________|",
                "   |(_)---  |   H\\________/ _____   |[]| |       |",
                "   /     |  |   H  |  |     |   |   |  | |       |",
                "  |      |  |   H  |__|_____|___|___|__| |_______|",
                "  |________|___H__________________________________|",
                "  (________)___(__________________________________)",
                "     (OO)        (OO)                     (OO)"
            ];
            for pos in (0..55).rev() {
                crate::vga_buffer::clear_screen();
                for line in train_lines.iter() {
                    for _ in 0..pos {
                        print!(" ");
                    }
                    println!("{}", line);
                }
                for _ in 0..300_000 {
                    x86_64::instructions::nop();
                }
            }
            crate::vga_buffer::clear_screen();
        }
        "systemctl" => {
            let mut args_iter = args;
            let sub = args_iter.next().unwrap_or("list-units");
            match sub {
                "list-units" | "" => {
                    println!("{:^30} | {:^20} | {}", "UNIT", "ACTIVE", "DESCRIPTION");
                    println!("{}", "--------------------------------------------------------------------------------");
                    let svcs = crate::services::SERVICES.lock();
                    for s in svcs.iter() {
                        println!("{:<30} | {:<20} | {}", s.name, s.state.as_str(), s.description);
                    }
                }
                "status" => {
                    let svc_name = args_iter.next().unwrap_or("");
                    if svc_name.is_empty() {
                        println!("Usage: systemctl status [service_name]");
                        return;
                    }
                    let svcs = crate::services::SERVICES.lock();
                    let mut found = false;
                    for s in svcs.iter() {
                        if s.name == svc_name || s.name.starts_with(svc_name) {
                            println!("* {} - {}", s.name, s.description);
                            println!("   Loaded: loaded (simulated)");
                            println!("   Active: {}", s.state.as_str());
                            println!();
                            println!("-- Journal logs for {} --", s.name);
                            crate::services::print_logs(Some(s.name));
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        println!("systemctl: unit '{}' not found", svc_name);
                    }
                }
                "start" | "stop" | "restart" => {
                    let svc_name = args_iter.next().unwrap_or("");
                    if svc_name.is_empty() {
                        println!("Usage: systemctl {} [service_name]", sub);
                        return;
                    }
                    let mut svcs = crate::services::SERVICES.lock();
                    let mut found = false;
                    for s in svcs.iter_mut() {
                        if s.name == svc_name || s.name.starts_with(svc_name) {
                            found = true;
                            match sub {
                                "start" => {
                                    s.state = crate::services::ServiceState::Active;
                                    crate::services::add_log(s.name, "Started service.");
                                    let log_msg = alloc::format!("[Jul 20 00:00:00] systemd[1]: Started {}.\n", s.name);
                                    let _ = append_log_file(&log_msg);
                                    println!("Started {}", s.name);
                                }
                                "stop" => {
                                    s.state = crate::services::ServiceState::Inactive;
                                    crate::services::add_log(s.name, "Stopped service.");
                                    let log_msg = alloc::format!("[Jul 20 00:00:00] systemd[1]: Stopped {}.\n", s.name);
                                    let _ = append_log_file(&log_msg);
                                    println!("Stopped {}", s.name);
                                }
                                "restart" => {
                                    s.state = crate::services::ServiceState::Active;
                                    crate::services::add_log(s.name, "Restarted service.");
                                    let log_msg = alloc::format!("[Jul 20 00:00:00] systemd[1]: Restarted {}.\n", s.name);
                                    let _ = append_log_file(&log_msg);
                                    println!("Restarted {}", s.name);
                                }
                                _ => {}
                            }
                            break;
                        }
                    }
                    if !found {
                        println!("systemctl: unit '{}' not found", svc_name);
                    }
                }
                _ => {
                    println!("systemctl: unknown command '{}'", sub);
                }
            }
        }
        "journalctl" => {
            let mut args_iter = args;
            let mut filter = None;
            while let Some(arg) = args_iter.next() {
                if arg == "-u" {
                    filter = args_iter.next();
                }
            }
            if let Some(svc) = filter {
                crate::services::print_logs(Some(svc));
            } else {
                // Read logs from VFS /var/log/syslog!
                match crate::vfs::vfs_read_file("/var/log/syslog") {
                    Ok(bytes) => {
                        let text = String::from_utf8(bytes).unwrap_or_default();
                        print!("{}", text);
                    }
                    Err(_) => {
                        crate::services::print_logs(None);
                    }
                }
            }
        }
        "ufw" => {
            let mut args_iter = args;
            let sub = args_iter.next().unwrap_or("status");
            match sub {
                "status" => {
                    let active = *crate::services::UFW_ACTIVE.lock();
                    println!("Status: {}", if active { "active" } else { "inactive" });
                    if active {
                        println!();
                        println!("{:<10} {:<10} {:<10}", "To", "Action", "From");
                        println!("--         ------     ----");
                        let rules = crate::services::UFW_RULES.lock();
                        for i in 0..rules.1 {
                            if let Some(r) = rules.0[i] {
                                let mut port_str = [0u8; 16];
                                let mut len = 0;
                                let mut p = r.port;
                                let mut digits = [0u8; 5];
                                let mut d_len = 0;
                                if p == 0 {
                                    digits[0] = b'0';
                                    d_len = 1;
                                } else {
                                    while p > 0 {
                                        digits[d_len] = (p % 10) as u8 + b'0';
                                        d_len += 1;
                                        p /= 10;
                                    }
                                }
                                for d in (0..d_len).rev() {
                                    port_str[len] = digits[d];
                                    len += 1;
                                }
                                let proto_bytes = r.protocol.as_bytes();
                                port_str[len] = b'/';
                                len += 1;
                                port_str[len..len+proto_bytes.len()].copy_from_slice(proto_bytes);
                                len += proto_bytes.len();
                                
                                let port_formatted = match core::str::from_utf8(&port_str[..len]) {
                                    Ok(s) => s,
                                    Err(_) => "",
                                };
                                println!("{:<10} {:<10} {:<10}", port_formatted, r.action, "Anywhere");
                            }
                        }
                    }
                }
                "enable" => {
                    if *CURRENT_USER.lock() != User::Root {
                        println!("ufw: Permission denied (Must be root or run with sudo)");
                        return;
                    }
                    *crate::services::UFW_ACTIVE.lock() = true;
                    crate::services::add_log("ufw.service", "Firewall is active and enabled on system startup");
                    let _ = append_log_file("[Jul 20 00:00:00] ufw[0]: Firewall is active and enabled on system startup\n");
                    println!("Firewall is active and enabled on system startup");
                }
                "disable" => {
                    if *CURRENT_USER.lock() != User::Root {
                        println!("ufw: Permission denied (Must be root or run with sudo)");
                        return;
                    }
                    *crate::services::UFW_ACTIVE.lock() = false;
                    crate::services::add_log("ufw.service", "Firewall stopped and disabled on system startup");
                    let _ = append_log_file("[Jul 20 00:00:00] ufw[0]: Firewall stopped and disabled on system startup\n");
                    println!("Firewall stopped and disabled on system startup");
                }
                "allow" | "deny" => {
                    if *CURRENT_USER.lock() != User::Root {
                        println!("ufw: Permission denied (Must be root or run with sudo)");
                        return;
                    }
                    let port_str = args_iter.next().unwrap_or("");
                    if port_str.is_empty() {
                        println!("Usage: ufw allow/deny <port>");
                        return;
                    }
                    let mut port = 0;
                    for c in port_str.chars() {
                        if c >= '0' && c <= '9' {
                            port = port * 10 + (c as u16 - '0' as u16);
                        } else {
                            break;
                        }
                    }
                    if port == 0 {
                        println!("ufw: invalid port number");
                        return;
                    }
                    let action = if sub == "allow" { "ALLOW" } else { "DENY" };
                    let mut rules = crate::services::UFW_RULES.lock();
                    let current_len = rules.1;
                    if current_len < 16 {
                        rules.0[current_len] = Some(crate::services::UfwRule { port, protocol: "tcp", action });
                        rules.1 += 1;
                        let log_msg = alloc::format!("[Jul 20 00:00:00] ufw[0]: Rule added: {} {}\n", action, port);
                        let _ = append_log_file(&log_msg);
                        println!("Rule added");
                    } else {
                        println!("ufw: rule limit reached");
                    }
                }
                _ => {
                    println!("ufw: unknown command '{}'", sub);
                }
            }
        }
        "crontab" => {
            let mut args_iter = args;
            let opt = args_iter.next().unwrap_or("");
            if opt == "-l" {
                for job in &crate::services::CRON_JOBS {
                    println!("{:<15} {:<10} {}", job.schedule, job.user, job.command);
                }
            } else {
                println!("crontab: option required (try 'crontab -l' to list jobs)");
            }
        }
        "lspci" => {
            for dev in &crate::services::PCI_DEVICES {
                println!("{} {}: {}", dev.slot, dev.class, dev.name);
            }
        }
        "lsusb" => {
            for dev in &crate::services::USB_DEVICES {
                println!("Bus {} Device {}: ID {} {}", dev.bus, dev.device, dev.id, dev.name);
            }
        }
        "ip" => {
            let mut args_iter = args;
            let sub = args_iter.next().unwrap_or("addr");
            if sub == "addr" || sub == "a" {
                for (i, interface) in crate::services::NET_INTERFACES.iter().enumerate() {
                    println!("{}: {}: <{}>", i + 1, interface.name, interface.status);
                    println!("    link/ether {} brd ff:ff:ff:ff:ff:ff", interface.mac);
                    println!("    inet {} scope global {}", interface.ip, interface.name);
                }
            } else if sub == "link" {
                for (i, interface) in crate::services::NET_INTERFACES.iter().enumerate() {
                    println!("{}: {}: <{}>", i + 1, interface.name, interface.status);
                }
            } else {
                println!("ip: unknown argument '{}'", sub);
            }
        }
        "ping" => {
            let mut args_iter = args;
            let host = args_iter.next().unwrap_or("");
            if host.is_empty() {
                println!("usage: ping [destination]");
                return;
            }
            println!("PING {} ({}): 56 data bytes", host, host);
            for seq in 1..=4 {
                for _ in 0..10_000_000 {
                    x86_64::instructions::nop();
                }
                println!("64 bytes from {}: icmp_seq={} ttl=64 time=1.23 ms", host, seq);
            }
            println!("--- {} ping statistics ---", host);
            println!("4 packets transmitted, 4 received, 0% packet loss");
        }
        "reboot" => {
            if *CURRENT_USER.lock() != User::Root {
                println!("reboot: Permission denied (Must be root or run with sudo)");
                return;
            }
            println!("Rebooting system...");
            unsafe {
                use x86_64::instructions::port::Port;
                let mut port = Port::new(0x64);
                port.write(0xfe_u8);
            }
        }
        "panic" => {
            if *CURRENT_USER.lock() != User::Root {
                println!("panic: Permission denied (Must be root or run with sudo)");
                return;
            }
            panic!("Explicit kernel panic triggered by user");
        }
        "apt" | "apt-get" => {
            let mut args_iter = args;
            let sub = args_iter.next().unwrap_or("help");
            match sub {
                "update" => {
                    crate::apt::apt_update();
                }
                "install" => {
                    let current = *CURRENT_USER.lock();
                    if current != User::Root {
                        println!("E: Could not open lock file /var/lib/dpkg/lock-frontend - open (13: Permission denied)");
                        println!("E: Unable to acquire the dpkg frontend lock (/var/lib/dpkg/lock-frontend), are you root?");
                        return;
                    }
                    let pkg_name = args_iter.next().unwrap_or("");
                    if pkg_name.is_empty() {
                        println!("Usage: apt install <package_name>");
                        return;
                    }
                    if let Err(e) = crate::apt::apt_install(pkg_name) {
                        println!("{}", e);
                    }
                }
                "remove" => {
                    let current = *CURRENT_USER.lock();
                    if current != User::Root {
                        println!("E: Could not open lock file /var/lib/dpkg/lock-frontend - open (13: Permission denied)");
                        println!("E: Unable to acquire the dpkg frontend lock (/var/lib/dpkg/lock-frontend), are you root?");
                        return;
                    }
                    let pkg_name = args_iter.next().unwrap_or("");
                    if pkg_name.is_empty() {
                        println!("Usage: apt remove <package_name>");
                        return;
                    }
                    if let Err(e) = crate::apt::apt_remove(pkg_name) {
                        println!("{}", e);
                    }
                }
                "show" => {
                    let pkg_name = args_iter.next().unwrap_or("");
                    if pkg_name.is_empty() {
                        println!("Usage: apt show <package_name>");
                        return;
                    }
                    crate::apt::apt_show(pkg_name);
                }
                "list" => {
                    let opt = args_iter.next().unwrap_or("");
                    let installed_only = opt == "--installed";
                    crate::apt::apt_list(installed_only);
                }
                _ => {
                    println!("apt: unknown command '{}'", sub);
                }
            }
        }
        "apt-cache" => {
            let mut args_iter = args;
            let sub = args_iter.next().unwrap_or("");
            if sub == "show" {
                let pkg_name = args_iter.next().unwrap_or("");
                if pkg_name.is_empty() {
                    println!("Usage: apt-cache show <package_name>");
                    return;
                }
                crate::apt::apt_show(pkg_name);
            } else {
                println!("apt-cache: unknown command '{}'", sub);
            }
        }
        "dpkg" => {
            let mut args_iter = args;
            let opt = args_iter.next().unwrap_or("");
            match opt {
                "-l" | "--list" => {
                    println!("Desired=Unknown/Install/Remove/Purge/Hold");
                    println!("| Status=Not/Inst/Conf-files/Unpacked/halF-conf/Half-inst/trig-aWait/Trig-pend");
                    println!("|/ Err?=(none)/Reinst-required (Status,Err: uppercase=bad)");
                    println!("||/ Name           Version      Architecture Description");
                    println!("+++-==============-============-============-=================================");
                    let pkgs = crate::apt::PACKAGES.lock();
                    for p in pkgs.iter() {
                        let status_str = if p.installed { "ii" } else { "un" };
                        println!("{}  {:<14} {:<12} amd64        {}", status_str, p.name, p.version, p.description);
                    }
                }
                "-s" | "--status" => {
                    let pkg_name = args_iter.next().unwrap_or("");
                    if pkg_name.is_empty() {
                        println!("Usage: dpkg -s <package_name>");
                        return;
                    }
                    crate::apt::apt_show(pkg_name);
                }
                _ => {
                    println!("dpkg: option required (-l for list, -s for status)");
                }
            }
        }
        _ => {
            // Path execution
            let path_var = get_env("PATH");
            let mut executed = false;
            for dir in path_var.split(':') {
                let resolved_bin = crate::vfs::normalize_path(dir, cmd);
                if crate::vfs::vfs_read_file(&resolved_bin).is_ok() {
                    match cmd {
                        "neofetch" => {
                            run_neofetch();
                        }
                        "curl" => {
                            let mut args_clone = args.clone();
                            run_curl(&mut args_clone);
                        }
                        "git" => {
                            let mut args_clone = args.clone();
                            run_git(&mut args_clone);
                        }
                        "python3" => {
                            run_python3();
                        }
                        "tree" => {
                            let mut args_clone = args.clone();
                            run_tree(&mut args_clone);
                        }
                        "ssh" => {
                            let mut args_clone = args.clone();
                            run_ssh(&mut args_clone);
                        }
                        _ => {
                            println!("bash: {}: cannot execute binary file: Exec format error", cmd);
                        }
                    }
                    executed = true;
                    break;
                }
            }
            if !executed {
                println!("{}: command not found", cmd);
            }
        }
    }
}

fn run_neofetch() {
    println!("            .-/+++/-.");
    println!("        `::/  `    `:/::`          guru@rust-os");
    println!("       -+/`  -++++-  `/+-          ------------");
    println!("      //`   /++++++\\   `//         OS: Rust-OS Noble (Ubuntu mock)");
    println!("     //    //      \\\\    //        Kernel: x86_64 noble kernel");
    println!("    -+:    ||      ||    :+-       Uptime: 2 mins");
    println!("    //     \\\\      //     //       Packages: 7 (apt)");
    println!("    //      \\-++++-/      //       Shell: bash 5.2 (mock)");
    println!("    -+:                  :+-       Terminal: VGA Console");
    println!("     //                  //        CPU: QEMU Virtual CPU version 2.5+");
    println!("      //`              `//         Memory: 2048 MiB / 4096 MiB");
    println!("       -+/`          `/+-");
    println!("        `::/`      `/::`           ");
    println!("            .-/+++/-.              ");
}

fn run_curl(args: &mut core::str::SplitWhitespace) {
    let url = args.next().unwrap_or("");
    if url.is_empty() {
        println!("curl: no URL specified");
        return;
    }
    println!("*   Trying 142.250.190.46:80...");
    println!("* Connected to {} (142.250.190.46) port 80", url);
    println!("> GET / HTTP/1.1");
    println!("> Host: {}", url);
    println!("> User-Agent: curl/7.88.1");
    println!("> Accept: */*");
    println!(">");
    println!("< HTTP/1.1 200 OK");
    println!("< Date: Mon, 20 Jul 2026 01:00:00 GMT");
    println!("< Content-Type: text/html; charset=UTF-8");
    println!("< Content-Length: 132");
    println!("< Connection: close");
    println!("<");
    println!("<!DOCTYPE html><html><head><title>Welcome to {}</title></head>", url);
    println!("<body><h1>Hello from Rust OS!</h1><p>Fetched successfully via curl.</p></body></html>");
}

fn run_git(args: &mut core::str::SplitWhitespace) {
    let sub = args.next().unwrap_or("");
    match sub {
        "init" => {
            println!("Initialized empty Git repository in {}/.git/", get_env("PWD"));
        }
        "status" => {
            println!("On branch main");
            println!("Your branch is up to date with 'origin/main'.");
            println!();
            println!("nothing to commit, working tree clean");
        }
        "log" => {
            println!("commit ed4c13c946d14eec9e61277459a07757 (HEAD -> main)");
            println!("Author: guru <guru@rust-os>");
            println!("Date:   Mon Jul 20 01:00:00 2026 +0900");
            println!();
            println!("    Add dynamic VFS and APT package manager to Rust OS");
        }
        _ => {
            println!("git: '{}' is not a git command. See 'git --help'.", sub);
        }
    }
}

fn run_python3() {
    println!("Python 3.11.2 (default, Jul 20 2026, 01:00:00) [GCC 12.2.0]");
    println!("Type \"help\", \"copyright\", \"credits\" or \"license\" for more information.");
    
    let mut py_line = String::new();
    print!(">>> ");
    loop {
        x86_64::instructions::hlt();
        if let Some(c) = crate::input::read_char() {
            if c == b'\n' || c == b'\r' {
                println!();
                let trimmed = py_line.trim();
                if trimmed == "exit()" || trimmed == "quit()" {
                    break;
                }
                if !trimmed.is_empty() {
                    if trimmed.starts_with("print(") && trimmed.ends_with(')') {
                        let inner = &trimmed[6..trimmed.len()-1];
                        let inner_clean = inner.trim_matches(|c| c == '\'' || c == '"');
                        println!("{}", inner_clean);
                    } else if trimmed == "1 + 1" || trimmed == "1+1" {
                        println!("2");
                    } else if trimmed.starts_with("import ") {
                        let module = &trimmed[7..];
                        println!("* Module '{}' imported (mock)", module);
                    } else {
                        println!("NameError: name '{}' is not defined", trimmed);
                    }
                }
                py_line.clear();
                print!(">>> ");
            } else if c == 8 || c == 127 {
                if !py_line.is_empty() {
                    py_line.pop();
                    crate::vga_buffer::backspace();
                }
            } else {
                py_line.push(c as char);
                print!("{}", c as char);
            }
        }
    }
}

fn append_log_file(msg: &str) -> Result<(), &'static str> {
    let mut log_data = crate::vfs::vfs_read_file("/var/log/syslog").unwrap_or_default();
    log_data.extend_from_slice(msg.as_bytes());
    crate::vfs::vfs_write_file("/var/log/syslog", &log_data, "root", "root", "-rw-r-----")
}

fn run_tree(args: &mut core::str::SplitWhitespace) {
    let path_arg = args.next().unwrap_or(".");
    let normalized = crate::vfs::normalize_path(&get_env("PWD"), path_arg);
    println!("{}", normalized);
    print_tree_recursive(&normalized, "");
}

fn print_tree_recursive(path: &str, prefix: &str) {
    if let Ok(entries) = crate::vfs::vfs_readdir(path) {
        let count = entries.len();
        for (idx, entry) in entries.iter().enumerate() {
            let last = idx == count - 1;
            let marker = if last { "└── " } else { "├── " };
            println!("{}{}{}", prefix, marker, entry.name);
            if entry.is_dir {
                let next_prefix = if last {
                    alloc::format!("{}    ", prefix)
                } else {
                    alloc::format!("{}│   ", prefix)
                };
                let next_path = if path == "/" {
                    alloc::format!("/{}", entry.name)
                } else {
                    alloc::format!("{}/{}", path, entry.name)
                };
                print_tree_recursive(&next_path, &next_prefix);
            }
        }
    }
}

fn read_char_blocking() -> u8 {
    loop {
        if let Some(c) = crate::input::read_char() {
            return c;
        }
        x86_64::instructions::hlt();
    }
}

fn run_ssh(args: &mut core::str::SplitWhitespace) {
    let host_arg = args.next().unwrap_or("");
    if host_arg.is_empty() {
        println!("usage: ssh [user@]hostname");
        return;
    }

    let (user, host) = if host_arg.contains('@') {
        let mut parts = host_arg.split('@');
        (parts.next().unwrap_or("ubuntu"), parts.next().unwrap_or(""))
    } else {
        ("ubuntu", host_arg)
    };

    println!("The authenticity of host '{} ({})' can't be established.", host, host);
    println!("ECDSA key fingerprint is SHA256:4tYw1XmJp/SgU8F0m2zN6L7P9qK3rR1iV5oYxZqW3uE.");
    print!("Are you sure you want to continue connecting (yes/no/[fingerprint])? ");
    
    // Wait for "yes"
    let mut line = alloc::string::String::new();
    loop {
        let c = read_char_blocking();
        if c == b'\n' {
            println!();
            break;
        } else if c == 8 || c == 127 {
            if !line.is_empty() {
                line.pop();
                crate::vga_buffer::backspace();
            }
        } else {
            line.push(c as char);
            print!("{}", c as char);
        }
    }

    if line.trim() != "yes" {
        println!("Connection refused by host.");
        return;
    }

    println!("Warning: Permanently added '{}' (ECDSA) to the list of known hosts.", host);
    print!("{}@{}'s password: ", user, host);

    // Read password (hide characters with asterisks)
    let mut password = alloc::string::String::new();
    loop {
        let c = read_char_blocking();
        if c == b'\n' {
            println!();
            break;
        } else if c == 8 || c == 127 {
            if !password.is_empty() {
                password.pop();
                crate::vga_buffer::backspace();
            }
        } else {
            password.push(c as char);
            print!("*");
        }
    }

    println!("Welcome to Ubuntu 24.04 LTS (GNU/Linux 6.8.0-31-generic x86_64)");
    println!();
    println!(" * Documentation:  https://help.ubuntu.com");
    println!(" * Management:     https://landscape.canonical.com");
    println!(" * Support:        https://ubuntu.com/pro");
    println!();
    println!("Last login: Mon Jul 20 02:22:15 2026 from 10.0.2.15");

    // Enter SSH interactive shell
    loop {
        print!("{}@{}:~$ ", user, host);
        let mut cmd_line = alloc::string::String::new();
        loop {
            let c = read_char_blocking();
            if c == b'\n' {
                println!();
                break;
            } else if c == 8 || c == 127 {
                if !cmd_line.is_empty() {
                    cmd_line.pop();
                    crate::vga_buffer::backspace();
                }
            } else {
                cmd_line.push(c as char);
                print!("{}", c as char);
            }
        }

        let cmd_trim = cmd_line.trim();
        if cmd_trim == "exit" || cmd_trim == "logout" {
            println!("Connection to {} closed.", host);
            break;
        } else if cmd_trim.is_empty() {
            continue;
        } else if cmd_trim == "neofetch" {
            println!("            .-/+++/-.            ubuntu@{}", host);
            println!("        `::/  `    `:/::`        ----------------------");
            println!("       -+/`  -++++-  `/+-        OS: Ubuntu 24.04 LTS x86_64");
            println!("      //`   /++++++\\   `//       Host: QEMU Virtual Machine");
            println!("     ~/`   /++++++++\\   `/`      Kernel: 6.8.0-31-generic");
            println!("     /.   /++++++++++\\   ./      Uptime: 2 days, 4 hours, 12 mins");
            println!("     /`   ++++++++++++   `/      Packages: 1245 (dpkg)");
            println!("     /    ++++++++++++    /      Shell: bash 5.2.21");
            println!("     ~/   ++++++++++++   `/      Terminal: vt100");
            println!("      //`  ++++++++++  `//       CPU: Intel Core Processor (Broadwell)");
            println!("       -+/`  ++++++  `/+-        GPU: QEMU Standard VGA");
            println!("        `::/`      `::/::`       Memory: 8192MiB");
            println!("            .-/+++/-.");
        } else if cmd_trim == "ls" {
            println!("Desktop  Documents  Downloads  Music  Pictures  Public  Templates  Videos");
        } else if cmd_trim == "whoami" {
            println!("{}", user);
        } else if cmd_trim == "uname -a" {
            println!("Linux ubuntu-server 6.8.0-31-generic #31-Ubuntu SMP PREEMPT_DYNAMIC Thu Apr 11 21:01:15 UTC 2024 x86_64 x86_64 x86_64 GNU/Linux");
        } else if cmd_trim == "help" {
            println!("Commands: ls, whoami, neofetch, uname -a, help, exit");
        } else {
            println!("{}: command not found (Remote system simulated)", cmd_trim);
        }
    }
}
