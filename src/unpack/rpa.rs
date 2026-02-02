//! RPA archive parsing and extraction
//! Supported versions: RPA-2.0, RPA-3.0, RPA-3.2, RPA-4.0, ALT-1.0

use anyhow::{Context, Result};
use flate2::read::ZlibDecoder;
use serde_pickle::{HashableValue, Value as PickleValue};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const ALT_KEY_MASK: u64 = 0xDABE8DF0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RpaVersion {
    Rpa2,
    Rpa3,
    Rpa32,
    Rpa40,
    Alt1,
}

impl std::fmt::Display for RpaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RpaVersion::Rpa2 => write!(f, "RPA-2.0"),
            RpaVersion::Rpa3 => write!(f, "RPA-3.0"),
            RpaVersion::Rpa32 => write!(f, "RPA-3.2"),
            RpaVersion::Rpa40 => write!(f, "RPA-4.0"),
            RpaVersion::Alt1 => write!(f, "ALT-1.0"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RpaEntry {
    pub offset: u64,
    pub length: u64,
    pub prefix: Vec<u8>,
}

#[derive(Debug)]
pub struct RpaArchive {
    path: PathBuf,
    pub version: RpaVersion,
    pub index: HashMap<String, RpaEntry>,
}

impl RpaArchive {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path).context("Failed to open RPA file")?;
        let mut reader = BufReader::new(file);

        let mut first_line = Vec::new();
        reader
            .read_until(b'\n', &mut first_line)
            .context("Failed to read RPA header")?;

        let (version, index_offset, key) = Self::parse_header(&first_line)?;

        reader
            .seek(SeekFrom::Start(index_offset))
            .context("Failed to seek to index")?;

        let mut compressed = Vec::new();
        reader
            .read_to_end(&mut compressed)
            .context("Failed to read index data")?;

        let index = Self::parse_index(&compressed, key)?;

        Ok(Self {
            path,
            version,
            index,
        })
    }

    fn parse_header(header: &[u8]) -> Result<(RpaVersion, u64, Option<u64>)> {
        let header_str = String::from_utf8_lossy(header);
        let header_str = header_str.trim();

        // RPA-3.x formats: "RPA-X.X <offset> <key>"
        if header_str.starts_with("RPA-3.0")
            || header_str.starts_with("RPA-3.2")
            || header_str.starts_with("RPA-4.0")
        {
            let parts: Vec<&str> = header_str.split_whitespace().collect();
            if parts.len() >= 3 {
                let version = match parts[0] {
                    "RPA-3.2" => RpaVersion::Rpa32,
                    "RPA-4.0" => RpaVersion::Rpa40,
                    _ => RpaVersion::Rpa3,
                };
                let offset =
                    u64::from_str_radix(parts[1], 16).context("Invalid index offset in header")?;
                let key = u64::from_str_radix(parts[2], 16).context("Invalid key in header")?;
                return Ok((version, offset, Some(key)));
            }
            anyhow::bail!(
                "Invalid {} header format",
                parts.first().unwrap_or(&"RPA-3.x")
            );
        }

        // ALT-1.0 format: "ALT-1.0 <key^mask> <offset>"
        if header_str.starts_with("ALT-1.0") {
            let parts: Vec<&str> = header_str.split_whitespace().collect();
            if parts.len() >= 3 {
                let key_masked =
                    u64::from_str_radix(parts[1], 16).context("Invalid key in header")?;
                let key = key_masked ^ ALT_KEY_MASK;
                let offset =
                    u64::from_str_radix(parts[2], 16).context("Invalid index offset in header")?;
                return Ok((RpaVersion::Alt1, offset, Some(key)));
            }
            anyhow::bail!("Invalid ALT-1.0 header format");
        }

        // RPA-2.0 format: "RPA-2.0 <offset>"
        if header_str.starts_with("RPA-2.0") {
            let parts: Vec<&str> = header_str.split_whitespace().collect();
            if parts.len() >= 2 {
                let offset =
                    u64::from_str_radix(parts[1], 16).context("Invalid index offset in header")?;
                return Ok((RpaVersion::Rpa2, offset, None));
            }
            anyhow::bail!("Invalid RPA-2.0 header format");
        }

        anyhow::bail!("Unsupported or invalid RPA format: {}", header_str)
    }

    fn parse_index(compressed: &[u8], key: Option<u64>) -> Result<HashMap<String, RpaEntry>> {
        let mut decoder = ZlibDecoder::new(compressed);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .context("Failed to decompress index")?;

        let pickle_value: PickleValue = serde_pickle::from_slice(&decompressed, Default::default())
            .context("Failed to parse pickle index")?;

        Self::convert_index(pickle_value, key)
    }

    fn convert_index(value: PickleValue, key: Option<u64>) -> Result<HashMap<String, RpaEntry>> {
        let mut index = HashMap::new();

        let dict = match value {
            PickleValue::Dict(d) => d,
            _ => anyhow::bail!("Index is not a dictionary"),
        };

        for (k, v) in dict {
            let path = Self::extract_string_from_hashable(&k)?;
            let entry = Self::extract_entry(&v, key)?;
            index.insert(path, entry);
        }

        Ok(index)
    }

    fn extract_string_from_hashable(value: &HashableValue) -> Result<String> {
        match value {
            HashableValue::String(s) => Ok(s.clone()),
            HashableValue::Bytes(b) => {
                String::from_utf8(b.clone()).or_else(|_| Ok(String::from_utf8_lossy(b).to_string()))
            }
            _ => anyhow::bail!("Expected string, got {:?}", value),
        }
    }

    fn extract_entry(value: &PickleValue, key: Option<u64>) -> Result<RpaEntry> {
        let list = match value {
            PickleValue::List(l) => l,
            _ => anyhow::bail!("Entry is not a list"),
        };

        if list.is_empty() {
            anyhow::bail!("Empty entry list");
        }

        let first = &list[0];
        // serde-pickle may deserialize Python tuples as either Tuple or List
        let tuple = match first {
            PickleValue::Tuple(t) => t.clone(),
            PickleValue::List(l) => l.clone(),
            _ => anyhow::bail!("Entry item is not a tuple or list: {:?}", first),
        };

        if tuple.len() < 2 {
            anyhow::bail!("Entry tuple too short");
        }

        let offset = Self::extract_int(tuple.first().context("Missing offset")?)?;
        let length = Self::extract_int(tuple.get(1).context("Missing length")?)?;
        let prefix = if tuple.len() > 2 {
            Self::extract_bytes(tuple.get(2).unwrap())?
        } else {
            Vec::new()
        };

        let (offset, length) = if let Some(k) = key {
            ((offset as u64) ^ k, (length as u64) ^ k)
        } else {
            (offset as u64, length as u64)
        };

        Ok(RpaEntry {
            offset,
            length,
            prefix,
        })
    }

    fn extract_int(value: &PickleValue) -> Result<i64> {
        match value {
            PickleValue::I64(i) => Ok(*i),
            PickleValue::Int(i) => i
                .try_into()
                .map_err(|_| anyhow::anyhow!("Integer too large")),
            _ => anyhow::bail!("Expected integer, got {:?}", value),
        }
    }

    fn extract_bytes(value: &PickleValue) -> Result<Vec<u8>> {
        match value {
            PickleValue::Bytes(b) => Ok(b.clone()),
            PickleValue::String(s) => Ok(s.as_bytes().to_vec()),
            _ => Ok(Vec::new()),
        }
    }

    pub fn extract_file<P: AsRef<Path>>(&self, name: &str, output_dir: P) -> Result<PathBuf> {
        let entry = self
            .index
            .get(name)
            .context(format!("File '{}' not found in archive", name))?;

        let output_path = output_dir.as_ref().join(name);

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).context("Failed to create output directory")?;
        }

        let mut archive = File::open(&self.path).context("Failed to open archive")?;
        archive
            .seek(SeekFrom::Start(entry.offset))
            .context("Failed to seek to file data")?;

        let mut data = vec![0u8; entry.length as usize];
        archive
            .read_exact(&mut data)
            .context("Failed to read file data")?;

        let mut output = File::create(&output_path).context("Failed to create output file")?;

        if !entry.prefix.is_empty() {
            output
                .write_all(&entry.prefix)
                .context("Failed to write prefix")?;
        }
        output
            .write_all(&data)
            .context("Failed to write file data")?;

        Ok(output_path)
    }

    pub fn extract_all<P: AsRef<Path>>(
        &self,
        output_dir: P,
        progress: Option<&indicatif::ProgressBar>,
    ) -> Result<Vec<PathBuf>> {
        let names: Vec<String> = self.index.keys().cloned().collect();
        let mut extracted = Vec::with_capacity(names.len());

        for name in &names {
            let path = self.extract_file(name, output_dir.as_ref())?;
            extracted.push(path);
            if let Some(pb) = progress {
                pb.inc(1);
            }
        }

        Ok(extracted)
    }

    pub fn file_count(&self) -> usize {
        self.index.len()
    }
}
