from typing import Self
from types import TracebackType

ZIP_STORED: int
ZIP_DEFLATED: int
ZIP_BZIP2: int
ZIP_LZMA: int

class ZipInfo:
    filename: str
    compress_type: int
    compress_level: int | None

class ZipExtFile:
    def read(self) -> bytes: ...
    def write(self, buffer: bytes) -> None: ...
    def close(self) -> None: ...
    def __enter__(self) -> Self: ...
    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_value: BaseException | None,
        traceback: TracebackType | None,
    ) -> None: ...

class ZipFile:
    def __init__(
        self,
        file: str,
        mode: str = "r",
        compression: int = ZIP_STORED,
        allowZip64: bool = True,
        compresslevel: int | None = None,
    ) -> None: ...
    def close(self) -> None: ...
    def open(
        self, name: str | ZipInfo, mode: str = "r", pwd: bytes | None = None
    ) -> ZipExtFile: ...
    def namelist(self) -> list[str]: ...
    def __enter__(self) -> Self: ...
    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_value: BaseException | None,
        traceback: TracebackType | None,
    ) -> None: ...
