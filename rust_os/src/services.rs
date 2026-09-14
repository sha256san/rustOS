use spin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    Active,
    Inactive,
}

impl ServiceState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ServiceState::Active => "active (running)",
            ServiceState::Inactive => "inactive (dead)",
        }
    }
}

#[derive(Clone, Copy)]
pub struct Service {
    pub name: &'static str,
    pub description: &'static str,
    pub state: ServiceState,
}

#[derive(Clone, Copy)]
pub struct LogEntry {
    pub service: &'static str,
    pub message: [u8; 64],
    pub message_len: usize,
    pub timestamp: &'static str,
}

const LOG_LIMIT: usize = 64;
static mut LOGS: [Option<LogEntry>; LOG_LIMIT] = [None; LOG_LIMIT];
static mut LOG_COUNT: usize = 0;

pub fn add_log(service: &'static str, msg: &str) {
    unsafe {
        let index = LOG_COUNT % LOG_LIMIT;
        let mut entry = LogEntry {
            service,
            message: [0; 64],
            message_len: 0,
            timestamp: "Jul 19 16:20:00",
        };
        let bytes = msg.as_bytes();
        let len = if bytes.len() > 64 { 64 } else { bytes.len() };
        entry.message[..len].copy_from_slice(&bytes[..len]);
        entry.message_len = len;
        LOGS[index] = Some(entry);
        LOG_COUNT += 1;
    }
}

pub fn print_logs(filter_service: Option<&str>) {
    unsafe {
        let count = LOG_COUNT;
        let start = if count > LOG_LIMIT { count - LOG_LIMIT } else { 0 };
        for i in start..count {
            let index = i % LOG_LIMIT;
            if let Some(entry) = &LOGS[index] {
                if let Some(filt) = filter_service {
                    if entry.service != filt {
                        continue;
                    }
                }
                let msg_str = match core::str::from_utf8(&entry.message[..entry.message_len]) {
                    Ok(s) => s,
                    Err(_) => "",
                };
                crate::println!("[{}] {}[{}]: {}", entry.timestamp, entry.service, i, msg_str);
            }
        }
    }
}

pub static SERVICES: spin::Mutex<[Service; 6]> = spin::Mutex::new([
    Service { name: "systemd-journald.service", description: "Journal Service", state: ServiceState::Active },
    Service { name: "systemd-networkd.service", description: "Network Service", state: ServiceState::Active },
    Service { name: "udev.service", description: "udev Device Event Manager", state: ServiceState::Active },
    Service { name: "cron.service", description: "cron Periodic Command Scheduler", state: ServiceState::Active },
    Service { name: "ufw.service", description: "Uncomplicated Firewall", state: ServiceState::Active },
    Service { name: "ssh.service", description: "OpenSSH server daemon", state: ServiceState::Active },
]);

#[derive(Clone, Copy)]
pub struct UfwRule {
    pub port: u16,
    pub protocol: &'static str,
    pub action: &'static str,
}

pub static UFW_ACTIVE: spin::Mutex<bool> = spin::Mutex::new(true);

const UFW_RULE_LIMIT: usize = 16;
pub static UFW_RULES: spin::Mutex<([Option<UfwRule>; UFW_RULE_LIMIT], usize)> = spin::Mutex::new(([None; UFW_RULE_LIMIT], 0));

pub fn init_ufw() {
    let mut rules = UFW_RULES.lock();
    rules.0[0] = Some(UfwRule { port: 22, protocol: "tcp", action: "ALLOW" });
    rules.0[1] = Some(UfwRule { port: 80, protocol: "tcp", action: "ALLOW" });
    rules.1 = 2;
}

pub struct PciDevice {
    pub slot: &'static str,
    pub class: &'static str,
    pub name: &'static str,
}

pub const PCI_DEVICES: [PciDevice; 5] = [
    PciDevice { slot: "00:00.0", class: "Host bridge", name: "Intel Corporation 440FX - 82441FX PMC [Natoma]" },
    PciDevice { slot: "00:01.0", class: "ISA bridge", name: "Intel Corporation 82371SB PIIX3 ISA [Triton II]" },
    PciDevice { slot: "00:02.0", class: "VGA compatible controller", name: "Red Hat, Inc. QXL paravirtual graphic card" },
    PciDevice { slot: "00:03.0", class: "Ethernet controller", name: "Intel Corporation 82540EM Gigabit Ethernet Controller" },
    PciDevice { slot: "00:04.0", class: "SATA controller", name: "Intel Corporation 82801IR/IO/IH (ICH9R/DO/DH) 6 port SATA Controller [AHCI mode]" },
];

pub struct UsbDevice {
    pub bus: &'static str,
    pub device: &'static str,
    pub id: &'static str,
    pub name: &'static str,
}

pub const USB_DEVICES: [UsbDevice; 3] = [
    UsbDevice { bus: "001", device: "001", id: "1d6b:0002", name: "Linux Foundation 2.0 root hub" },
    UsbDevice { bus: "001", device: "002", id: "0627:0001", name: "Adomax Technology Co., Ltd QEMU USB Tablet" },
    UsbDevice { bus: "001", device: "003", id: "04f2:0110", name: "Chicony Electronics Co., Ltd Keyboard" },
];

pub struct NetInterface {
    pub name: &'static str,
    pub mac: &'static str,
    pub ip: &'static str,
    pub _mask: &'static str,
    pub status: &'static str,
}

pub const NET_INTERFACES: [NetInterface; 2] = [
    NetInterface { name: "lo", mac: "00:00:00:00:00:00", ip: "127.0.0.1/8", _mask: "255.0.0.0", status: "LOOPBACK,UP,LOWER_UP" },
    NetInterface { name: "enp0s3", mac: "52:54:00:12:34:56", ip: "10.0.2.15/24", _mask: "255.255.255.0", status: "BROADCAST,MULTICAST,UP,LOWER_UP" },
];

pub struct CronJob {
    pub schedule: &'static str,
    pub user: &'static str,
    pub command: &'static str,
}

pub const CRON_JOBS: [CronJob; 3] = [
    CronJob { schedule: "*/5 * * * *", user: "guru", command: "/home/guru/backup_config.sh" },
    CronJob { schedule: "0 0 * * *", user: "root", command: "apt-get update && apt-get upgrade -y" },
    CronJob { schedule: "0 1 * * *", user: "root", command: "/usr/sbin/logrotate /etc/logrotate.conf" },
];
