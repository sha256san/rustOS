#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::panic::PanicInfo;

mod vga_buffer;
mod gdt;
mod interrupts;
mod input;
mod shell;
mod services;
mod allocator;
mod vfs;
mod apt;
mod serial2;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // Initialize COM2 serial port for networking
    serial2::SERIAL2.lock().init();

    // Initialize global allocator
    unsafe {
        allocator::ALLOCATOR.init();
    }

    // Initialize VFS
    vfs::init_vfs();

    // Initialize Shell environment variables
    shell::init_env();

    // Clear screen first
    vga_buffer::clear_screen();
    
    println!("OS Booting...");

    // Initialize GDT and TSS
    gdt::init();

    // Initialize Interrupts (IDT)
    interrupts::init_idt();

    // Initialize PIC
    unsafe {
        interrupts::PICS.lock().initialize();
    }

    // Enable CPU interrupts
    x86_64::instructions::interrupts::enable();

    // Initialize services / logs
    services::init_ufw();
    services::add_log("systemd-journald.service", "Journal Service started.");
    services::add_log("systemd-networkd.service", "Network Service started.");
    services::add_log("udev.service", "udev Device Event Manager started.");
    services::add_log("cron.service", "cron Periodic Command Scheduler started.");
    services::add_log("ufw.service", "Uncomplicated Firewall started.");
    services::add_log("ssh.service", "OpenSSH server daemon started.");

    println!("Welcome to Ubuntu-mock OS! Base infrastructure loaded.");
    println!("Type 'help' to see list of available commands.");
    println!();

    // Start the interactive shell
    shell::run();
}

/// This function is called on panic.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    hlt_loop();
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
