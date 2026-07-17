use core::fmt::Write;
use uefi::proto::console::text::{Color, Output};

use crate::environment;

const RECT: Color = Color::LightGray;
const TITLE: &str = "Welcome to";
const SUBTITLE: &str = "Please select the OS you wish to install.";
const PROJECT_TITLE: &str = "rsboot";

pub struct Tui<'a> {
    out: &'a mut Output,
    cols: usize,
    rows: usize,
}

impl<'a> Tui<'a> {
    pub fn new(out: &'a mut Output) -> Self {
        let (cols, rows) = Tui::get_dimensions(out);
        Self { out, cols, rows }
    }

    pub fn clear_screen(&mut self) {
        self.out.set_color(Color::LightGray, Color::Black).ok(); // set colors to default
        self.out.clear().ok();
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
                if let Some(&item) = items.get(selected) {
                    self.out.set_color(Color::Magenta, RECT).ok();

                    self.center_line(item, 0);

                    self.out.write_fmt(format_args!("{}", item)).ok();
                }
            }
        }
    }

    fn draw_item(&mut self, items: &[&str], i: usize, selected: usize) {
        let Some(&item) = items.get(i) else { return };
        if i == selected {
            self.out.set_color(Color::Magenta, RECT).ok();
        } else {
            self.out.set_color(Color::Black, RECT).ok();
        }
        self.center_line(item, i);
        self.out.write_fmt(format_args!("{}", item)).ok();
    }

    // helper function to overwrite only the 2 affected selections
    pub fn update_selection(&mut self, items: &[&str], old: usize, new: usize) {
        self.draw_item(items, old, new);
        self.draw_item(items, new, new);
    }

    fn get_dimensions(out: &Output) -> (usize, usize) {
        out.current_mode()
            .ok()
            .flatten()
            .map(|m| (m.columns(), m.rows()))
            .unwrap_or((80, 25))
    }

    fn draw_title(&mut self) {
        let col = self
            .cols
            .saturating_sub(TITLE.len() + PROJECT_TITLE.len() + 2)
            / 2; // + 2 pentru spatii!"
        self.out.set_color(Color::Red, RECT).ok();

        self.out.set_cursor_position(col, self.rows / 5).ok();
        self.out.write_fmt(format_args!("{TITLE}")).ok();

        self.out
            .set_cursor_position(col + TITLE.len() + 1, self.rows / 5)
            .ok();
        self.out.set_color(Color::Yellow, RECT).ok();
        self.out.write_fmt(format_args!("{PROJECT_TITLE}")).ok();

        self.out
            .set_cursor_position(col + TITLE.len() + PROJECT_TITLE.len() + 1, self.rows / 5)
            .ok();
        self.out.set_color(Color::Red, RECT).ok();
        self.out.write_fmt(format_args!("!")).ok();

        let col2 = self.cols.saturating_sub(SUBTITLE.len()) / 2;

        self.out.set_cursor_position(col2, self.rows / 5 + 1).ok();
        self.out.write_fmt(format_args!("{SUBTITLE}")).ok();
    }

    fn center_line(&mut self, text: &str, row_index: usize) {
        let col = self.cols.saturating_sub(text.len()) / 2;
        let row = self.rows / 3;
        self.out.set_cursor_position(col, row + row_index).ok();
    }

    fn draw_rect(&mut self) {
        self.out.set_color(Color::Black, RECT).ok();
        let col_start = self.cols / 5;
        let col_end = self.cols.saturating_sub(col_start);
        let width = col_end.saturating_sub(col_start);

        let row_start = self.rows / 5;
        let row_end = self.rows.saturating_sub(row_start);

        for r in row_start..row_end {
            self.out.set_cursor_position(col_start, r).ok();
            self.out.write_fmt(format_args!("{:width$}", "")).ok();
        }
    }
}
