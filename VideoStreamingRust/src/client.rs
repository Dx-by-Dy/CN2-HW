use std::collections::VecDeque;
use std::io::{self, Read};
use std::net::{Shutdown, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui;

use crate::event::Event;
use crate::rtp_packet::RtpPacket;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Init,
    Ready,
    Playing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    Setup,
    Play,
    Pause,
    Teardown,
}

/// Изменяемое состояние RTSP-сеанса.
pub struct RtspState {
    pub state: State,
    pub rtsp_seq: u32,
    pub session_id: u32,
    pub request_sent: Option<Request>,
    pub teardown_acked: bool,
}

/// Протокольная часть клиента: используется и из интерфейса, и из фоновых потоков.
pub struct ClientCore {
    pub server_addr: String,
    pub server_port: u16,
    pub rtp_port: u16,
    pub file_name: String,
    pub rtsp: Mutex<RtspState>,
    pub rtsp_socket: Option<TcpStream>,
    pub rtp_socket: Mutex<Option<Arc<UdpSocket>>>,
    pub play_event: Mutex<Arc<Event>>,
    pub frame_nbr: AtomicU16,
    /// Последний принятый кадр, который интерфейс еще не отобразил.
    frame: Mutex<Option<egui::ColorImage>>,
    /// Очередь предупреждений (аналог tkMessageBox.showwarning).
    warnings: Mutex<VecDeque<(String, String)>>,
    ctx: egui::Context,
}

impl ClientCore {
    pub fn new(
        ctx: egui::Context,
        server_addr: String,
        server_port: u16,
        rtp_port: u16,
        file_name: String,
    ) -> Arc<Self> {
        let mut core = Self {
            server_addr,
            server_port,
            rtp_port,
            file_name,
            rtsp: Mutex::new(RtspState {
                state: State::Init,
                rtsp_seq: 0,
                session_id: 0,
                request_sent: None,
                teardown_acked: false,
            }),
            rtsp_socket: None,
            rtp_socket: Mutex::new(None),
            play_event: Mutex::new(Arc::new(Event::new())),
            frame_nbr: AtomicU16::new(0),
            frame: Mutex::new(None),
            warnings: Mutex::new(VecDeque::new()),
            ctx,
        };
        core.connect_to_server();
        Arc::new(core)
    }

    pub fn state(&self) -> State {
        self.rtsp.lock().unwrap().state
    }

    /// Соединяемся с сервером. Запускаем новый сеанс RTSP/TCP.
    fn connect_to_server(&mut self) {
        match TcpStream::connect((self.server_addr.as_str(), self.server_port)) {
            Ok(stream) => self.rtsp_socket = Some(stream),
            Err(_) => self.show_warning(
                "Соединение невозможно",
                format!("Соединение с '{}' невозможно.", self.server_addr),
            ),
        }
    }

    /// Слушаем RTP-пакеты.
    fn listen_rtp(&self, play_event: Arc<Event>) {
        let Some(rtp_socket) = self.rtp_socket.lock().unwrap().clone() else {
            return;
        };
        let mut buf = vec![0u8; 20480];
        loop {
            match rtp_socket.recv(&mut buf) {
                Ok(n) if n > 0 => {
                    let mut rtp_packet = RtpPacket::new();
                    rtp_packet.decode(&buf[..n]);

                    let curr_frame_nbr = rtp_packet.seq_num();
                    println!("Текущий номер последовательности: {curr_frame_nbr}");

                    // Отбрасываем опоздавшие пакеты
                    if curr_frame_nbr > self.frame_nbr.load(Ordering::Relaxed) {
                        self.frame_nbr.store(curr_frame_nbr, Ordering::Relaxed);
                        self.update_movie(rtp_packet.get_payload());
                    }
                }
                _ => {
                    // Прекращаем слушать по запросу PAUSE или TEARDOWN
                    if play_event.is_set() {
                        break;
                    }

                    // Закрываем RTP-сокет при получении подтверждения TEARDOWN
                    if self.rtsp.lock().unwrap().teardown_acked {
                        *self.rtp_socket.lock().unwrap() = None;
                        break;
                    }
                }
            }
        }
    }

    /// Декодируем полученный JPEG-кадр и передаем его интерфейсу.
    fn update_movie(&self, data: &[u8]) {
        let Ok(img) = image::load_from_memory_with_format(data, image::ImageFormat::Jpeg) else {
            println!("Не удалось декодировать кадр");
            return;
        };
        let img = img.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
        *self.frame.lock().unwrap() = Some(color_image);
        self.ctx.request_repaint();
    }

