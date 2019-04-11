// Despite claiming to be a self-contained, modular library, elfutils's libasm requires the use of
// Ebl structures from the internal and unstable libebl.  This limitation carries three significant
// disadvantages in addition to the lack of portability across versions:
//  * Some distributions don't package libebl.h and/or libebl.a, which makes building a pain because
//    they're required to #include and make use of libebl's facilities.
//  * The whole elfutils family is cross-licensed under the GPL and LGPL, but libebl is only
//    available as a static library, so linking against it effectively GPLs the client code.
//  * The necessary libebl interfaces are really just a thin shim over functionality contained in
//    architecture-specific shared libraries that are usually distributed alongside libasm.so (or
//    with a dependency thereof), but they load and unload this library at runtime.
// We can avoid these limitations while only giving up architecture agnosticism and the tiniest
// shred of interface portability by calling directly into the x86-64 shared support library.  To
// enable this, simply add this file to your *system* include path (for portability to development
// systems that do supply the header) before #include'ing libasm.h and link against libebl_x86_64
// with your output object file's RUNPATH set to e.g., /local/x86_64-linux-gnu/elfutils.

#ifndef LIBEBL_H_
#define LIBEBL_H_

#include <gelf.h>
#include <stddef.h>
#include <stdint.h>

#define X86_64_MAX_INSTR_LEN 15

// This number needs to be at least as large as the size of libebl's opaque Ebl structure.
typedef uint8_t Ebl[512];

// The first two parameters are not required.  Be sure to zero out your Ebl before invoking this
// function, and use sizeof on it to determine the last argument.
const char *x86_64_init(Elf *, GElf_Half, Ebl *, size_t);

#endif
