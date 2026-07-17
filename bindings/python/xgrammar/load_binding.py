"""Load the generated pure-Rust xgrammar bindings."""

import os

if os.environ.get("XGRAMMAR_BUILD_DOCS") == "1":
    LIB = None
else:
    from . import xgrammar_rs as LIB
