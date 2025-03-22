use super::CompressionKind;
use crate::ZipInfo;
use parking_lot::ArcMutexGuard;
use parking_lot::Mutex;
use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyString;
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

    pub fn open(&self, name: &Bound<'_, PyAny>) -> PyResult<WriteZipExtFile> {
        let mut lock = self.file.try_lock_arc().ok_or_else(|| {
            PyRuntimeError::new_err(
                "Cannot open another file handle while another file handle is still open",
            )
        })?;

        let writer = lock.as_mut().ok_or_else(|| {
            PyValueError::new_err("Attempt to use ZIP archive that was already closed")
        })?;

        let zip_info = if let Ok(name) = name.downcast::<PyString>() {
            let name = name.to_cow()?;

            let mut zip_info = ZipInfo::new(&name);
            zip_info.compress_type = u8::from(self.compression_kind);
            zip_info.compress_level = self.compression_level;

            zip_info
        } else if let Ok(zip_info) = name.extract::<PyRef<'_, ZipInfo>>() {
            zip_info.clone()
        } else {
            return Err(PyValueError::new_err("name must be a string or ZipInfo"));
        };

        let mut options = SimpleFileOptions::default();
        let compression_kind = CompressionKind::try_from(zip_info.compress_type)?;
        match compression_kind {
            CompressionKind::Stored => {
                options = options.compression_method(zip::CompressionMethod::Stored);
            }
            CompressionKind::Deflated => {
                options = options.compression_method(zip::CompressionMethod::Deflated);
                if let Some(compression_level) = zip_info.compress_level {
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
                if let Some(compression_level) = zip_info.compress_level {
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
            .start_file(zip_info.filename, options)
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
