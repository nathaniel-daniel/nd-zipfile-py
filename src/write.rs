use parking_lot::Mutex;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
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
}
