mod read;
mod write;

use self::read::ReadZipExtFile;
use self::read::ReadZipFile;
use self::write::WriteZipFile;
use crate::write::WriteZipExtFile;
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::exceptions::PyNotImplementedError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use pyo3::types::PyString;
use pyo3::types::PyStringMethods;
use std::fs::File;

const ZIP_STORED: u8 = 0;
const ZIP_DEFLATED: u8 = 8;
const ZIP_BZIP2: u8 = 12;
const ZIP_LZMA: u8 = 14;

create_exception!(nd_zip, BadZipFile, PyException, "File is not a zip file");

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum CompressionKind {
    Stored,
    Deflated,
    Bzip2,
    Lzma,
}

impl TryFrom<u8> for CompressionKind {
    type Error = PyErr;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            ZIP_STORED => Ok(Self::Stored),
            ZIP_DEFLATED => Ok(Self::Deflated),
            ZIP_BZIP2 => Ok(Self::Bzip2),
            ZIP_LZMA => Ok(Self::Lzma),
            _ => Err(PyNotImplementedError::new_err(format!(
                "{value} is not a known compression type"
            ))),
        }
    }
}

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
    ) -> PyResult<ZipExtFile> {
        match (self, mode) {
            (Self::Read(file), "r") => Ok(ZipExtFile {
                inner: ZipExtFileInner::Read(Box::new(file.open(name, pwd)?)),
            }),
            (Self::Read(_file), "w") => Err(PyValueError::new_err("archive opened as read-only")),
            (Self::Write(_file), "r") => Err(PyValueError::new_err("archive opened as write-only")),
            (Self::Write(file), "w") => {
                if pwd.is_some() {
                    return Err(PyNotImplementedError::new_err(
                        "writing encrypted files is currently not supported",
                    ));
                }

                Ok(ZipExtFile {
                    inner: ZipExtFileInner::Write(file.open(name)?),
                })
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
    #[pyo3(signature = (file, mode="r", compression=ZIP_STORED, allowZip64=true, compresslevel=None), text_signature = "(file, mode=\"r\", compression=ZIP_STORED, allowZip64=True, compressionlevel=None)")]
    fn new(
        file: PyObject,
        mode: &str,
        compression: u8,
        // Follow original python api
        #[allow(non_snake_case)] allowZip64: bool,
        compresslevel: Option<u8>,
        py: Python<'_>,
    ) -> PyResult<Self> {
        if !allowZip64 {
            return Err(PyNotImplementedError::new_err(
                "allowZip64 must currently always be true",
            ));
        }

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
                let compression_kind = CompressionKind::try_from(compression)?;

                ZipFileInner::Write(WriteZipFile::new(file, compression_kind, compresslevel)?)
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
    ) -> PyResult<ZipExtFile> {
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

enum ZipExtFileInner {
    Read(Box<ReadZipExtFile>),
    Write(WriteZipExtFile),
}

#[pyclass(unsendable)]
pub struct ZipExtFile {
    inner: ZipExtFileInner,
}

#[pymethods]
impl ZipExtFile {
    pub fn read(&mut self) -> PyResult<Vec<u8>> {
        match &mut self.inner {
            ZipExtFileInner::Read(file) => file.read(),
            ZipExtFileInner::Write(_file) => Err(PyNotImplementedError::new_err(
                "Attempted to read to a write-only ZipExtFile",
            )),
        }
    }

    pub fn write(&mut self, buffer: &[u8]) -> PyResult<()> {
        match &mut self.inner {
            ZipExtFileInner::Read(_file) => Err(PyNotImplementedError::new_err(
                "Attempted to write to a read-only ZipExtFile",
            )),
            ZipExtFileInner::Write(file) => file.write(buffer),
        }
    }

    pub fn close(&mut self) {
        match &mut self.inner {
            ZipExtFileInner::Read(file) => file.close(),
            ZipExtFileInner::Write(file) => file.close(),
        }
    }

    pub fn __enter__<'p>(this: PyRef<'p, Self>, _py: Python<'p>) -> PyResult<PyRef<'p, Self>> {
        Ok(this)
    }

    pub fn __exit__(&mut self, _exc_type: PyObject, _exc_value: PyObject, _traceback: PyObject) {
        match &mut self.inner {
            ZipExtFileInner::Read(file) => file.__exit__(),
            ZipExtFileInner::Write(file) => file.__exit__(),
        }
    }
}

#[pymodule]
#[pyo3(name = "nd_zipfile")]
fn nd_zipfile(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("ZIP_STORED", ZIP_STORED)?;
    m.add("ZIP_DEFLATED", ZIP_DEFLATED)?;
    m.add("ZIP_BZIP2", ZIP_BZIP2)?;
    m.add("ZIP_LZMA", ZIP_LZMA)?;
    m.add_class::<ZipFile>()?;
    Ok(())
}
