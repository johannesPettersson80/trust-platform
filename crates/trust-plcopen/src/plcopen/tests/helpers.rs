    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("trust-runtime-{prefix}-{stamp}"));
        std::fs::create_dir_all(&dir).expect("create temp directory");
        dir
    }

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(path, content).expect("write file");
    }

    fn pou_signatures(xml: &str) -> Vec<(String, String, String)> {
        let doc = roxmltree::Document::parse(xml).expect("parse XML");
        let mut items = doc
            .descendants()
            .filter(|node| is_element_named(*node, "pou"))
            .filter_map(|pou| {
                let name = pou.attribute("name")?.to_string();
                let pou_type = pou.attribute("pouType")?.to_string();
                let body = pou
                    .children()
                    .find(|child| is_element_named(*child, "body"))
                    .and_then(|body| {
                        body.children()
                            .find(|child| is_element_named(*child, "ST"))
                            .and_then(|st| st.text())
                    })
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_string();
                Some((name, pou_type, body))
            })
            .collect::<Vec<_>>();
        items.sort();
        items
    }
