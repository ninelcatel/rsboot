#![no_main]
#![no_std]

use uefi::Char16;
use uefi::prelude::*;
use uefi::proto::console::text::{Key, ScanCode};
use uefi::system;

mod draw;
mod environment;

use environment::ENV;

const OS: [&str; 4] = ["Debian", "Arch Linux", "Ubuntu", "Fedora"];

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();

    let mut running: bool = true;

    let mut env = ENV::MENU;
    let mut current_pick: usize = 0;

    draw::clear_screen();
    draw::draw_menu(current_pick, &OS, &env);

    while running {
        let Ok(Some(key)) = system::with_stdin(|stdin| stdin.read_key()) else {
            continue;
        };

        match (&env, key) {
            (ENV::MENU, Key::Special(ScanCode::UP)) => {
                let old = current_pick;
                current_pick = current_pick.saturating_sub(1);
                if current_pick != old {
                    draw::update_selection(&OS, old, current_pick);
                }
                continue;
            }
            (ENV::MENU, Key::Special(ScanCode::DOWN)) => {
                let old = current_pick;
                if current_pick + 1 < OS.len() {
                    current_pick += 1;
                    draw::update_selection(&OS, old, current_pick);
                }
                continue;
            }
            (ENV::MENU, Key::Special(ScanCode::ESCAPE)) => {
                running = false;
                draw::clear_screen();
            }
            (ENV::MENU, Key::Printable(c)) if c == Char16::try_from('\r').unwrap() => {
                env = ENV::OS;
            }
            (ENV::OS, Key::Special(ScanCode::ESCAPE)) => {
                env = ENV::MENU;
            }
            _ => continue,
        }

        draw::draw_menu(current_pick, &OS, &env);
    }

    Status::SUCCESS
}
