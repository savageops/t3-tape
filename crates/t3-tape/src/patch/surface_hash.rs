use sha2::{Digest, Sha256};

use super::diff::UnifiedDiff;

pub fn compute(diff: &UnifiedDiff) -> String {
    let mut hasher = Sha256::new();

    for file in &diff.files {
        hasher.update(b"file:");
        hasher.update(file.path.as_bytes());
        hasher.update(b"\n");

        for hunk in &file.hunks {
            hasher.update(b"hunk:");
            hasher.update(hunk.header.as_bytes());
            hasher.update(b"\n");

            for line in &hunk.preimage_lines {
                hasher.update(b"pre:");
                hasher.update(line.as_bytes());
                hasher.update(b"\n");
            }
        }
    }

    let digest = hasher.finalize();
    let mut rendered = String::with_capacity(digest.len() * 2);
    for byte in digest {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}
