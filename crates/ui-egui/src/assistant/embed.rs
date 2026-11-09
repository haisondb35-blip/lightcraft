//! Save chat with file, LightCraft's way: a library is a folder, not a file that is saved, so the
//! assistant's conversation about it goes into the folder, as [`FILE`], when the person turns it
//! on (off by default). It travels with the library and comes back when the library is opened on
//! another computer.
//!
//! The file (portable form, without this computer's conversation id, see `Memory::to_json`) is
//! written a few seconds after the conversation changes, while the pane is open. Turning the
//! option off stops writing it; the file already there stays until it is deleted.

use std::sync::Mutex;

use serde_json::Value;

use super::memory::Shelf;

/// The file in the library folder that holds the conversation.
pub const FILE: &str = "artcraft-assistant.json";
/// The largest conversation file read back.
const MAX_FILE: u64 = 8 << 20;
/// How often the conversation is compared with what was written.
const EVERY: std::time::Duration = std::time::Duration::from_secs(3);

/// What was last written (or found) for a library, and when it was checked.
static LAST: Mutex<Option<(String, Option<Value>, web_time::Instant)>> = Mutex::new(None);

fn file_in(library: &str) -> std::path::PathBuf {
    std::path::Path::new(library).join(FILE)
}

/// The conversation saved in the library folder `library`, if it has one.
pub fn read(library: &str) -> Option<Value> {
    let file = file_in(library);
    if std::fs::metadata(&file).ok()?.len() > MAX_FILE {
        return None;
    }
    serde_json::from_slice(&std::fs::read(file).ok()?).ok()
}

/// Write the conversation `shelf` has for `library` into its folder when it changed (and the
/// option is on). Called every frame the pane shows; does its work every few seconds.
pub fn sync(shelf: &Mutex<Shelf>, library: &str) {
    let Ok(mut last) = LAST.lock() else { return };
    let now = web_time::Instant::now();
    if let Some((dir, _, at)) = last.as_ref()
        && dir == library
        && now.duration_since(*at) < EVERY
    {
        return;
    }
    let written = match last.take() {
        Some((dir, chat, _)) if dir == library => chat,
        _ => read(library),
    };
    let chat = shelf.lock().ok().and_then(|s| s.chat_for(library));
    let keep = match &chat {
        Some(c) if written.as_ref() != Some(c) && write(library, c) => Some(c.clone()),
        _ => written,
    };
    *last = Some((library.to_string(), keep, now));
}

/// Write `chat` into the library folder through a temporary file, so a failed write never leaves
/// half a file. True when it was written.
fn write(library: &str, chat: &Value) -> bool {
    let file = file_in(library);
    let tmp = file.with_extension(format!("json.{}.tmp", std::process::id()));
    let ok = std::fs::write(&tmp, chat.to_string()).and_then(|()| std::fs::rename(&tmp, &file));
    if let Err(e) = &ok {
        log::warn!("Save chat with file: couldn't write {}: {e}", file.display());
        let _ = std::fs::remove_file(&tmp);
    }
    ok.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_chat_written_into_the_library_folder_reads_back() {
        let dir = std::env::temp_dir().join(format!("lightcraft-chat-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let library = dir.to_string_lossy().into_owned();
        assert_eq!(read(&library), None);
        let chat = json!({"version": 1, "messages": [{"role": "user", "text": "Chỉnh ảnh cưới cho sáng hơn"}]});
        assert!(write(&library, &chat));
        assert_eq!(read(&library), Some(chat));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "no temporary file left");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
