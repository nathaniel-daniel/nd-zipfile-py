use super::CompressionKind;
use parking_lot::ArcMutexGuard;
use parking_lot::Mutex;
use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use zip::write::SimpleFileOptions;
use zip::write::ZipWriter;

#[derive(Debug)]
pub struct WriteZipFile {
    file: Arc<Mutex<Option<ZipWriter<File>>>>,
    compression_kind: CompressionKind,
    compression_level: Option<u8>,
}

impl WriteZipFile {
    pub fn new(
        file: File,
        compression_kind: CompressionKind,
        compression_level: Option<u8>,
    ) -> PyResult<Self> {
        let file = ZipWriter::new(file);
        Ok(Self {
            file: Arc::new(Mutex::new(Some(file))),
            compression_kind,
            compression_level,
        })
    }

    /// Close the archive file.
    pub(crate) fn close(&mut self) -> PyResult<()> {
        if let Some(file) = self.file.lock().take() {
            let mut writer = file
                .finish()
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            writer.flush()?;
        }

        Ok(())
    }

    pub fn open(&self, name: &str) -> PyResult<WriteZipExtFile> {
        let mut lock = self.file.try_lock_arc().ok_or_else(|| {
            PyRuntimeError::new_err(
                "Cannot open another file handle while another file handle is still open",
            )
        })?;

        let writer = lock.as_mut().ok_or_else(|| {
            PyValueError::new_err("Attempt to use ZIP archive that was already closed")
        })?;

        let mut options = SimpleFileOptions::default();
        match self.compression_kind {
            CompressionKind::Stored => {
                options = options.compression_method(zip::CompressionMethod::Stored);
            }
            CompressionKind::Deflated => {
                options = options.compression_method(zip::CompressionMethod::Deflated);
                if let Some(compression_level) = self.compression_level {
                    if !(0..=9).contains(&compression_level) {
                        return Err(PyValueError::new_err(format!(
                            "invalid ZIP_DEFLATED compresslevel {compression_level}"
                        )));
                    }

                    options = options.compression_level(Some(compression_level.into()));
                }
            }
            CompressionKind::Bzip2 => {
                options = options.compression_method(zip::CompressionMethod::Bzip2);
                if let Some(compression_level) = self.compression_level {
                    if !(1..=9).contains(&compression_level) {
                        return Err(PyValueError::new_err(format!(
                            "invalid ZIP_BZIP2 compresslevel {compression_level}"
                        )));
                    }

                    options = options.compression_level(Some(compression_level.into()));
                }
            }
            CompressionKind::Lzma => {
                options = options.compression_method(zip::CompressionMethod::Lzma);
            }
        }
        writer
            .start_file(name, options)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;

        Ok(WriteZipExtFile { lock })
    }
}

pub(crate) struct WriteZipExtFile {
    lock: ArcMutexGuard<parking_lot::RawMutex, Option<ZipWriter<File>>>,
}

impl WriteZipExtFile {
    pub(crate) fn write(&mut self, buffer: &[u8]) -> PyResult<()> {
        let writer = self.lock.as_mut().ok_or_else(|| {
            PyValueError::new_err("Attempt to use ZIP archive that was already closed")
        })?;

        writer.write_all(buffer)?;

        Ok(())
    }

    pub(crate) fn close(&mut self) {}

    pub(crate) fn __exit__(&mut self) {
        self.close();
    }
}
