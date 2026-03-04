use base64::{Engine as _, engine::general_purpose};

const OBFUSCATION_KEY: &[u8] = b"khushu-prayer-times-app-privacy-key";

pub fn obfuscate(data: &str) -> Result<String, String> {
    let input = data.as_bytes();
    let mut output = Vec::with_capacity(input.len());

    for (i, &byte) in input.iter().enumerate() {
        output.push(byte ^ OBFUSCATION_KEY[i % OBFUSCATION_KEY.len()]);
    }

    Ok(general_purpose::STANDARD.encode(output))
}

pub fn deobfuscate(encoded: &str) -> Result<String, String> {
    let decoded = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| e.to_string())?;
    let mut output = Vec::with_capacity(decoded.len());

    for (i, &byte) in decoded.iter().enumerate() {
        output.push(byte ^ OBFUSCATION_KEY[i % OBFUSCATION_KEY.len()]);
    }

    String::from_utf8(output).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let original = "36.7528";
        let encoded = obfuscate(original).expect("Obfuscation failed");
        assert_ne!(original, encoded);

        let decoded = deobfuscate(&encoded).expect("Deobfuscation failed");
        assert_eq!(original, decoded);
    }

    #[test]
    fn deterministic() {
        let data = "3.0588";
        let enc1 = obfuscate(data).unwrap();
        let enc2 = obfuscate(data).unwrap();
        assert_eq!(enc1, enc2);
    }
}
