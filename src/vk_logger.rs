pub struct VkLogger;

impl VkLogger {
    pub fn init(level: log::LevelFilter) {
        static VK_LOGGER: VkLogger = VkLogger;
        log::set_logger(&VK_LOGGER).unwrap();
        log::set_max_level(level);
    }
}

impl log::Log for VkLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        println!("[{}]: {}", record.level(), record.args());
    }

    fn flush(&self) {
        todo!()
    }
}