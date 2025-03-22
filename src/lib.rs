mod read;
mod write;

use self::read::ReadZipExtFile;
use self::read::ReadZipFile;
use self::write::WriteZipFile;
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::exceptions::PyNotImplementedError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyString;
use pyo3::types::PyStringMethods;
use std::fs::File;

create_exception!(nd_zip, BadZipFile, PyException, "File is not a zip file");

#[derive(Debug)]
enum ZipFileInner {
    Read(ReadZipFile),
    Write(WriteZipFile),
}

impl ZipFileInner {
    pub fn close(&mut self) -> PyResult<()> {
        match self {
            Self::Read(file) => file.close(),
            Self::Write(file) => file.close(),
        }
    }

    pub fn open(
        &mut self,
        name: &str,
        mode: &str,
        pwd: Option<Bound<'_, PyBytes>>,
    ) -> PyResult<ReadZipExtFile> {
        match (self, mode) {
            (Self::Read(file), "r") => file.open(name, pwd),
            (Self::Read(_file), "w") => Err(PyValueError::new_err("archive opened as read-only")),
            (Self::Write(_file), "r") => Err(PyValueError::new_err("archive opened as write-only")),
            (Self::Write(_file), "w") => {
                if pwd.is_some() {
                    return Err(PyNotImplementedError::new_err(
                        "writing encrypted files is currently not supported",
                    ));
                }

                // file.open(name)
                todo!()
            }
            _ => Err(PyValueError::new_err("open() requires mode \"r\" or \"w\"")),
        }
    }
}

#[pyclass]
pub struct ZipFile {
    file: ZipFileInner,
}

#[pymethods]
impl ZipFile {
    #[new]
    #[pyo3(signature = (file, mode="r"))]
    fn new(file: PyObject, mode: &str, py: Python<'_>) -> PyResult<Self> {
        let file = match file.downcast_bound::<PyString>(py) {
            Ok(file) => file.to_cow()?,
            Err(_error) => {
                return Err(PyValueError::new_err(
                    "ZipFile file currently must be a string",
                ));
            }
        };

        let file = match mode {
            "r" => {
                let file = File::open(&*file)?;

                ZipFileInner::Read(ReadZipFile::new(file)?)
            }
            "w" => {
                let file = File::create(&*file)?;

                ZipFileInner::Write(WriteZipFile::new(file)?)
            }
            "x" | "a" => {
                return Err(PyNotImplementedError::new_err(
                    "ZipFile modes 'w', 'x', and 'a' are currently unsupported",
                ));
            }
            _ => {
                return Err(PyValueError::new_err(
                    "ZipFile requires mode 'r', 'w', 'x', or 'a'",
                ));
            }
        };

        Ok(Self { file })
    }

    /// Close the archive file.
    pub fn close(&mut self) -> PyResult<()> {
        self.file.close()
    }

    #[pyo3(signature = (name, mode="r", pwd=None))]
    pub fn open(
        &mut self,
        name: &str,
        mode: &str,
        pwd: Option<Bound<'_, PyBytes>>,
    ) -> PyResult<ReadZipExtFile> {
        self.file.open(name, mode, pwd)
    }

    pub fn __enter__<'p>(this: PyRef<'p, Self>, _py: Python<'p>) -> PyResult<PyRef<'p, Self>> {
        Ok(this)
    }

    pub fn __exit__(
        &mut self,
        _exc_type: PyObject,
        _exc_value: PyObject,
        _traceback: PyObject,
    ) -> PyResult<()> {
        self.close()?;
        Ok(())
    }
}

#[pymodule]
#[pyo3(name = "nd_zipfile")]
fn nd_zipfile(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ZipFile>()?;
    Ok(())
}
