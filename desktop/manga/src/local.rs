//! Reading a comic archive from the device: a CBZ is a zip of page images.

use crate::eval::natural_cmp;
use anyhow::Context;
use sha2::Digest;
use std::io::Read;
use std::path::{Path, PathBuf};

/// A comic taken from an archive: its title and the page images, in reading order, on disk.
#[derive(Debug, PartialEq)]
pub struct Imported {
    pub id: String,
    pub title: String,
    pub pages: Vec<PathBuf>,
}

const MAX_PAGES: usize = 3000;
const MAX_PAGE_BYTES: u64 = 40 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;

fn is_image(name: &str) -> bool {
    let name = name.to_lowercase();
    ["jpg", "jpeg", "png", "webp", "gif"].iter().any(|ext| name.ends_with(&format!(".{ext}")))
}

/// Unpacks the page images of `archive` into `dir/<id>/`, where the id comes from the file's name. Anything in
/// the archive that is not an image is ignored; an archive with no image is an error.
pub fn import(archive: &Path, dir: &Path) -> anyhow::Result<Imported> {
    let stem = archive.file_stem().and_then(|stem| stem.to_str()).context("the file has no name")?;
    let readable: String = stem.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect::<String>().trim_matches('-').chars().take(48).collect();
    // A name that had to be changed to be one (or has no latin letters) is told apart by a hash of the real name.
    let hash: String = sha2::Sha256::digest(stem.as_bytes()).iter().take(3).map(|byte| format!("{byte:02x}")).collect();
    let id = if readable.is_empty() { format!("comic-{hash}") } else if readable == stem { readable } else { format!("{readable}-{hash}") };
    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive).with_context(|| format!("cannot open {}", archive.display()))?).context("that is not a comic archive")?;
    let mut names: Vec<String> = zip.file_names().filter_map(Result::ok).filter(|name| is_image(name) && !name.contains("__MACOSX")).map(|name| name.to_string()).collect();
    names.sort_by(|a, b| natural_cmp(a, b));
    anyhow::ensure!(!names.is_empty(), "the archive holds no images");
    anyhow::ensure!(names.len() <= MAX_PAGES, "the archive holds more pages than a comic does");
    let target = dir.join(&id);
    let _ = std::fs::remove_dir_all(&target);
    std::fs::create_dir_all(&target)?;
    let mut total = 0u64;
    let mut pages = Vec::new();
    for (index, name) in names.iter().enumerate() {
        let mut bytes = Vec::new();
        zip.by_name(name)?.take(MAX_PAGE_BYTES + 1).read_to_end(&mut bytes)?;
        total += bytes.len() as u64;
        anyhow::ensure!(bytes.len() as u64 <= MAX_PAGE_BYTES && total <= MAX_TOTAL_BYTES, "the archive is far larger than a comic should be");
        let ext = Path::new(name).extension().and_then(|ext| ext.to_str()).unwrap_or("jpg").to_lowercase();
        let path = target.join(format!("{index:04}.{ext}"));
        std::fs::write(&path, bytes)?;
        pages.push(path);
    }
    Ok(Imported { id, title: stem.replace(['_', '-'], " ").trim().to_string(), pages })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn archive(path: &Path, files: &[&str]) {
        let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for name in files {
            writer.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            writer.write_all(name.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn pages_come_out_in_counting_order_and_only_images() {
        let dir = std::env::temp_dir().join(format!("aki-local-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("My_Comic 1.cbz");
        archive(&file, &["10.jpg", "2.jpg", "1.png", "notes.txt", "__MACOSX/1.jpg"]);
        let imported = import(&file, &dir.join("out")).unwrap();
        assert!(imported.id.starts_with("my-comic-1-"), "{}", imported.id);
        assert_eq!(imported.title, "My Comic 1");
        let contents: Vec<String> = imported.pages.iter().map(|page| std::fs::read_to_string(page).unwrap()).collect();
        assert_eq!(contents, ["1.png", "2.jpg", "10.jpg"]);
        archive(&dir.join("empty.cbz"), &["readme.txt"]);
        assert!(import(&dir.join("empty.cbz"), &dir.join("out")).unwrap_err().to_string().contains("no images"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
