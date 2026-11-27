//! The part of TorBox the TV needs: which releases it already holds, and a link to one file.
//! The TV holds the key itself, so the player takes the link straight from TorBox.
//! Only releases TorBox already has are used: one it would have to fetch cannot be watched now.

use akidev_core::model::Release;
use akidev_core::releases::VideoFile;
use anyhow::{Context, bail};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

const API: &str = "https://api.torbox.app/v1/api/torrents";

pub struct TorBox {
    key: String,
    client: reqwest::Client,
}

/// A torrent in the account, with the numeric ids `requestdl` needs.
pub struct Entry {
    pub id: i64,
    pub files: Vec<(i64, VideoFile)>,
}

impl TorBox {
    pub fn new(key: &str) -> TorBox {
        TorBox {
            key: key.to_string(),
            // A stalled network must end in a message, not in a screen that waits for ever.
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("a client"),
        }
    }

    /// Takes the key out of text that may be shown: it rides in the query of `requestdl`.
    pub fn redact(&self, text: impl ToString) -> String {
        text.to_string().replace(&self.key, "[KEY]")
    }

    /// TorBox answers every route with `{ success, detail, data }`.
    fn unwrap(body: Value) -> anyhow::Result<Value> {
        if body["success"].as_bool() != Some(true) {
            bail!("TorBox: {}", body["detail"].as_str().unwrap_or("refused"));
        }
        Ok(body["data"].clone())
    }

