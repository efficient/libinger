_libgotcha_
===========
This is the source code of the _libgotcha_ runtime for making otherwise shared libraries selfish,
as described in our ATC '20 paper, "Lightweight Preemptible Functions."  At load time of any process
that depends on it, this library opens a bunch (specifically, `NUM_SHADOW_NAMESPACES`) of copies of
all of the application's dependencies ("libsets") and modifies the program's GOTs (global offset
tables) to allow itself to intercept dynamic function calls and route them to a different libset as
determined by a setting called the thread's group (pronounced "next libset").  It also intercepts
accesses to global variables via dynamic symbols and uses a custom segmentation fault handler to
reroute those.

It may sound like _libgotcha_ does a lot of setup work, and that's because it does.  Sadly, since
the next libset starts off set to 0 (the "starting libset"), all function calls and variable
references always resolve to the original program and the libraries it would've loaded even without
_libgotcha_, so none of that setup work has any affect at all.  That is, unless a control library
is present to change the next libset during the program's run...  Presumably, such a control library
would also have its own idea of what it wanted to use libsets _for_.

You can also load _libgotcha_ into a program that doesn't even know about it by using `$LD_PRELOAD`!
But that would be _truly_ useless because it would imply that the program didn't depend on a control
library that could do anything interesting with _libgotcha_.  Of course, I suppose you might preload
a control library, too...


License
-------
The entire contents and history of this repository are distributed under the following license:
```
Copyright 2020 Carnegie Mellon University

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```


Types of control libraries
--------------------------
There are two possible types of control library:
 * An _internal control library_ is statically linked with _libgotcha_ to form a single shared
   object file that is loaded (or preloaded) as one unit.  In addition to having access to the
   _libgotcha_ control API (declared in `libgotcha_api.h`), internal libraries enjoy the guarantee
   that their code always executes in the base libset, and have direct access to the C library
   functions deemed by _libgotcha_ to be too dangerous for the rest of the program to call
   (enumerated in `libgotcha_repl.h`).
 * An _external control library_ is dynamically linked with _libgotcha_, and therefore constitutes
   a separate shared object file that depends on `libgotcha.so` (or, perhaps, a compatible internal
   control library).  Calls into such a library do not cause an automatic libset switch unless
   explicitly whitelisted (which is currently not pluggable and done via `whitelist.c`), and the
   library does not have special access to dangerous C library functions.  External control
   libraries haven't been tested as extensively as internal ones, but are necessary when you want to
   have more than one control library in a single application, if you want to be able to update
   _libgotcha_ without rebuilding the control library, and maybe for other reasons yet to be
   discovered.

If an internal control library needs to alter the behavior of any additional third-party library
functions, it may designate them as dangerous by defining its own implementation of the same name.
Calls to the function from the rest of the process image will be rerouted to this implementation,
but calls from the control library (and _libgotcha_ itself) will be routed to the original
third-party implementation.  For more details, see the documentation in `libgotcha_api.h`, the
advice about compiler optimizations in `libgotcha_repl.h`, and the example control libraries in the
projects under the `example` directory.

Note that internal control libraries' calls to _existing_ dangerous C library functions are also
routed directly to the real implementation, but the control library may instead choose to call
_libgotcha_'s safer wrapper.  A control library might even use this feature to wrap _libgotcha_'s
own wrapper of a third-party library function!  These use cases are considered advanced: see the
documentation in `libgotcha_repl.h`, and be prepared for some additional build system complexity
(e.g., our Make integration supports replacing calls to `cp` via the `$(CP)` variable, which could
be used to inject an intermediate `objcopy` step into the build process).


Building _libgotcha_
--------------------
The runtime may be built using either Make or Cargo, but only the former supports all features.

To build everything using Make, do one of:
```
$ make #release build
$ make ASFLAGS="-g" CFLAGS="-g3 -Og" RUSTFLAGS="-g -Copt-level=0" #debug build
```

This will deposit the following output files in the root of the tree:
 * `libgotcha.a`: static library for embedding into _internal_ control libraries written in C
 * `libgotcha.rlib`: static library for embedding into _internal_ control libraries written in Rust
 * `libgotcha.so`: shared library for use by _external_ control libraries written in any language
 * `libgotcha.mk`: helper makefile to support building dependent projects with Make

To build `libgotcha.rlib` using Cargo, do one of:
```
$ cargo build --release #release build
$ cargo build #debug build
```

In this case, the output file is under `target/release` or `target/debug`.  Note that, if
the tree is located within a Cargo workspace, the `target` directory will be created in the root of
that workspace instead.

Building control libraries against _libgotcha_
----------------------------------------------
Building a control library requires a great many compiler and linker flags that we do not endeavor
to enumerate herein.  Instead, we provide integrations for building them with either the Make or
Cargo build system.  An example of each type of project can be found in the `examples` subdirectory.

Note that the control libraries themselves under the `examples` directory are not terribly
instructive: rather than actually using the _libgotcha_ control API, they merely demonstrate forced
wrapping of third-party library functions.
