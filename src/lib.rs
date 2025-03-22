mod read;

use self::read::ReadZipExtFile;
use self::read::ReadZipFile;
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

#[pyclass]
pub struct ZipFile {
    file: ReadZipFile,
}

#[pymethods]
impl ZipFile {
    #[new]
    #[pyo3(signature = (file, mode="r"))]
    fn new(file: PyObject, mode: &str, py: Python<'_>) -> PyResult<Self> {
        let file = match file.downcast_bound::<PyString>(py) {
            Ok(file) => {
                let file = file.to_cow()?;
                File::open(&*file)?
            }
            Err(_error) => {
                return Err(PyValueError::new_err(
                    "ZipFile file currently must be a string",
                ));
            }
        };

        let file = match mode {
            "r" => ReadZipFile::new(file)?,
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
        };

        Ok(Self { file })
    }

    /// Close the archive file.
    pub fn close(&mut self) {
        self.file.close()
    }

    #[pyo3(signature = (name, mode="r", pwd=None))]
    pub fn open(
        &mut self,
        name: &str,
        mode: &str,
        pwd: Option<Bound<'_, PyBytes>>,
    ) -> PyResult<ReadZipExtFile> {
        match mode {
            "r" => self.file.open(name, pwd),
            "w" => Err(PyNotImplementedError::new_err(
                "open() currently requires mode \"r\"",
            )),
            _ => Err(PyNotImplementedError::new_err(
                "open() requires mode \"r\" or \"w\"",
            )),
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