    async fn get(&self, route: &str, query: &[(&str, String)]) -> anyhow::Result<Value> {
        let body = self
            .client
            .get(format!("{API}/{route}"))
            .bearer_auth(&self.key)
            .query(query)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!(self.redact(e.without_url())))?
            .json()
            .await
            .map_err(|e| anyhow::anyhow!(self.redact(e.without_url())))?;
        Self::unwrap(body)
    }

    /// Whether the key is accepted.
    pub async fn check_key(&self) -> anyhow::Result<()> {
        self.get("mylist", &[("limit", "1".into())])
            .await
            .map(|_| ())
    }

    /// The releases TorBox holds, each with its files when TorBox listed them (`None`: not listed).
    pub async fn cached(
        &self,
        releases: &[Release],
    ) -> anyhow::Result<HashMap<String, Option<Vec<VideoFile>>>> {
        let mut hashes: Vec<&str> = releases.iter().map(|r| r.info_hash.as_str()).collect();
        hashes.sort_unstable();
        hashes.dedup();
        if hashes.is_empty() {
            return Ok(HashMap::new());
        }
        let data = self
            .get(
                "checkcached",
                &[
                    ("hash", hashes.join(",")),
                    ("format", "object".into()),
                    ("list_files", "true".into()),
                ],
            )
            .await?;
        Ok(data
            .as_object()
            .map(|found| {
                found
                    .iter()
                    .filter(|(_, entry)| !entry.is_null() && entry.as_bool() != Some(false))
                    .map(|(hash, entry)| (hash.to_ascii_lowercase(), listed_files(entry)))
                    .collect()
            })
            .unwrap_or_default())
    }

    /// The account's entry for a release. A cached release the account does not hold yet is added
    /// first: that is what makes this work on a fresh account.
    pub async fn entry_for(&self, release: &Release) -> anyhow::Result<Option<Entry>> {
        if let Some(id) = self.add_if_cached(release).await
            && let Some(entry) = self.entry_by_id(id).await?
        {
            return Ok(Some(entry));
        }
        // Already in the account (the add gave no id), or not ready yet: look for it by its hash.
        let list = self.get("mylist", &[("limit", "5000".into())]).await?;
        for item in list.as_array().into_iter().flatten() {
            let same = item["hash"]
                .as_str()
                .is_some_and(|hash| hash.eq_ignore_ascii_case(&release.info_hash));
            if same && let Some(entry) = entry_of(item) {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }

    /// `add_only_if_cached` refuses anything TorBox does not already hold.
    async fn add_if_cached(&self, release: &Release) -> Option<i64> {
        let form = reqwest::multipart::Form::new()
            .text("magnet", release.magnet.clone())
            .text("add_only_if_cached", "true");
        let sent = self
            .client
            .post(format!("{API}/createtorrent"))
            .bearer_auth(&self.key)
            .multipart(form)
            .send()
            .await
            .ok()?;
        let data = Self::unwrap(sent.json().await.ok()?).ok()?;
        data["torrent_id"].as_i64()
    }

    async fn entry_by_id(&self, id: i64) -> anyhow::Result<Option<Entry>> {
        let data = self
            .get(
                "mylist",
                &[("id", id.to_string()), ("bypass_cache", "true".into())],
            )
            .await?;
        let one = match &data {
            Value::Array(all) => all.first().cloned().unwrap_or(Value::Null),
            other => other.clone(),
        };
        Ok(entry_of(&one))
    }

    /// The link for one file. It is valid for a short time and carries the key, so it is asked for
    /// again for each play and never kept.
    pub async fn link(&self, torrent_id: i64, file_id: i64) -> anyhow::Result<String> {
        let data = self
            .get(
                "requestdl",
                &[
                    ("torrent_id", torrent_id.to_string()),
                    ("file_id", file_id.to_string()),
                    ("token", self.key.clone()),
                ],
            )
            .await?;
        data.as_str()
            .map(str::to_string)
            .context("TorBox gave no link")
    }
}

fn entry_of(item: &Value) -> Option<Entry> {
    let files: Vec<(i64, VideoFile)> = item["files"]
        .as_array()?
        .iter()
        .enumerate()
        .filter_map(|(index, file)| {
            let name = file["name"]
                .as_str()
                .filter(|name| !name.is_empty())
                .or_else(|| file["short_name"].as_str())?;
            let video = VideoFile {
                index,
                name: name.to_string(),
                length: file["size"].as_u64().unwrap_or(0),
            };
            Some((file["id"].as_i64()?, video))
        })
        .collect();
    (!files.is_empty()).then(|| Entry {
        id: item["id"].as_i64().unwrap_or_default(),
        files,
    })
}

/// The files a `checkcached` answer lists, or `None` when it lists none (it does not always).
fn listed_files(entry: &Value) -> Option<Vec<VideoFile>> {
    let files: Vec<VideoFile> = entry["files"]
        .as_array()?
        .iter()
        .enumerate()
        .filter_map(|(index, file)| {
            let name = file["name"]
                .as_str()
                .or_else(|| file["short_name"].as_str())?;
            Some(VideoFile {
                index,
                name: name.to_string(),
                length: file["size"].as_u64().unwrap_or(0),
            })
        })
        .collect();
    (!files.is_empty()).then_some(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_account_entry_keeps_the_ids_and_names_of_its_files() {
        let item = json!({ "id": 7, "hash": "abc", "files": [
            { "id": 1, "name": "Show/Show - 04.mkv", "size": 100 },
            { "id": 2, "name": "", "short_name": "Show - 05.mkv", "size": 200 },
            { "name": "no id", "size": 5 },
        ]});
        let entry = entry_of(&item).unwrap();
        assert_eq!(entry.id, 7);
        let ids: Vec<_> = entry
            .files
            .iter()
            .map(|(id, file)| (*id, file.name.as_str(), file.index))
            .collect();
        // The file with no id is left out. Positions are those of the whole list, so they stay in step with TorBox's own.
        assert_eq!(
            ids,
            vec![(1, "Show/Show - 04.mkv", 0), (2, "Show - 05.mkv", 1)]
        );
    }

    #[test]
    fn an_entry_with_no_files_is_not_an_entry() {
        assert!(entry_of(&json!({ "id": 7, "files": [] })).is_none());
        assert!(entry_of(&json!({ "id": 7 })).is_none());
    }

    #[test]
    fn a_cached_answer_lists_files_only_when_it_has_names() {
        let listed = json!({ "files": [{ "name": "a.mkv", "size": 9 }] });
        assert_eq!(listed_files(&listed).unwrap()[0].name, "a.mkv");
        assert!(listed_files(&json!({ "name": "x" })).is_none());
        assert!(listed_files(&json!({ "files": [] })).is_none());
    }

    #[test]
    fn the_key_is_taken_out_of_text() {
        let torbox = TorBox::new("secret-key");
        assert_eq!(
            torbox.redact("GET https://x/?token=secret-key failed"),
            "GET https://x/?token=[KEY] failed"
        );
    }
}
