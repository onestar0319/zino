use crate::SharedString;
use http::header::{self, HeaderMap};

/// Extension trait for [`HeaderMap`](http::HeaderMap).
pub trait HeaderMapExt {
    /// Extracts the string corresponding to the key.
    fn get_str(&self, key: &str) -> Option<&str>;

    /// Extracts the essence of the `content-type` header, discarding the optional parameters.
    fn get_content_type(&self) -> Option<&str>;

    /// Gets the data type by parsing the `content-type` header.
    fn get_data_type(&self) -> Option<SharedString>;

    /// Checks whether it has a `content-type: application/json` or similar header.
    fn has_json_content_type(&self) -> bool;

    /// Selects a language from the supported locales by parsing and comparing
    /// the `accept-language` header.
    fn select_language<'a>(&'a self, supported_locales: &[&'a str]) -> Option<&'a str>;
}

impl HeaderMapExt for HeaderMap {
    #[inline]
    fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.to_str().ok())
    }

    fn get_content_type(&self) -> Option<&str> {
        self.get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|content_type| {
                if let Some((essence, _)) = content_type.split_once(';') {
                    essence
                } else {
                    content_type
                }
            })
    }

    fn get_data_type(&self) -> Option<SharedString> {
        self.get_content_type()
            .map(|content_type| match content_type {
                "application/json" => "json".into(),
                "application/octet-stream" => "bytes".into(),
                "application/x-www-form-urlencoded" => "form".into(),
                "multipart/form-data" => "multipart".into(),
                "text/plain" => "text".into(),
                _ => {
                    if content_type.starts_with("application/") && content_type.ends_with("+json") {
                        "json".into()
                    } else {
                        content_type.to_owned().into()
                    }
                }
            })
    }

    fn has_json_content_type(&self) -> bool {
        if let Some(content_type) = self.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()) {
            let essence = if let Some((essence, _)) = content_type.split_once(';') {
                essence
            } else {
                content_type
            };
            essence == "application/json"
                || (essence.starts_with("application/") && essence.ends_with("+json"))
        } else {
            false
        }
    }

    fn select_language<'a>(&'a self, supported_locales: &[&'a str]) -> Option<&'a str> {
        let header_value = self
            .get(header::ACCEPT_LANGUAGE)
            .and_then(|v| v.to_str().ok())?;
        let mut languages = header_value
            .split(',')
            .filter_map(|s| {
                let (language, quality) = if let Some((language, quality)) = s.split_once(';') {
                    let quality = quality.trim().strip_prefix("q=")?.parse::<f32>().ok()?;
                    (language.trim(), quality)
                } else {
                    (s.trim(), 1.0)
                };
                supported_locales.iter().find_map(|&locale| {
                    (locale.eq_ignore_ascii_case(language) || locale.starts_with(language))
                        .then_some((locale, quality))
                })
            })
            .collect::<Vec<_>>();
        languages.sort_by(|a, b| b.1.total_cmp(&a.1));
        languages.first().map(|&(language, _)| language)
    }
}

#[cfg(test)]
mod tests {
    use super::HeaderMapExt;
    use http::header::{self, HeaderMap, HeaderValue};

    #[test]
    fn it_selects_language() {
        let mut headers = HeaderMap::new();
        let header_value = "zh-CN,zh;q=0.9,en;q=0.8,en-US;q=0.7";
        headers.insert(
            header::ACCEPT_LANGUAGE,
            HeaderValue::from_static(header_value),
        );
        assert_eq!(
            headers.select_language(vec!["en-US", "zh-CN"]),
            Some("zh-CN"),
        );

        let header_value = "zh-HK,zh;q=0.8,en-US; q=0.7";
        headers.insert(
            header::ACCEPT_LANGUAGE,
            HeaderValue::from_static(header_value),
        );
        assert_eq!(
            headers.select_language(vec!["en-US", "zh-CN"]),
            Some("zh-CN"),
        );

        let header_value = "zh-HK, zh;q=0.8,en-US; q=0.9";
        headers.insert(
            header::ACCEPT_LANGUAGE,
            HeaderValue::from_static(header_value),
        );
        assert_eq!(
            headers.select_language(vec!["en-US", "zh-CN"]),
            Some("en-US"),
        );
    }
}
