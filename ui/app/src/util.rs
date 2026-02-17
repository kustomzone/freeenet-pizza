use std::time::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = "
export function get_current_time() {
    return Date.now();
}
export function format_time_local(timestamp_ms) {
    const date = new Date(timestamp_ms);
    return date.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', hour12: false });
}
export function format_full_datetime_local(timestamp_ms) {
    const date = new Date(timestamp_ms);
    return date.toLocaleString(undefined, {
        weekday: 'short',
        year: 'numeric',
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
        hour12: false
    });
}
")]
extern "C" {
    fn get_current_time() -> f64;
    fn format_time_local(timestamp_ms: f64) -> String;
    fn format_full_datetime_local(timestamp_ms: f64) -> String;
}

pub fn get_current_system_time() -> SystemTime {
    #[cfg(target_arch = "wasm32")]
    {
        // Convert milliseconds since epoch to a Duration
        let millis = unsafe { get_current_time() };
        let duration_since_epoch = Duration::from_millis(millis as u64);
        UNIX_EPOCH + duration_since_epoch
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        SystemTime::now()
    }
}

/// Format a UTC timestamp as a local time string (HH:MM format)
pub fn format_utc_as_local_time(timestamp_ms: i64) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        unsafe { format_time_local(timestamp_ms as f64) }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use chrono::{Local, TimeZone, Utc};
        let utc_time = Utc.timestamp_millis_opt(timestamp_ms).unwrap();
        utc_time.with_timezone(&Local).format("%H:%M").to_string()
    }
}

/// Format a UTC timestamp as a full local datetime string for tooltips
pub fn format_utc_as_full_datetime(timestamp_ms: i64) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        unsafe { format_full_datetime_local(timestamp_ms as f64) }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use chrono::{Local, TimeZone, Utc};
        let utc_time = Utc.timestamp_millis_opt(timestamp_ms).unwrap();
        utc_time
            .with_timezone(&Local)
            .format("%a, %b %d, %Y %H:%M:%S")
            .to_string()
    }
}