    /// Отправляем RTSP-запрос серверу.
    #[allow(
        unreachable_code,
        unused_variables,
        unused_mut,
        clippy::diverging_sub_expression
    )] // убрать после заполнения
    pub fn send_rtsp_request(self: &Arc<Self>, request_code: Request) {
        let mut s = self.rtsp.lock().unwrap();
        let request: String;
        //---------------
        // ДЛЯ ЗАПОЛНЕНИЯ
        //---------------

        // Setup-запрос
        if request_code == Request::Setup && s.state == State::Init {
            let core = Arc::clone(self);
            thread::spawn(move || core.recv_rtsp_reply());
            // Обновляем номер последовательности RTSP.
            // ...

            // Записываем RTSP-запрос для отправки.
            request = todo!("request = ...");

            // Отслеживаем отправленный запрос.
            // s.request_sent = ...
        }
        // Play-запрос
        else if request_code == Request::Play && s.state == State::Ready {
            // Обновляем номер последовательности RTSP.
            // ...

            // Записываем RTSP-запрос для отправки.
            request = todo!("request = ...");

            // Отслеживаем отправленный запрос.
            // s.request_sent = ...
        }
        // Pause-запрос
        else if request_code == Request::Pause && s.state == State::Playing {
            // Обновляем номер последовательности RTSP.
            // ...

            // Записываем RTSP-запрос для отправки.
            request = todo!("request = ...");

            // Отслеживаем отправленный запрос.
            // s.request_sent = ...
        }
        // Teardown-запрос
        else if request_code == Request::Teardown && s.state != State::Init {
            // Обновляем номер последовательности RTSP.
            // ...

            // Записываем RTSP-запрос для отправки.
            request = todo!("request = ...");

            // Отслеживаем отправленный запрос.
            // s.request_sent = ...
        } else {
            return;
        }

        // Отправляем RTSP-запрос, используя rtsp_socket.
        // (self.rtsp_socket: Option<TcpStream>; для &TcpStream реализован Write)
        // ...

        println!("\nData sent:\n{request}");
    }

    /// Получаем RTSP-ответ от сервера.
    fn recv_rtsp_reply(&self) {
        let Some(rtsp_socket) = &self.rtsp_socket else {
            return;
        };
        let mut buf = [0u8; 1024];
        loop {
            let reply = match (&*rtsp_socket).read(&mut buf) {
                Ok(0) | Err(_) => break, // сервер закрыл соединение
                Ok(n) => String::from_utf8_lossy(&buf[..n]).into_owned(),
            };
            self.parse_rtsp_reply(&reply);

            // Закрываем RTSP-сокет по Teardown-запросу
            if self.rtsp.lock().unwrap().request_sent == Some(Request::Teardown) {
                let _ = rtsp_socket.shutdown(Shutdown::Both);
                break;
            }
        }
    }

    /// Обрабатываем RTSP-ответ от сервера.
    fn parse_rtsp_reply(&self, data: &str) {
        let lines: Vec<&str> = data.split('\n').collect();
        let field =
            |i: usize| -> Option<u32> { lines.get(i)?.split(' ').nth(1)?.trim().parse().ok() };
        let Some(seq_num) = field(1) else {
            return;
        };

        let mut s = self.rtsp.lock().unwrap();
        // Обрабатываем только в случае совпадения номеров последовательности запроса и ответа
        if seq_num == s.rtsp_seq {
            let Some(session) = field(2) else {
                return;
            };
            // Идентификатор нового сеанса
            if s.session_id == 0 {
                s.session_id = session;
            }

            // Обрабатываем, только если идентификаторы сеанса совпадают
            if s.session_id == session && field(0) == Some(200) {
                match s.request_sent {
                    Some(Request::Setup) => {
                        //---------------
                        // ДЛЯ ЗАПОЛНЕНИЯ
                        //---------------
                        // Обновляем состояние RTSP-сеанса.
                        // s.state = ...

                        // Открываем RTP-порт.
                        self.open_rtp_port();
                    }
                    Some(Request::Play) => {
                        // s.state = ...
                    }
                    Some(Request::Pause) => {
                        // s.state = ...

                        // Поток воспроизведения уже существует. Создаем новый при возобновлении воспроизведения.
                        self.play_event.lock().unwrap().set();
                    }
                    Some(Request::Teardown) => {
                        // s.state = ...

                        // Устанавливаем флаг ACK для закрытия сокета.
                        s.teardown_acked = true;
                    }
                    None => {}
                }
            }
        }
    }

    /// Открываем RTP-сокет, привязанный к указанному порту.
    #[allow(
        unreachable_code,
        unused_variables,
        unused_mut,
        clippy::diverging_sub_expression
    )] // убрать после заполнения
    fn open_rtp_port(&self) {
        //---------------
        // ДЛЯ ЗАПОЛНЕНИЯ
        //---------------
        // Создаем новый датаграммный сокет для получения RTP-пакетов от сервера
        // и связываем его с адресом, используя RTP-порт, указанный клиентом
        // (в Rust UDP-сокет создается сразу привязанным: UdpSocket::bind).
        let bound: io::Result<UdpSocket> = todo!("UdpSocket::bind(...)");

        match bound {
            Ok(mut rtp_socket) => {
                // Устанавливаем значение тайм-аута сокета в 0.5 сек
                // ...

                *self.rtp_socket.lock().unwrap() = Some(Arc::new(rtp_socket));
            }
            Err(_) => self.show_warning(
                "Невозможно привязать",
                format!("Невозможно привязать сокет к порту={}", self.rtp_port),
            ),
        }
    }

    fn show_warning(&self, title: &str, text: String) {
        self.warnings
            .lock()
            .unwrap()
            .push_back((title.to_owned(), text));
        self.ctx.request_repaint();
    }
}

