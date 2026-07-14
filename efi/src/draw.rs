use uefi::system;

use crate::environment;

pub fn draw_menu(selected: usize, items: &[&str], env: &environment::ENV) {
    system::with_stdout(|out| {
        out.clear().unwrap();
        match env {
            environment::ENV::MENU => {
                for (i, item) in items.iter().enumerate() {
                    let marker = if i == selected { "* " } else { "  " };

                    uefi::println!("{marker}{item}");
                }
            }
            environment::ENV::OS => {
                uefi::println!("  {}", items[selected]);
            }
        }
    });
}
