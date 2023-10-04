use std::env;
use std::str::FromStr;

pub fn get_env_or_default<T>(key: &str, default: T) -> T
where
    T: FromStr,
{
    match env::var(key) {
        Ok(value) => match value.parse::<T>() {
            Ok(value) => value,
            Err(_) => default,
        },
        Err(_) => default,
    }
}

pub fn get_env_or_panic<T>(key: &str) -> T
where
    T: FromStr,
{
    match env::var(key) {
        Ok(value) => match value.parse::<T>() {
            Ok(value) => value,
            Err(_) => panic!("Could not parse environment variable: {}", key),
        },
        Err(_) => panic!("Environment variable not found: {}", key),
    }
}

#[macro_export]
macro_rules! log_identifier {
    () => {
        format!("{:05}", rand::random::<u16>())
    };
}

#[macro_export]
macro_rules! create_error_logger {
    ($target:expr, $identifier:expr, $message:literal) => {
        |e| {
            log::debug!(target: $target, "{} | {}: {:?}", $identifier, $message, e);
            e
        }
    };
}
