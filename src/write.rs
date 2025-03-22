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
}

impl WriteZipFile {
    pub fn new(file: File) -> PyResult<Self> {
        let file = ZipWriter::new(file);
        Ok(Self {
            file: Arc::new(Mutex::new(Some(file))),
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

        let options = SimpleFileOptions::default();
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
