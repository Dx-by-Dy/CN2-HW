# Потоковое видео через RTSP/RTP (Rust)

Порт кода из `../VideoStreamingCode` на Rust. Места для выполнения задания помечены
`ДЛЯ ЗАПОЛНЕНИЯ`, как и в исходнике:

- `src/client.rs`: `send_rtsp_request`, `parse_rtsp_reply`, `open_rtp_port` (RTSP на клиенте);
- `src/rtp_packet.rs`: `encode` (RTP-пакетирование на сервере).

| Python             | Rust                                 |
|--------------------|--------------------------------------|
| `Server.py`        | `src/bin/server.rs`                  |
| `ServerWorker.py`  | `src/server_worker.rs`               |
| `ClientLauncher.py`| `src/bin/client_launcher.rs`         |
| `Client.py`        | `src/client.rs` (GUI на egui)        |
| `RtpPacket.py`     | `src/rtp_packet.rs`                  |
| `VideoStream.py`   | `src/video_stream.rs`                |

## Запуск

```sh
cargo run --release --bin server -- 8554
cargo run --release --bin client -- 127.0.0.1 8554 25000 ../VideoStreamingCode/movie.Mjpeg
```

Имя видеофайла сервер открывает относительно своего текущего каталога.
