//! Turns a browser-chosen `.zip` File into the `{"path","content"}` pairs
//! `h_upload_repository` already accepts. The archive itself never leaves
//! the browser as a blob -- only the decoded text of each entry does, over
//! the same JSON body a hand-typed upload would send.
//!
//! Bounds mirror the server's own (`h_upload_repository` in
//! `workers/api/src/lib.rs`) so a rejection is a client-side error message
//! instead of a wasted round trip: 500 files, 2MB/file, 20MB total. A
//! binary entry (not valid UTF-8) is skipped, not rejected -- an upload
//! is meant for source, and a Curator connector or a knock resource reads
//! the manifest as text; silently dropping a `.png` is the right call,
//! failing the whole upload over it is not.

use std::io::Cursor;

pub struct ZippedFile {
    pub path: String,
    pub content: String,
}

const MAX_FILES: usize = 500;
const MAX_FILE_BYTES: usize = 2_000_000;
const MAX_TOTAL_BYTES: usize = 20_000_000;

pub fn extract(bytes: &[u8]) -> Result<Vec<ZippedFile>, String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("not a valid zip: {e}"))?;
    if archive.len() > MAX_FILES {
        return Err(format!("zip has {} entries, this account's cap is {MAX_FILES}", archive.len()));
    }

    let mut out = Vec::new();
    let mut total: usize = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("reading entry {i}: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let Some(path) = entry.enclosed_name() else {
            // `enclosed_name` is None for a path that would escape the
            // extraction root (`../..`) or isn't representable -- the
            // same discipline as the server's own `contains("..")` check
            // on an uploaded path, just caught one layer earlier.
            continue;
        };
        let path = path.to_string_lossy().replace('\\', "/");
        if entry.size() as usize > MAX_FILE_BYTES {
            continue;
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        std::io::Read::read_to_end(&mut entry, &mut buf).map_err(|e| format!("reading {path}: {e}"))?;
        let Ok(content) = String::from_utf8(buf) else {
            continue; // binary entry -- skipped, not fatal, see module doc
        };
        total += content.len();
        if total > MAX_TOTAL_BYTES {
            return Err(format!("zip exceeds this account's {}MB cap", MAX_TOTAL_BYTES / 1_000_000));
        }
        out.push(ZippedFile { path, content });
    }
    if out.is_empty() {
        return Err("no readable text files found in that zip".into());
    }
    Ok(out)
}
