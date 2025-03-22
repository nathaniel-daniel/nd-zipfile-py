use ouroboros::self_referencing;
use parking_lot::ArcMutexGuard;
use parking_lot::Mutex;
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::exceptions::PyNotImplementedError;
use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyBytesMethods;
use pyo3::types::PyString;
use pyo3::types::PyStringMethods;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use zip::read::ZipArchive;

create_exception!(nd_zip, BadZipfile, PyException, "File is not a zip file");

#[pyclass]
pub struct ZipFile {
    file: Option<Arc<Mutex<ZipArchive<File>>>>,
}

#[pymethods]
impl ZipFile {
    #[new]
    #[pyo3(signature = (file, mode="r"))]
    fn new(file: PyObject, mode: &str, py: Python<'_>) -> PyResult<Self> {
        match mode {
            "r" => {}
            "w" | "x" | "a" => {
                return Err(PyNotImplementedError::new_err(
                    "ZipFile modes 'w', 'x', and 'a' are currently unsupported",
                ));
            }
            _ => {
                return Err(PyValueError::new_err(
                    "ZipFile requires mode 'r', 'w', 'x', or 'a'",
                ));
            }
        }

        let file = match file.downcast_bound::<PyString>(py) {
            Ok(file) => {
                let file = file.to_cow()?;
                let file = File::open(&*file)?;
                ZipArchive::new(file).map_err(|error| BadZipfile::new_err(error.to_string()))?
            }
            Err(_error) => {
                return Err(PyValueError::new_err(
                    "ZipFile file currently must be a string",
                ));
            }
        };

        Ok(Self {
            file: Some(Arc::new(Mutex::new(file))),
        })
    }

    /// Close the archive file.
    pub fn close(&mut self) {
        if let Some(file) = self.file.take() {
            // The zip crate does not expose a way to access the internal file.
            // This is the best we can do here.
            drop(file);
        }
    }

    #[pyo3(signature = (name, mode="r", pwd=None))]
    pub fn open(
        &mut self,
        name: &str,
        mode: &str,
        pwd: Option<Bound<'_, PyBytes>>,
    ) -> PyResult<ZipExtFile> {
        match mode {
            "r" => {
                let zip_file = self
                    .file
                    .as_ref()
                    .ok_or_else(|| {
                        PyValueError::new_err("Attempt to use ZIP archive that was already closed")
                    })?
                    .clone();

                let lock = zip_file.try_lock_arc().ok_or_else(|| {
                    PyRuntimeError::new_err(
                        "Cannot open another file handle while another file handle is still open",
                    )
                })?;

                let inner_result = ZipExtFileInnerTryBuilder {
                    lock,
                    file_builder: |lock| {
                        let encrypted = {
                            let file = lock
                                .by_name(name)
                                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;

                            file.encrypted()
                        };

                        if encrypted {
                            let password = pwd
                                .as_ref()
                                .ok_or_else(|| {
                                    PyRuntimeError::new_err(format!(
                                        "File {name} is encrypted, password equired for extraction"
                                    ))
                                })?
                                .as_bytes();

                            lock.by_name_decrypt(name, password)
                                .map_err(|error| PyRuntimeError::new_err(error.to_string()))
                        } else {
                            lock.by_name(name)
                                .map_err(|error| PyRuntimeError::new_err(error.to_string()))
                        }
                    },
                }
                .try_build()?;

                return Ok(ZipExtFile {
                    inner: Some(inner_result),
                });
            }
            "w" => {
                return Err(PyNotImplementedError::new_err(
                    "open() currently requires mode \"r\"",
                ))
            }
            _ => {
                return Err(PyNotImplementedError::new_err(
                    "open() requires mode \"r\" or \"w\"",
                ))
            }
        }
    }

    pub fn __enter__<'p>(this: PyRef<'p, Self>, _py: Python<'p>) -> PyResult<PyRef<'p, Self>> {
        Ok(this)
    }

    pub fn __exit__(&mut self, _exc_type: PyObject, _exc_value: PyObject, _traceback: PyObject) {
        self.close();
    }
}

#[self_referencing]
struct ZipExtFileInner {
    lock: ArcMutexGuard<parking_lot::RawMutex, ZipArchive<File>>,

    #[borrows(mut lock)]
    #[not_covariant]
    file: zip::read::ZipFile<'this>,
}

#[pyclass(unsendable)]
pub struct ZipExtFile {
    inner: Option<ZipExtFileInner>,
}

#[pymethods]
impl ZipExtFile {
    pub fn read(&mut self) -> PyResult<Vec<u8>> {
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

    pub fn close(&mut self) {
        if let Some(inner) = self.inner.take() {
            drop(inner);
        }
    }
    
    pub fn __enter__<'p>(this: PyRef<'p, Self>, _py: Python<'p>) -> PyResult<PyRef<'p, Self>> {
        Ok(this)
    }

    pub fn __exit__(&mut self, _exc_type: PyObject, _exc_value: PyObject, _traceback: PyObject) {
        self.close();
    }
}

#[pymodule]
#[pyo3(name = "nd_zipfile")]
fn nd_zipfile(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ZipFile>()?;
    Ok(())
}
