// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    disable_dma_buf_renderer_on_nvidia();

    game_launcher_lib::run()
}

/// Обход известной несовместимости WebKitGTK с драйвером NVIDIA на Wayland:
/// веб-процесс не может создать GBM-буфер, соединение с композитором рвётся
/// и программа падает с «Error 71 (Protocol error) dispatching to Wayland
/// display». Отключаем ускоренный рендеринг — интерфейсу лаунчера он не нужен,
/// а окно после этого работает штатно в родном Wayland, без XWayland.
///
/// Переменную выставляем до того, как WebKit к ней обратится; в этот момент
/// поток в main один, поэтому работа с окружением безопасна. Если переменная
/// уже задана снаружи, ничего не трогаем — явная настройка важнее.
#[cfg(target_os = "linux")]
fn disable_dma_buf_renderer_on_nvidia() {
    const KEY: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";

    let nvidia_driver_loaded = std::path::Path::new("/dev/nvidia0").exists();
    if std::env::var_os(KEY).is_some() || !nvidia_driver_loaded {
        return;
    }

    unsafe { std::env::set_var(KEY, "1") };
}
