#![no_main]
#![no_std]

use uefi::Char16;
use uefi::prelude::*;
use uefi::proto::console::text::{Key, ScanCode};
use uefi::system;

mod downloader;
mod draw;
mod environment;
mod loadbootable;

use environment::Env;

use crate::loadbootable::boot_from_iso;

const OS: [environment::OS; 6] = [
    environment::OS {
        name: "Debian",
        url: "http://ftp2.de.debian.org/debian/dists/trixie/main/installer-amd64/20250803+deb13u6/images/netboot/mini.iso",
        boot_method: environment::BootMethod::RamDisk,
    },
    environment::OS {
        name: "Arch Linux",
        url: "http://10.0.2.2:8000/arch.iso",
        boot_method: environment::BootMethod::Memmap,
    },
    environment::OS {
        name: "Ubuntu",
        url: "http://10.0.2.2:8000/ubuntu.iso",
        boot_method: environment::BootMethod::RamDisk,
    },
    environment::OS {
        name: "Fedora",
        url: "http://10.0.2.2:8000/fedora.iso",
        boot_method: environment::BootMethod::RamDisk,
    },
    environment::OS {
        name: "CachyOS",
        url: "http://10.0.2.2:8000/cachy.iso",
        boot_method: environment::BootMethod::Memmap,
    },
    environment::OS {
        name: "Gentoo",
        url: "http://10.0.2.2:8000/gentoo_gui.iso",
        boot_method: environment::BootMethod::LoopInjection,
    },
];

#[entry]
fn main() -> Status {
    match run() {
        Ok(()) => Status::SUCCESS,
        Err(e) => e.status(),
    }
}
fn run() -> uefi::Result {
    uefi::helpers::init()?;
    log::set_max_level(log::LevelFilter::Info); // without this, it adds unnecessarry buffering and
    // downloads REALLY slow
    let mut dl = downloader::Downloader::connect()?;

    let mut running: bool = true;

    let enter: Char16 = Char16::try_from('\r').expect("'\\r' should always be a valid char");

    let mut env = Env::Menu;
    let mut current_pick: usize = 0;

    system::with_stdout(|out| {
        let mut tui = draw::Tui::new(out);
        tui.clear_screen();

        let names: [&str; OS.len()] = core::array::from_fn(|i| OS[i].name);
        tui.draw_menu(current_pick, &names, &env);

        while running {
            let key = system::with_stdin(|input| {
                if let Ok(event) = input.wait_for_key_event() {
                    let _ = uefi::boot::wait_for_event(&mut [event]);
                }
                input.read_key()
            });
            let Ok(Some(key)) = key else {
                continue;
            };

            match (&env, key) {
                (Env::Menu, Key::Special(ScanCode::UP)) => {
                    let old = current_pick;
                    current_pick = current_pick.saturating_sub(1);
                    if current_pick != old {
                        tui.update_selection(&names, old, current_pick);
                    }
                    continue;
                }
                (Env::Menu, Key::Special(ScanCode::DOWN)) => {
                    let old = current_pick;
                    if current_pick + 1 < OS.len() {
                        current_pick += 1;
                        tui.update_selection(&names, old, current_pick);
                    }
                    continue;
                }
                (Env::Menu, Key::Special(ScanCode::ESCAPE)) => {
                    running = false;
                    tui.clear_screen();
                }
                (Env::Menu, Key::Printable(c)) if c == enter => {
                    let os = &OS[current_pick];
                    let buffer = dl.get(os.url).unwrap();
                    boot_from_iso(buffer, os.boot_method).unwrap();
                }
                (Env::Os, Key::Special(ScanCode::ESCAPE)) => {
                    env = Env::Menu;
                }
                _ => continue,
            }

            tui.draw_menu(current_pick, &names, &env);
        }
    });

    Ok(())
}
