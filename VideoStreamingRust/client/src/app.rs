use std::sync::Arc;

use eframe::egui;

use crate::config::Config;
use crate::rtsp_client::RtspClient;
use crate::ui_handle::UiHandle;

/// Окно клиента: кнопки управления и область видео.
pub struct ClientApp {
    rtsp: Arc<RtspClient>,
    ui: Arc<UiHandle>,
    texture: Option<egui::TextureHandle>,
    exit_dialog: bool,
    exiting: bool,
}

impl ClientApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        let ui = Arc::new(UiHandle::new(cc.egui_ctx.clone()));
        Self {
            rtsp: RtspClient::connect(&config, Arc::clone(&ui)),
            ui,
            texture: None,
            exit_dialog: false,
            exiting: false,
        }
    }

    /// Teardown: завершаем сеанс и закрываем окно.
    fn exit(&mut self, ctx: &egui::Context) {
        self.rtsp.teardown();
        self.exiting = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn update_texture(&mut self, ctx: &egui::Context) {
        let Some(frame) = self.ui.take_frame() else {
            return;
        };
        match &mut self.texture {
            Some(texture) => texture.set(frame, egui::TextureOptions::LINEAR),
            None => {
                self.texture = Some(ctx.load_texture("frame", frame, egui::TextureOptions::LINEAR))
            }
        }
    }

    /// Закрытие окна: ставим на паузу и спрашиваем подтверждение.
    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.exiting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.rtsp.pause();
            self.exit_dialog = true;
        }
    }

    fn buttons_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::Panel::bottom("buttons").show(ui, |ui| {
            ui.columns(4, |cols| {
                let button = |ui: &mut egui::Ui, text: &str| {
                    ui.add_sized([ui.available_width(), 28.0], egui::Button::new(text))
                        .clicked()
                };
                if button(&mut cols[0], "Setup") {
                    self.rtsp.setup();
                }
                if button(&mut cols[1], "Play") {
                    self.rtsp.play();
                }
                if button(&mut cols[2], "Pause") {
                    self.rtsp.pause();
                }
                if button(&mut cols[3], "Teardown") {
                    self.exit(ctx);
                }
            });
        });
    }

    fn video_panel(&self, ui: &mut egui::Ui) {
        egui::CentralPanel::default_margins().show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                if let Some(texture) = &self.texture {
                    ui.add(egui::Image::new(texture).shrink_to_fit());
                }
            });
        });
    }

    fn exit_dialog(&mut self, ctx: &egui::Context) {
        if !self.exit_dialog {
            return;
        }
        egui::Modal::new(egui::Id::unique("exit_dialog")).show(ctx, |ui| {
            ui.heading("Выход?");
            ui.label("Вы уверены, что хотите выйти из приложения?");
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    self.exit_dialog = false;
                    self.exit(ctx);
                }
                // Cancel — продолжаем воспроизведение
                if ui.button("Cancel").clicked() {
                    self.exit_dialog = false;
                    self.rtsp.play();
                }
            });
        });
    }

    fn warning_dialog(&self, ctx: &egui::Context) {
        let Some(warning) = self.ui.current_warning() else {
            return;
        };
        egui::Modal::new(egui::Id::unique("warning")).show(ctx, |ui| {
            ui.heading(warning.title);
            ui.label(warning.text);
            if ui.button("OK").clicked() {
                self.ui.dismiss_warning();
            }
        });
    }
}

impl eframe::App for ClientApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.update_texture(&ctx);
        self.handle_close_request(&ctx);
        self.buttons_panel(ui, &ctx);
        self.video_panel(ui);
        self.exit_dialog(&ctx);
        self.warning_dialog(&ctx);
    }
}
