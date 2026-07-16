use core::fmt::Write;
use uefi::proto::console::text::Color;

use crate::environment;

const RECT: Color = Color::LightGray;
const TITLE: &str = "Welcome to";
const SUBTITLE: &str = "Please select the OS you wish to install.";
const PROJECT_TITLE: &str = "rsboot";

pub struct Tui<'a> {
    out: &'a mut uefi::proto::console::text::Output,
    cols: usize,
    rows: usize,
}

impl<'a> Tui<'a> {
    pub fn new(out: &mut uefi::proto::console::text::Output) -> Tui {
        let (cols, rows) = Tui::get_dimensions(out);
        Tui { out, cols, rows }
    }

    pub fn clear_screen(&mut self) {
        self.out.set_color(Color::LightGray, Color::Black).unwrap(); // set colors to default
        self.out.clear().unwrap();
    }

    pub fn draw_menu(&mut self, selected: usize, items: &[&str], env: &environment::Env) {
        self.draw_rect();
        self.draw_title();

        match env {
            environment::Env::Menu => {
                for i in 0..items.len() {
                    self.draw_item(items, i, selected);
                }
            }
            environment::Env::Os => {
                self.out.set_color(Color::Magenta, RECT).unwrap();

                self.center_line(items[selected], 0);

                self.out
                    .write_fmt(format_args!("{}", items[selected]))
                    .unwrap();
            }
        }
    }

    fn draw_item(&mut self, items: &[&str], i: usize, selected: usize) {
        if i == selected {
            self.out.set_color(Color::Magenta, RECT).unwrap();
        } else {
            self.out.set_color(Color::Black, RECT).unwrap();
        }
        self.center_line(items[i], i);
        self.out.write_fmt(format_args!("{}", items[i])).unwrap();
    }

    // helper function to overwrite only the 2 affected selections
    pub fn update_selection(&mut self, items: &[&str], old: usize, new: usize) {
        self.draw_item(items, old, new);
        self.draw_item(items, new, new);
    }

    fn get_dimensions(out: &uefi::proto::console::text::Output) -> (usize, usize) {
        let (cols, rows) = out
            .current_mode()
            .ok()
            .flatten()
            .map(|m| (m.columns(), m.rows()))
            .unwrap();
        (cols, rows)
    }

    fn draw_title(&mut self) {
        let col = self
            .cols
            .saturating_sub(TITLE.len() + PROJECT_TITLE.len() + 2)
            / 2; // + 2 pentru spatii!"
        self.out.set_color(Color::Red, RECT).unwrap();

        self.out.set_cursor_position(col, self.rows / 5).unwrap();
        self.out.write_fmt(format_args!("{TITLE}")).unwrap();

        self.out
            .set_cursor_position(col + TITLE.len() + 1, self.rows / 5)
            .unwrap();
        self.out.set_color(Color::Yellow, RECT).unwrap();
        self.out.write_fmt(format_args!("{PROJECT_TITLE}")).unwrap();

        self.out
            .set_cursor_position(col + TITLE.len() + PROJECT_TITLE.len() + 1, self.rows / 5)
            .unwrap();
        self.out.set_color(Color::Red, RECT).unwrap();
        self.out.write_fmt(format_args!("!")).unwrap();

        let col2 = self.cols.saturating_sub(SUBTITLE.len()) / 2;

        self.out
            .set_cursor_position(col2, self.rows / 5 + 1)
            .unwrap();
        self.out.write_fmt(format_args!("{SUBTITLE}")).unwrap();
    }

    fn center_line(&mut self, text: &str, row_index: usize) {
        let col = self.cols.saturating_sub(text.len()) / 2;
        let row = self.rows / 3;
        self.out.set_cursor_position(col, row + row_index).unwrap();
    }

    fn draw_rect(&mut self) {
        self.out.set_color(Color::Black, RECT).unwrap();
        let col_start = self.cols / 5;
        let col_end = self.cols - col_start;
        let width = col_end - col_start;

        let row_start = self.rows / 5;
        let row_end = self.rows - row_start;

        for r in row_start..row_end {
            self.out.set_cursor_position(col_start, r).unwrap();
            self.out.write_fmt(format_args!("{:width$}", "")).unwrap();
        }
    }
}
