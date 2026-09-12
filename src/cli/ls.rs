use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Output structure for an item listed from R2.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LsItem {
    pub key: String,
    pub size: u64,
    pub last_modified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
}

/// Fetch all items matching the prefix and delimiter settings, handling continuation tokens.
pub async fn list_items(
    bucket: &r2kit::Bucket,
    prefix: Option<&str>,
    recursive: bool,
) -> Result<Vec<LsItem>, AppError> {
    let mut items = Vec::new();
    let mut seen_prefixes = HashSet::new();
    let mut continuation_token = None;

    loop {
        let mut builder = bucket.list();
        if let Some(p) = prefix
            && !p.is_empty()
        {
            builder = builder.prefix(p);
        }
        if !recursive {
            builder = builder.delimiter("/");
        }
        if let Some(token) = continuation_token {
            builder = builder.continuation_token(token);
        }

        let page = builder.send().await.map_err(crate::r2::map_r2_error)?;

        // Rolled up common prefixes (folders) when delimiter is '/'
        for prefix in page.common_prefixes() {
            if seen_prefixes.insert(prefix.clone()) {
                items.push(LsItem {
                    key: prefix.clone(),
                    size: 0,
                    last_modified: None,
                    etag: None,
                });
            }
        }

        // Concrete objects
        for obj in page.objects() {
            let last_modified = obj.last_modified().map(|t| {
                chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339()
            });
            items.push(LsItem {
                key: obj.key().to_string(),
                size: obj.size(),
                last_modified,
                etag: obj.etag().map(ToString::to_string),
            });
        }

        if let Some(token) = page.next_continuation_token() {
            continuation_token = Some(token.to_string());
        } else {
            break;
        }
    }

    // Sort entries alphabetically by key
    items.sort_by(|a, b| a.key.cmp(&b.key));

    Ok(items)
}

/// Format listed items into a human-readable table.
pub fn format_table(items: &[LsItem]) -> String {
    if items.is_empty() {
        return "No objects found\n".to_string();
    }

    let mut out = String::new();
    out.push_str(&format!(
        "{:<50} {:<15} {:<25}\n",
        "Key", "Size (bytes)", "Last Modified"
    ));
    out.push_str(&format!("{:-<50} {:-<15} {:-<25}\n", "", "", ""));

    for item in items {
        let size_str = if item.last_modified.is_none() && item.key.ends_with('/') {
            "-".to_string()
        } else {
            item.size.to_string()
        };
        let date_str = item.last_modified.as_deref().unwrap_or("-");
        out.push_str(&format!(
            "{:<50} {:<15} {:<25}\n",
            item.key, size_str, date_str
        ));
    }

    out
}

/// Execute the `ls` subcommand.
pub async fn execute(
    bucket: &r2kit::Bucket,
    prefix: Option<&str>,
    recursive: bool,
    json: bool,
) -> Result<(), AppError> {
    let items = list_items(bucket, prefix, recursive).await?;

    if json {
        let json_str = serde_json::to_string_pretty(&items)
            .map_err(|e| AppError::BadRequest(format!("Failed to serialize JSON: {e}")))?;
        println!("{json_str}");
    } else {
        print!("{}", format_table(&items));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_table_empty() {
        let out = format_table(&[]);
        assert_eq!(out, "No objects found\n");
    }

    #[test]
    fn test_format_table_with_items() {
        let items = vec![
            LsItem {
                key: "photos/".to_string(),
                size: 0,
                last_modified: None,
                etag: None,
            },
            LsItem {
                key: "doc.txt".to_string(),
                size: 1024,
                last_modified: Some("2026-09-12T12:00:00Z".to_string()),
                etag: Some("etag123".to_string()),
            },
        ];

        let out = format_table(&items);
        assert!(out.contains("Key"));
        assert!(out.contains("Size (bytes)"));
        assert!(out.contains("Last Modified"));
        assert!(out.contains("photos/"));
        assert!(out.contains("doc.txt"));
        assert!(out.contains("1024"));
        assert!(out.contains("2026-09-12T12:00:00Z"));
    }

    #[test]
    fn test_ls_item_serialization() {
        let item = LsItem {
            key: "test.png".to_string(),
            size: 2048,
            last_modified: Some("2026-09-12T12:00:00Z".to_string()),
            etag: Some("etag_abc".to_string()),
        };

        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"key\":\"test.png\""));
        assert!(json.contains("\"size\":2048"));
        assert!(json.contains("\"etag\":\"etag_abc\""));
    }

    #[test]
    fn test_ls_items_sorting() {
        let mut items = [
            LsItem {
                key: "zeta.txt".to_string(),
                size: 10,
                last_modified: None,
                etag: None,
            },
            LsItem {
                key: "alpha/".to_string(),
                size: 0,
                last_modified: None,
                etag: None,
            },
            LsItem {
                key: "beta.txt".to_string(),
                size: 20,
                last_modified: None,
                etag: None,
            },
        ];

        items.sort_by(|a, b| a.key.cmp(&b.key));
        assert_eq!(items[0].key, "alpha/");
        assert_eq!(items[1].key, "beta.txt");
        assert_eq!(items[2].key, "zeta.txt");
    }
}
