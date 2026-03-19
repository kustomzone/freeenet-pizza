use std::time::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// Access the build timestamp (ISO 8601 format) environment variable set by build.rs
const BUILD_TIMESTAMP_ISO: &str = env!("BUILD_TIMESTAMP_ISO", "Build timestamp not set");

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

/// Convert UTC ISO timestamp to local time string for build date display
pub fn format_build_time_local() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        use js_sys::Date;
        let date = Date::new(&wasm_bindgen::JsValue::from_str(BUILD_TIMESTAMP_ISO));
        if date.to_string().as_string().is_some() {
            // Format as "YYYY-MM-DD HH:MM" in local time
            let year = date.get_full_year();
            let month = date.get_month() + 1; // JS months are 0-indexed
            let day = date.get_date();
            let hours = date.get_hours();
            let minutes = date.get_minutes();
            let offset_min = date.get_timezone_offset() as i32;
            let tz_str = if offset_min == 0 {
                "UTC".to_string()
            } else {
                // getTimezoneOffset returns minutes west of UTC, so negate
                let sign = if offset_min <= 0 { '+' } else { '-' };
                let abs = offset_min.unsigned_abs();
                format!("UTC{}{}", sign, abs / 60)
            };
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02} {}",
                year, month, day, hours, minutes, tz_str
            )
        } else {
            BUILD_TIMESTAMP_ISO.to_string()
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        BUILD_TIMESTAMP_ISO.to_string()
    }
}
