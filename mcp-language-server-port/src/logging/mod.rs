use env_logger::Builder;
use log::{Level, LevelFilter};
use std::io::Write;

pub enum Component {
    Core,
    Lsp,
    Watcher,
    Transport,
    Tool,
}

impl Component {
    fn as_str(&self) -> &'static str {
        match self {
            Component::Core => "CORE",
            Component::Lsp => "LSP",
            Component::Watcher => "WATCHER",
            Component::Transport => "TRANSPORT",
            Component::Tool => "TOOL",
        }
    }
}

pub struct Logger {
    component: Component,
}

impl Logger {
    pub fn new(component: Component) -> Self {
        Self { component }
    }

    pub fn debug(&self, fmt: &str, args: impl std::fmt::Debug) {
        log::debug!("[{}] {} {:?}", self.component.as_str(), fmt, args);
    }

    pub fn info(&self, fmt: &str, args: impl std::fmt::Debug) {
        log::info!("[{}] {} {:?}", self.component.as_str(), fmt, args);
    }

    pub fn warn(&self, fmt: &str, args: impl std::fmt::Debug) {
        log::warn!("[{}] {} {:?}", self.component.as_str(), fmt, args);
    }

    pub fn error(&self, fmt: &str, args: impl std::fmt::Debug) {
        log::error!("[{}] {} {:?}", self.component.as_str(), fmt, args);
    }

    pub fn fatal(&self, fmt: &str, args: impl std::fmt::Debug) -> ! {
        log::error!("[{}] FATAL: {} {:?}", self.component.as_str(), fmt, args);
        std::process::exit(1);
    }
}

pub fn init() {
    init_with_level(Level::Info);
}

pub fn init_with_level(level: Level) {
    let mut builder = Builder::new();
    builder
        .format(|buf, record| {
            let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            writeln!(buf, "{} [{}] {}", ts, record.level(), record.args())
        })
        .filter(None, level.to_level_filter());

    if let Ok(var) = std::env::var("RUST_LOG") {
        builder.parse_filters(&var);
    }

    builder.init();
}

pub fn init_with_filter(filter: LevelFilter) {
    let mut builder = Builder::new();
    builder
        .format(|buf, record| {
            let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            writeln!(buf, "{} [{}] {}", ts, record.level(), record.args())
        })
        .filter(None, filter);

    if let Ok(var) = std::env::var("RUST_LOG") {
        builder.parse_filters(&var);
    }

    builder.init();
}
