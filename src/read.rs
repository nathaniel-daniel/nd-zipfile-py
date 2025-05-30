use crate::BadZipFile;
use parking_lot::ArcMutexGuard;
use parking_lot::Mutex;
use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use zip::ZipArchive;

#[derive(Debug)]
pub(crate) struct ReadZipFile {
    file: Arc<Mutex<Option<ZipArchive<File>>>>,
}

impl ReadZipFile {
    pub(crate) fn new(file: File) -> PyResult<Self> {
        let file = ZipArchive::new(file).map_err(|error| BadZipFile::new_err(error.to_string()))?;
        Ok(Self {
            file: Arc::new(Mutex::new(Some(file))),
        })
    }

    /// Close the archive file.
    pub(crate) fn close(&mut self) -> PyResult<()> {
        let mut file = self.file.try_lock().ok_or_else(|| {
            PyRuntimeError::new_err("Cannot close file while a file handle is still open")
        })?;

        if let Some(file) = file.take() {
            // The zip crate does not expose a way to access the internal file.
            // This is the best we can do here.
            drop(file);
        }

        Ok(())
    }

    pub fn open(&self, name: &str, pwd: Option<Bound<'_, PyBytes>>) -> PyResult<ReadZipExtFile> {
        let lock = self.file.try_lock_arc().ok_or_else(|| {
            PyRuntimeError::new_err(
                "Cannot open another file handle while another file handle is still open",
            )
        })?;

        let inner_result = ReadZipExtFileInnerTryBuilder {
            lock,
            file_builder: |lock| {
                let lock = lock.as_mut().ok_or_else(|| {
                    PyValueError::new_err("Attempt to use ZIP archive that was already closed")
                })?;

                let index = lock.index_for_name(name).ok_or_else(|| {
                    PyRuntimeError::new_err(format!("File {name} does not exist"))
                })?;

                let encrypted = {
                    let file = lock
                        .by_index_raw(index)
                        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;

                    file.encrypted()
                };

                if encrypted {
                    let password = pwd
                        .as_ref()
                        .ok_or_else(|| {
                            PyRuntimeError::new_err(format!(
                                "File {name} is encrypted, password required for extraction"
                            ))
                        })?
                        .as_bytes();

                    lock.by_index_decrypt(index, password)
                        .map_err(|error| PyRuntimeError::new_err(error.to_string()))
                } else {
                    lock.by_index(index)
                        .map_err(|error| PyRuntimeError::new_err(error.to_string()))
                }
            },
        }
        .try_build()?;

        Ok(ReadZipExtFile {
            inner: Some(inner_result),
        })
    }

    pub(crate) fn namelist(&self) -> PyResult<Vec<String>> {
        let mut lock = self.file.try_lock().ok_or_else(|| {
            PyRuntimeError::new_err("Cannot list zip while a file handle is still open")
        })?;
        let lock = lock.as_mut().ok_or_else(|| {
            PyValueError::new_err("Attempt to use ZIP archive that was already closed")
        })?;

        let names = lock.file_names().map(|v| v.to_string()).collect();

        Ok(names)
    }
}

#[ouroboros::self_referencing]
struct ReadZipExtFileInner {
    lock: ArcMutexGuard<parking_lot::RawMutex, Option<ZipArchive<File>>>,

    #[borrows(mut lock)]
    #[not_covariant]
    file: zip::read::ZipFile<'this, File>,
}

pub(crate) struct ReadZipExtFile {
    inner: Option<ReadZipExtFileInner>,
}

impl ReadZipExtFile {
    pub(crate) fn read(&mut self) -> PyResult<Vec<u8>> {
        let inner = self.inner.as_mut().ok_or_else(|| {
            PyValueError::new_err("Attempt to use ZipExtFile that was already closed")
        })?;
        inner.with_file_mut(|file| {
            let size = usize::try_from(file.size())
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            let mut buffer = Vec::with_capacity(size);
            file.read_to_end(&mut buffer)?;
            Ok(buffer)
        })
    }

    pub(crate) fn close(&mut self) {
        if let Some(inner) = self.inner.take() {
            drop(inner);
        }
    }

    pub(crate) fn __exit__(&mut self) {
        self.close();
    }
}
