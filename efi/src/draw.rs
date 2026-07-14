use core::fmt::Write;
use uefi::proto::console::text::Color;
use uefi::system;

use crate::environment;

const BACKGROUND: Color = Color::Black;

pub fn draw_menu(selected: usize, items: &[&str], env: &environment::ENV) {
    system::with_stdout(|out| {
        out.clear().unwrap();
        match env {
            environment::ENV::MENU => {
                for (i, item) in items.iter().enumerate() {
                    if i == selected {
                        out.set_color(Color::Magenta, BACKGROUND).unwrap();
                    } else {
                        out.set_color(Color::LightGray, BACKGROUND).unwrap();
                    };

                    center_line(out, item, i);

                    out.write_fmt(format_args!("{item}\n")).unwrap();
                }
            }
            environment::ENV::OS => {
                uefi::println!("  {}", items[selected]);
            }
        }
    });
}

pub fn center_line(out: &mut uefi::proto::console::text::Output, text: &str, row_index: usize) {
    let (cols, rows) = out
        .current_mode()
        .ok()
        .flatten()
        .map(|m| (m.columns(), m.rows()))
        .unwrap();
    let col = cols.saturating_sub(text.chars().count()) / 2;
    let row = rows / 3;
    out.set_cursor_position(col, row + row_index).unwrap();
}
