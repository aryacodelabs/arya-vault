//! The scrubbed diagnostics JSON (docs/14 §4.6). Built by hand from a closed set of numbers and
//! strings so that nothing else can slip in; no vault id, device id, name or item data.

use std::fmt::Write as _;

use arya_vault_session::HeaderInfo;

use crate::api::dto::InfoDto;

fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if u32::from(c) < 0x20 || c == '\u{7f}' => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

pub(crate) fn to_json(info: &InfoDto, header: Option<&HeaderInfo>) -> String {
    let mut o = String::new();
    let _ = write!(o, "{{\"apiVersion\":{},\"coreVersion\":", info.api_version);
    string(&mut o, &info.core_version);
    let _ = write!(
        o,
        ",\"formatVersion\":{},\"schemaVersion\":{}",
        info.format_version, info.schema_version
    );
    if let Some(h) = header {
        let _ = write!(
            o,
            ",\"header\":{{\"headerVersion\":{},\"epoch\":{},\"kdf\":{{\"mKib\":{},\"t\":{},\"p\":{}}}}}",
            h.header_version, h.epoch, h.kdf_m_kib, h.kdf_t, h.kdf_p
        );
    }
    o.push_str(",\"pinnedSettings\":{");
    for (i, s) in info.sqlcipher_settings.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        string(&mut o, &s.key);
        o.push(':');
        match &s.value {
            Some(v) => string(&mut o, v),
            None => o.push_str("null"),
        }
    }
    o.push_str("}}");
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::dto::PinnedSetting;

    #[test]
    fn json_escapes_and_has_no_ids() {
        let info = InfoDto {
            core_version: "1\"2".into(),
            api_version: 1,
            format_version: 1,
            schema_version: 2,
            sqlcipher_settings: vec![PinnedSetting {
                key: "k\n".into(),
                value: None,
            }],
        };
        let j = to_json(&info, None);
        assert_eq!(
            j,
            "{\"apiVersion\":1,\"coreVersion\":\"1\\\"2\",\"formatVersion\":1,\"schemaVersion\":2,\"pinnedSettings\":{\"k\\u000a\":null}}"
        );
    }
}