/// Окно клиента.
pub struct Client {
    core: Arc<ClientCore>,
    texture: Option<egui::TextureHandle>,
    exit_dialog: bool,
    exiting: bool,
}

impl Client {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        server_addr: String,
        server_port: u16,
        rtp_port: u16,
        file_name: String,
    ) -> Self {
        Self {
            core: ClientCore::new(
                cc.egui_ctx.clone(),
                server_addr,
                server_port,
                rtp_port,
                file_name,
            ),
            texture: None,
            exit_dialog: false,
            exiting: false,
        }
    }

    /// Обработчик кнопки Setup.
    fn setup_movie(&mut self) {
        if self.core.state() == State::Init {
            self.core.send_rtsp_request(Request::Setup);
        }
    }

    /// Обработчик кнопки Teardown.
    fn exit_client(&mut self, ctx: &egui::Context) {
        self.core.send_rtsp_request(Request::Teardown);
        self.exiting = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close); // Закрываем окно интерфейса
    }

    /// Обработчик кнопки Pause.
    fn pause_movie(&mut self) {
        if self.core.state() == State::Playing {
            self.core.send_rtsp_request(Request::Pause);
        }
    }

    /// Обработчик кнопки Play.
    fn play_movie(&mut self) {
        if self.core.state() == State::Ready {
            // Создаем новый поток, чтобы слушать RTP-пакеты
            let play_event = Arc::new(Event::new());
            *self.core.play_event.lock().unwrap() = Arc::clone(&play_event);
            let core = Arc::clone(&self.core);
            thread::spawn(move || core.listen_rtp(play_event));
            self.core.send_rtsp_request(Request::Play);
        }
    }

    /// Обработчик закрытия окна интерфейса.
    fn handler(&mut self) {
        self.pause_movie();
        self.exit_dialog = true;
    }

    /// Построение интерфейса пользователя.
    fn create_widgets(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        egui::Panel::bottom("buttons").show(ui, |ui| {
            ui.columns(4, |cols| {
                let button = |ui: &mut egui::Ui, text: &str| {
                    ui.add_sized([ui.available_width(), 28.0], egui::Button::new(text))
                        .clicked()
                };
                // Создаем кнопку Setup (Настройки)
                if button(&mut cols[0], "Setup") {
                    self.setup_movie();
                }
                // Создаем кнопку Play (Воспроизведение)
                if button(&mut cols[1], "Play") {
                    self.play_movie();
                }
                // Создаем кнопку Pause (Пауза)
                if button(&mut cols[2], "Pause") {
                    self.pause_movie();
                }
                // Создаем кнопку Teardown (Выход)
                if button(&mut cols[3], "Teardown") {
                    self.exit_client(&ctx);
                }
            });
        });

        // Область для отображения видео
        egui::CentralPanel::default_margins().show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                if let Some(texture) = &self.texture {
                    ui.add(egui::Image::new(texture).shrink_to_fit());
                }
            });
        });
    }
}

impl eframe::App for Client {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        // Забираем новый кадр, если он пришел
        if let Some(image) = self.core.frame.lock().unwrap().take() {
            match &mut self.texture {
                Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
                None => {
                    self.texture =
                        Some(ctx.load_texture("frame", image, egui::TextureOptions::LINEAR))
                }
            }
        }

        // Перехватываем закрытие окна
        if ctx.input(|i| i.viewport().close_requested()) && !self.exiting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.handler();
        }

        self.create_widgets(ui);

        if self.exit_dialog {
            egui::Modal::new(egui::Id::unique("exit_dialog")).show(ctx, |ui| {
                ui.heading("Выход?");
                ui.label("Вы уверены, что хотите выйти из приложения?");
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        self.exit_dialog = false;
                        self.exit_client(ctx);
                    }
                    // Возобновляем воспроизведение при нажатии кнопки Cancel.
                    if ui.button("Cancel").clicked() {
                        self.exit_dialog = false;
                        self.play_movie();
                    }
                });
            });
        }

        let warning = self.core.warnings.lock().unwrap().front().cloned();
        if let Some((title, text)) = warning {
            egui::Modal::new(egui::Id::unique("warning")).show(ctx, |ui| {
                ui.heading(title);
                ui.label(text);
                if ui.button("OK").clicked() {
                    self.core.warnings.lock().unwrap().pop_front();
                }
            });
        }
    }
}
