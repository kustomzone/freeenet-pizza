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

/// Common ISO 4217 currencies for UI selection.
pub const COMMON_CURRENCIES: &[&iso::Currency] = &[
    iso::AED,
    iso::AFN,
    iso::ALL,
    iso::AMD,
    iso::AOA,
    iso::ARS,
    iso::AUD,
    iso::AWG,
    iso::AZN,
    iso::BAM,
    iso::BBD,
    iso::BDT,
    iso::BGN,
    iso::BHD,
    iso::BIF,
    iso::BMD,
    iso::BND,
    iso::BOB,
    iso::BRL,
    iso::BSD,
    iso::BTN,
    iso::BWP,
    iso::BYN,
    iso::BZD,
    iso::CAD,
    iso::CDF,
    iso::CHF,
    iso::CLF,
    iso::CLP,
    iso::CNY,
    iso::COP,
    iso::CRC,
    iso::CUP,
    iso::CVE,
    iso::CZK,
    iso::DJF,
    iso::DKK,
    iso::DOP,
    iso::DZD,
    iso::EGP,
    iso::ERN,
    iso::ETB,
    iso::EUR,
    iso::FJD,
    iso::FKP,
    iso::GBP,
    iso::GEL,
    iso::GHS,
    iso::GIP,
    iso::GMD,
    iso::GNF,
    iso::GTQ,
    iso::GYD,
    iso::HKD,
    iso::HNL,
    iso::HTG,
    iso::HUF,
    iso::IDR,
    iso::ILS,
    iso::INR,
    iso::IQD,
    iso::IRR,
    iso::ISK,
    iso::JMD,
    iso::JOD,
    iso::JPY,
    iso::KES,
    iso::KGS,
    iso::KHR,
    iso::KMF,
    iso::KPW,
    iso::KRW,
    iso::KWD,
    iso::KYD,
    iso::KZT,
    iso::LAK,
    iso::LBP,
    iso::LKR,
    iso::LRD,
    iso::LSL,
    iso::LYD,
    iso::MAD,
    iso::MDL,
    iso::MGA,
    iso::MKD,
    iso::MMK,
    iso::MNT,
    iso::MOP,
    iso::MRU,
    iso::MUR,
    iso::MVR,
    iso::MWK,
    iso::MXN,
    iso::MYR,
    iso::MZN,
    iso::NAD,
    iso::NGN,
    iso::NIO,
    iso::NOK,
    iso::NPR,
    iso::NZD,
    iso::OMR,
    iso::PAB,
    iso::PEN,
    iso::PGK,
    iso::PHP,
    iso::PKR,
    iso::PLN,
    iso::PYG,
    iso::QAR,
    iso::RON,
    iso::RSD,
    iso::RUB,
    iso::RWF,
    iso::SAR,
    iso::SBD,
    iso::SCR,
    iso::SDG,
    iso::SEK,
    iso::SGD,
    iso::SHP,
    iso::SLE,
    iso::SOS,
    iso::SRD,
    iso::SSP,
    iso::STN,
    iso::SVC,
    iso::SYP,
    iso::SZL,
    iso::THB,
    iso::TJS,
    iso::TMT,
    iso::TND,
    iso::TOP,
    iso::TRY,
    iso::TTD,
    iso::TWD,
    iso::TZS,
    iso::UAH,
    iso::UGX,
    iso::USD,
    iso::UYU,
    iso::UYW,
    iso::UZS,
    iso::VES,
    iso::VED,
    iso::VND,
    iso::VUV,
    iso::WST,
    iso::XAF,
    iso::XAG,
    iso::XAU,
    iso::XBA,
    iso::XBB,
    iso::XBC,
    iso::XBD,
    iso::XCD,
    iso::XCG,
    iso::XDR,
    iso::XOF,
    iso::XPD,
    iso::XPF,
    iso::XPT,
    iso::XTS,
    iso::YER,
    iso::ZAR,
    iso::ZMW,
    iso::ZWG,
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
        format!(
            "{}{}.{:0width$}",
            symbol,
            major,
            minor,
            width = exponent as usize
        )
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
