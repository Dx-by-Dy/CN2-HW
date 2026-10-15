use std::collections::VecDeque;
use std::sync::Mutex;

use eframe::egui;

use crate::warning::Warning;

/// Через него фоновые потоки передают интерфейсу кадры и предупреждения.
pub struct UiHandle {
    ctx: egui::Context,
    /// Последний принятый кадр, который интерфейс еще не отобразил.
    frame: Mutex<Option<egui::ColorImage>>,
    warnings: Mutex<VecDeque<Warning>>,
}

impl UiHandle {
    pub fn new(ctx: egui::Context) -> Self {
        Self {
            ctx,
            frame: Mutex::new(None),
            warnings: Mutex::new(VecDeque::new()),
        }
    }

    pub fn show_frame(&self, frame: egui::ColorImage) {
        *self.frame.lock().unwrap() = Some(frame);
        self.ctx.request_repaint();
    }

    pub fn take_frame(&self) -> Option<egui::ColorImage> {
        self.frame.lock().unwrap().take()
    }

    pub fn show_warning(&self, title: &str, text: String) {
        self.warnings.lock().unwrap().push_back(Warning {
            title: title.to_owned(),
            text,
        });
        self.ctx.request_repaint();
    }

    pub fn current_warning(&self) -> Option<Warning> {
        self.warnings.lock().unwrap().front().cloned()
    }

    pub fn dismiss_warning(&self) {
        self.warnings.lock().unwrap().pop_front();
    }
}
