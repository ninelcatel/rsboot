#![no_main]
#![no_std]

use uefi::prelude::*;
use uefi::proto::console::text::{Key, ScanCode};
use uefi::system;

mod draw;

const OS: [&str; 4] = ["Debian", "Arch Linux", "Ubuntu", "Fedora"];

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();

    let mut running: bool = true;

    let mut current_pick: usize = 0;

    while running {
        draw::draw_menu(current_pick, &OS);
        if let Ok(Some(key)) = system::with_stdin(|stdin| stdin.read_key()) {
            match key {
                Key::Special(ScanCode::UP) => current_pick = current_pick.saturating_sub(1),
                Key::Special(ScanCode::DOWN) => {
                    if current_pick + 1 < OS.len() {
                        current_pick += 1
                    }
                }
                Key::Special(ScanCode::ESCAPE) => running = false,
                _ => {}
            }
        }
    }

    Status::SUCCESS
}
