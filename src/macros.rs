#[macro_export]
macro_rules! room_log {
    ($room:expr, $($arg:tt)*) => {
        log::log!(target: &$room.log_target(), $($arg)*);
    };
}

#[macro_export]
macro_rules! room_error {
    ($room:expr, $($arg:tt)*) => {
        room_log!($room, log::Level::Error, $($arg)*);
    };
}

#[macro_export]
macro_rules! room_warn {
    ($room:expr, $($arg:tt)*) => {
        room_log!($room, log::Level::Warn, $($arg)*);
    };
}

#[macro_export]
macro_rules! room_info {
    ($room:expr, $($arg:tt)*) => {
        room_log!($room, log::Level::Info, $($arg)*);
    };
}

#[macro_export]
macro_rules! room_debug {
    ($room:expr, $($arg:tt)*) => {
        room_log!($room, log::Level::Debug, $($arg)*);
    };
}

#[macro_export]
macro_rules! room_trace {
    ($room:expr, $($arg:tt)*) => {
        room_log!($room, log::Level::Trace, $($arg)*);
    };
}
