//! The transcribing page. It is the animation, or a plain display of the same
//! stage, progress and lines for when the animation's per-frame repaint is too
//! much for the desktop.

use std::cell::RefCell;

use gtk::glib;
use gtk::prelude::*;

use crate::animation::TranscribeAnimation;

/// Transcript lines the plain display keeps; older ones scroll off its top.
const LINES: usize = 12;

pub struct Transcribing {
    animation: TranscribeAnimation,
    animated: bool,
    page: gtk::Widget,
    stage: gtk::Label,
    bar: gtk::ProgressBar,
    text: gtk::Label,
    scroll: gtk::ScrolledWindow,
    lines: RefCell<Vec<String>>,
}

impl Transcribing {
    /// With `plain`, the page is the plain display and the animation's frame
    /// clock is never started, so nothing on it is ever drawn.
    pub fn new(plain: bool) -> Self {
        let animation = TranscribeAnimation::new();
        let stage = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .css_classes(["title-3"])
            .build();
        let bar = gtk::ProgressBar::builder().show_text(true).build();
        let text = gtk::Label::builder()
            .xalign(0.0)
            .yalign(0.0)
            .wrap(true)
            .selectable(true)
            .build();
        let scroll = gtk::ScrolledWindow::builder()
            .child(&text)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        let plain_page = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(24)
            .margin_end(24)
            .build();
        plain_page.append(&stage);
        plain_page.append(&bar);
        plain_page.append(&scroll);
        let page = if plain {
            plain_page.upcast()
        } else {
            animation.widget().clone().upcast()
        };
        Transcribing {
            animation,
            animated: !plain,
            page,
            stage,
            bar,
            text,
            scroll,
            lines: RefCell::default(),
        }
    }

    /// Whether the animation is what the page is showing.
    pub fn animates(&self) -> bool {
        self.animated
    }

    pub fn page(&self) -> &gtk::Widget {
        &self.page
    }

    pub fn set_progress(&self, progress: f64) {
        self.animation.set_progress(progress);
        // Determinate: the fraction is the real progress, not a pulse.
        self.bar.set_fraction(progress.clamp(0.0, 1.0));
    }

    pub fn set_stage(&self, stage: &str) {
        self.animation.set_stage(stage);
        self.stage.set_text(stage);
    }

    pub fn push_text(&self, line: &str) {
        self.animation.push_text(line);
        let added = push_line(&mut self.lines.borrow_mut(), line);
        if added {
            self.text.set_label(&self.lines.borrow().join("\n"));
            self.scroll_to_end();
        }
    }

    pub fn reset(&self) {
        self.animation.reset();
        self.lines.borrow_mut().clear();
        self.text.set_label("");
        self.stage.set_text("");
        self.set_progress(0.0);
    }

    /// The animation's frame clock is only ever started through here, and not
    /// at all on the plain page: that is what keeps the draw callback off.
    pub fn set_running(&self, running: bool) {
        if self.animated {
            self.animation.set_running(running);
        }
    }

    /// The label has laid the new line out only after this returns, so the
    /// scroll has to wait for the next idle before it knows how far down is.
    fn scroll_to_end(&self) {
        let adjustment = self.scroll.vadjustment();
        glib::idle_add_local_once(move || {
            adjustment.set_value(adjustment.upper() - adjustment.page_size());
        });
    }
}

/// Adds a trimmed line, dropping the oldest once there are `LINES` of them.
/// Says whether the line was one worth showing.
fn push_line(lines: &mut Vec<String>, line: &str) -> bool {
    let line = line.trim();
    if line.is_empty() {
        return false;
    }
    lines.push(line.to_owned());
    let older = lines.len().saturating_sub(LINES);
    lines.drain(..older);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_line_is_skipped_and_a_line_is_trimmed() {
        let mut lines = Vec::new();
        assert!(!push_line(&mut lines, "   "));
        assert!(lines.is_empty());
        assert!(push_line(&mut lines, "  You: hello  "));
        assert_eq!(lines, ["You: hello"]);
    }

    #[test]
    fn the_oldest_lines_scroll_off_the_top() {
        let mut lines = Vec::new();
        for i in 0..LINES + 3 {
            assert!(push_line(&mut lines, &format!("  line {i}  ")));
        }
        assert_eq!(lines.len(), LINES);
        assert_eq!(lines.first().map(String::as_str), Some("line 3"));
        assert_eq!(lines.last().map(String::as_str), Some("line 14"));
    }
}
