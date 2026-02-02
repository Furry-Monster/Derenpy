//! RPA archive writer

use anyhow::{Context, Result};
use flate2::Compression;
use flate2::write::ZlibEncoder;
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub enum RpaWriterVersion {
    Rpa2,
    Rpa3,
}

impl RpaWriterVersion {
    pub fn from_str(s: &str) -> Self {
        match s {
            "2.0" | "2" => Self::Rpa2,
            _ => Self::Rpa3,
        }
    }
}

struct FileEntry {
    offset: u64,
    length: u64,
    archive_path: String,
}

pub struct RpaWriter {
    file: BufWriter<File>,
    version: RpaWriterVersion,
    key: u64,
    entries: Vec<FileEntry>,
}

impl RpaWriter {
    pub fn new<P: AsRef<Path>>(path: P, version: &str) -> Result<Self> {
        let file = File::create(path.as_ref()).context("Failed to create RPA file")?;
        let mut writer = BufWriter::new(file);
        let version = RpaWriterVersion::from_str(version);

        // Generate random key for RPA-3.0
        let key = if matches!(version, RpaWriterVersion::Rpa3) {
            rand_key()
        } else {
            0
        };

        // Write placeholder header (will be updated at the end)
        let header = format!("{:0<50}\n", "");
        writer.write_all(header.as_bytes())?;

        Ok(Self {
            file: writer,
            version,
            key,
            entries: Vec::new(),
        })
    }

    pub fn add_file<P: AsRef<Path>>(&mut self, file_path: P, archive_path: &Path) -> Result<()> {
        let mut file = File::open(file_path.as_ref()).context("Failed to open input file")?;

        let offset = self.file.stream_position()?;

        // Copy file data
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        let length = buffer.len() as u64;

        self.file.write_all(&buffer)?;

        // Normalize path to use forward slashes
        let archive_path_str = archive_path.to_string_lossy().replace('\\', "/");

        self.entries.push(FileEntry {
            offset,
            length,
            archive_path: archive_path_str,
        });

        Ok(())
    }

    pub fn finish(mut self) -> Result<()> {
        // Get current position (this is where index will be written)
        let index_offset = self.file.stream_position()?;

        // Build index
        let index = self.build_index();

        // Serialize with pickle and compress
        let pickled = serde_pickle::to_vec(&index, Default::default())
            .context("Failed to serialize index")?;

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&pickled)?;
        let compressed = encoder.finish()?;

        // Write compressed index
        self.file.write_all(&compressed)?;

        // Seek back to start and write proper header
        self.file.seek(SeekFrom::Start(0))?;

        let header = match self.version {
            RpaWriterVersion::Rpa2 => {
                format!("RPA-2.0 {:016x}\n", index_offset)
            }
            RpaWriterVersion::Rpa3 => {
                format!("RPA-3.0 {:016x} {:08x}\n", index_offset, self.key)
            }
        };

        // Pad header to exactly 51 bytes
        let header = format!("{:0<51}", header);
        self.file.write_all(header.as_bytes())?;

        self.file.flush()?;

        Ok(())
    }

    fn build_index(&self) -> RpaIndex {
        let mut entries = BTreeMap::new();

        for entry in &self.entries {
            let (offset, length) = match self.version {
                RpaWriterVersion::Rpa2 => (entry.offset, entry.length),
                RpaWriterVersion::Rpa3 => (entry.offset ^ self.key, entry.length ^ self.key),
            };

            entries.insert(
                entry.archive_path.clone(),
                vec![(offset as i64, length as i64, Vec::new())],
            );
        }

        RpaIndex { entries }
    }
}

struct RpaIndex {
    entries: BTreeMap<String, Vec<(i64, i64, Vec<u8>)>>,
}

impl Serialize for RpaIndex {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.entries.len()))?;
        for (key, value) in &self.entries {
            // Use string as key - serde_pickle incorrectly serializes &[u8] as int list
            // Python/Ren'Py can handle both string and bytes keys
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

fn rand_key() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (duration.as_nanos() as u64) & 0xFFFFFFFF
}
