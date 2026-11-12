use super::Store;
use crate::error::{CoreError, Result};
use crate::model::*;
use rusqlite::params;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Clés de réglage connues (snake_case, comme en base).
const KEYS: &[&str] = &["undo_send_seconds", "remote_images", "theme", "accent", "text_size", "density", "show_snippet"];

fn parse<T: DeserializeOwned>(key: &str, v: &Value) -> Result<T> {
    serde_json::from_value(v.clone()).map_err(|_| CoreError::InvalidInput(format!("valeur refusée pour {key} : {v}")))
}

fn apply(st: &mut Settings, key: &str, v: &Value) -> Result<()> {
    match key {
        "undo_send_seconds" => {
            let n: u32 = parse(key, v)?;
            if ![5, 10, 30].contains(&n) {
                return Err(CoreError::InvalidInput(format!("délai d'annulation refusé : {n} s (5, 10 ou 30)")));
            }
            st.undo_send_seconds = n;
        }
        "remote_images" => st.remote_images = parse(key, v)?,
        "theme" => st.theme = parse(key, v)?,
        "accent" => st.accent = parse(key, v)?,
        "text_size" => st.text_size = parse(key, v)?,
        "density" => st.density = parse(key, v)?,
        "show_snippet" => st.show_snippet = parse(key, v)?,
        other => return Err(CoreError::InvalidInput(format!("réglage inconnu : {other}"))),
    }
    Ok(())
}

impl Store {
    pub fn settings(&self) -> Result<Settings> {
        let rows: Vec<(String, String)> = self.with_read(|c| {
            let mut st = c.prepare("SELECT key, value_json FROM settings")?;
            let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(v)
        })?;
        let mut out = Settings::default();
        for (k, raw) in rows {
            if !KEYS.contains(&k.as_str()) {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<Value>(&raw) {
                // Valeur corrompue en base : on garde la valeur par défaut.
                let _ = apply(&mut out, &k, &v);
            }
        }
        Ok(out)
    }

    pub fn set_setting(&self, key: &str, value: Value) -> Result<Settings> {
        let mut current = self.settings()?;
        apply(&mut current, key, &value)?;
        self.with(|c| {
            c.execute(
                "INSERT INTO settings (key, value_json) VALUES (?1, ?2) \
                 ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
                params![key, value.to_string()],
            )?;
            Ok(())
        })?;
        Ok(current)
    }
}

#[cfg(test)]
mod tests {
    use crate::model::*;
    use crate::store::Store;
    use crate::CoreError;
    use serde_json::json;

    #[test]
    fn defaults_when_empty() {
        let s = Store::open_in_memory().unwrap();
        assert_eq!(s.settings().unwrap(), Settings::default());
        assert_eq!(Settings::default().undo_send_seconds, 10);
        assert_eq!(Settings::default().theme, Theme::Dark);
        assert!(Settings::default().show_snippet);
    }

    #[test]
    fn set_and_read_back() {
        let s = Store::open_in_memory().unwrap();
        s.set_setting("undo_send_seconds", json!(30)).unwrap();
        let st = s.set_setting("theme", json!("light")).unwrap();
        assert_eq!(st.undo_send_seconds, 30);
        assert_eq!(st.theme, Theme::Light);
        assert_eq!(s.settings().unwrap().theme, Theme::Light);
    }

    #[test]
    fn rejects_bad_values() {
        let s = Store::open_in_memory().unwrap();
        for (k, v) in [("undo_send_seconds", json!(7)), ("theme", json!("pink")), ("nope", json!(1)), ("show_snippet", json!("oui"))] {
            assert!(matches!(s.set_setting(k, v.clone()), Err(CoreError::InvalidInput(_))), "{k} = {v}");
        }
        assert_eq!(s.settings().unwrap(), Settings::default());
    }

    #[test]
    fn settings_serialize_camel_case() {
        let v = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(v["undoSendSeconds"], 10);
        assert_eq!(v["remoteImages"], "block");
        assert_eq!(v["textSize"], "normal");
    }
}
