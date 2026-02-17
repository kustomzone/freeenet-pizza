use base64::{engine::general_purpose, Engine as _};
use data_encoding::BASE32;
use ed25519_dalek::{Signature, SignatureError, Signer, SigningKey, Verifier, VerifyingKey};
use rusty_money::iso;
use serde::Serialize;

pub fn sign_struct<T: Serialize>(message: T, signing_key: &SigningKey) -> Signature {
    let mut data_to_sign = Vec::new();
    ciborium::ser::into_writer(&message, &mut data_to_sign).expect("Serialization should not fail");
    signing_key.sign(&data_to_sign)
}

pub fn verify_struct<T: Serialize>(
    message: &T,
    signature: &Signature,
    verifying_key: &VerifyingKey,
) -> Result<(), SignatureError> {
    let mut data_to_sign = Vec::new();
    ciborium::ser::into_writer(message, &mut data_to_sign).expect("Serialization should not fail");
    verifying_key.verify(&data_to_sign, signature)
}

pub fn truncated_base64<T: AsRef<[u8]>>(data: T) -> String {
    let encoded = general_purpose::STANDARD_NO_PAD.encode(data);
    encoded.chars().take(10).collect()
}

pub fn truncated_base32(bytes: &[u8]) -> String {
    let encoded = BASE32.encode(bytes);
    encoded.chars().take(8).collect()
}

/// Validates that a currency code is a valid ISO 4217 currency code.
/// Returns the currency if valid, or an error message if invalid.
pub fn validate_currency(code: &str) -> Result<&'static iso::Currency, String> {
    iso::find(code).ok_or_else(|| format!("Invalid ISO 4217 currency code: {}", code))
}

/// Common ISO 4217 currency codes for UI selection.
/// Each entry is (code, name, symbol).
pub const COMMON_CURRENCIES: &[(&str, &str, &str)] = &[
    ("USD", "US Dollar", "$"),
    ("EUR", "Euro", "€"),
    ("GBP", "British Pound", "£"),
    ("JPY", "Japanese Yen", "¥"),
    ("CHF", "Swiss Franc", "Fr"),
    ("CAD", "Canadian Dollar", "CA$"),
    ("AUD", "Australian Dollar", "A$"),
    ("CNY", "Chinese Yuan", "¥"),
    ("INR", "Indian Rupee", "₹"),
    ("MXN", "Mexican Peso", "MX$"),
    ("BRL", "Brazilian Real", "R$"),
    ("KRW", "South Korean Won", "₩"),
    ("SGD", "Singapore Dollar", "S$"),
    ("HKD", "Hong Kong Dollar", "HK$"),
    ("NOK", "Norwegian Krone", "kr"),
    ("SEK", "Swedish Krona", "kr"),
    ("DKK", "Danish Krone", "kr"),
    ("NZD", "New Zealand Dollar", "NZ$"),
    ("ZAR", "South African Rand", "R"),
    ("PLN", "Polish Zloty", "zł"),
];

/// Formats a price in the smallest currency unit (e.g., cents) using the specified ISO currency.
/// Falls back to USD formatting if the currency code is invalid.
pub fn format_price_with_currency(minor_units: u64, currency_code: &str) -> String {
    let currency = iso::find(currency_code).unwrap_or(iso::USD);
    let exponent = currency.exponent as u32;
    let divisor = 10u64.pow(exponent);
    let major = minor_units / divisor;
    let minor = minor_units % divisor;

    // Format with currency symbol
    let symbol = currency.symbol;
    if exponent > 0 {
        format!("{}{}.{:0width$}", symbol, major, minor, width = exponent as usize)
    } else {
        format!("{}{}", symbol, major)
    }
}

/// Formats a price in cents as USD (legacy function for backwards compatibility).
pub fn format_price(cents: u64) -> String {
    format_price_with_currency(cents, "USD")
}

pub fn parse_price(input: &str) -> Result<u64, String> {
    let clean: String = input
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        .collect();
    if clean.is_empty() {
        return Err("Price cannot be empty".to_string());
    }

    let split_at = if clean.contains(',') { ',' } else { '.' };

    if let Some((dollars, cents)) = clean.split_once(split_at) {
        let d: u64 = if dollars.is_empty() {
            0
        } else {
            dollars
                .parse()
                .map_err(|_| "Invalid dollar amount".to_string())?
        };
        let mut c_str = cents.to_string();
        c_str.push_str("00");
        let c: u64 = c_str[..2]
            .parse()
            .map_err(|_| "Invalid cents amount".to_string())?;
        Ok(d * 100 + c)
    } else {
        let d: u64 = clean
            .parse()
            .map_err(|_| "Invalid price format".to_string())?;
        Ok(d * 100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn test_sign_verify_struct() {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();

        let message = "Hello, World!";
        let signature = sign_struct(message, &signing_key);
        assert!(verify_struct(&message, &signature, &verifying_key).is_ok());
    }
}
