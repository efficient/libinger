_libinger_
==========
This is the source code of the _libinger_ library for making function calls with timeouts from Rust
and C, described in our ATC '20 paper, "Lightweight Preemptible Functions."  _As the proverb goes,
"If you don't want your function calls to linger, link with `-linger`."_

Also present are a few supporting libraries:
 * `external/libgotcha`: runtime providing the libset abstraction that makes all of this possible
 * `external/libtimetravel`: makes it easier to safely perform unstructured jumps from Rust code
 * `internal/libsignal`: unsafe wrappers around C library functions (don't use at home!)


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


System requirements
-------------------
Presently, x86-64 GNU/Linux is the only supported platform.  The build requirements are as follows:
 * `rustc` ≥1.36.0 (versions starting with 1.37.0 include a breaking change to dylib symbol exports—
   https://github.com/rust-lang/rust/issues/64340 —but the build system now implements a workaround)
 * `cargo` ≤1.39.0/"0.40.0" or ≥1.55.0/"0.56.0" (a version between these introduced a breaking
   but since fixed regression in path resolution: https://github.com/rust-lang/cargo/issues/8202)
 * `bindgen` ≤0.52 (or a newer version with a wrapper script that passes the `--size_t-is-usize`
   flag, as per https://github.com/rust-lang/rust-bindgen/issues/1901)
 * `gcc` (tested with 9.2.1)
 * GNU `make`
 * GNU binutils (tested with 2.34)
 * GNU libc ≤2.33 (versions starting with 2.30 include a breaking change to `dlopen()` behavior on
   executables—https://sourceware.org/bugzilla/show_bug.cgi?id=24323 —but libgotcha now implements a
   workaround; versions starting with 2.34 combine libpthread into libc, which is unlikely to work
   out of the box)
 * `libebl` from elfutils 0.176 (newer versions eliminate the libebl shared library, which we
   currently depend on directly)
 * `git`
On Debian-based systems, bindgen appears to require the libclang-dev package; if it is missing, you
will get errors about missing headers.  If you don't want to install this package, you may be able
to work around it by symlinking /usr/local/include/asm to /usr/include/asm.


A note on terminology
---------------------
We use the term lightweight preemptible function (LPF) to refer to the timed _version_ of a
function, as invoked via the `launch()` wrapper function in this library.  It's not quite right to
say that _libinger_ "provides preemptible functions"; rather, it provides a transformation from
an ordinary function into a preemptible one.

To provide the memory isolation necessary to introduce preemption and asynchronous cancellation at
sub-thread granularity without breaking existing program dependencies, the _libgotcha_ runtime
allocates a separate copy of all the program's loaded dynamic libraries for each preemptible
function.  While the paper refers to this isolation unit as a libset, that term was unfortunately
coined late in development; as such, the source code and configuration variables refer to it as a
"group" instead.


A note on design
----------------
The paper describes the _libinger_ API, and the generated HTML documentation gives a few more usage
details.  Here are some of the guiding principles that inspired our interface choices:
 * **We do not assume users need asynchrony.**  Hence, preemptible functions _run on the same kernel
   thread as their caller_.  This is good for performance (and especially invocation latency), but
   it is also important to be aware of; for instance, it means that a preemptible function will
   deadlock if it attempts to acquire a lock held by its caller, or vice versa.  If asynchrony is
   something you require, you can build it atop _libinger_, as we have demonstrated with our
   _libturquoise_ preemptive userland thread library.
 * **We assume that simply calling a function with a timeout is the common use case.**  As such, the
   `launch()` wrapper both constructs and begins executing the preemptible function rather than
   asking the user to first employ a separate constructor.  The latter behavior can be achieved by
   passing the sentinel `0` as the timeout, then later using `resume()` to invoke the preemptible
   function.
 * **We endeavor to keep argument and return value passing simple yet extensible.**  Because Rust
   supports closures, the Rust version of `launch()` accepts only nullary functions: those seeking
   to pass arguments should just capture them from the environment.  Because C supports neither
   closures nor generics, the C version of `launch()` accepts a single `void *` argument that can
   serve as an inout parameter; it occupies the last position in the parameter list to permit
   (possible) eventual support for variable argument lists.
 * **We choose defaults to favor flexibility and performance.**  When a preemptible function times
   out, _libinger_ assumes the caller might later want to resume it from where it left off.  As
   such, both `launch()` and `resume()` pause in this situation; this incurs some memory and time
   overhead to provide a separate execution stack and package the continuation object, but has much
   lower overall cost than asynchronous cancellation.  If the program does require cancellation, it
   can request it explicitly by calling `cancel()` (C) or dropping the continuation object (Rust).
 * **We provide preemption out of the box, but the flexibility to cooperatively yield.**  The
   `pause()` primitive allows a preemptible function to "yield" back to its caller by immediately
   "timing out."  One can imagine building higher-level synchronization constructs atop this; for
   example, a custom mutex that paused instead of blocking would allow two or more preemptible
   functions to share state, even when some of them executed from the same kernel thread.
 * **We favor a simple, language-agnostic interface.**  Because the interface is based on the
   foundational function call abstraction, it looks very similar in both C and Rust.  Someday, it
   may look _equally_ similar in other languages as well, and in the meantime, it ought to enjoy
   compatibility with languages' C foreign-function interfaces.  It's relatively simple to integrate
   higher-level abstractions on top, such as the Rust futures wrapper available in _libinger_'s
   `future` module.


A note on implementation
------------------------
The timer signal handler in _libinger_ refuses to pause while the next libset is set to 0 (the
starting libset).  Because _libinger_ is statically linked with _libgotcha_, the latter enforces a
transparent switch to this libset whenever a dynamic function call transfers control into the module
in the process image that corresponds to `libinger.so`.  This means that preemption is deferred on a
given kernel thread while _libinger_'s own code is executing on that thread.

Of course, things are not quite that simple.  There are noteworthy exceptions to the rule:
 * The public _libinger_ Rust interface includes a number of generic functions.  Because the Rust
   compiler monomorphizes such functions for the client code that uses them, _their implementations
   are_ not _in `libinger.so`!_  Rather, there is, roughly speaking, one or more copies of them
   (specialized for various type arguments) in each program module that calls them.  The generic
   functions are therefore implemented such that they package everything that differs by type, then
   call into non-specialized functions such as `setup_stack()` and `switch_stack()` to do the scary
   stuff.
 * The `resume_preemption()` function is installed as a _libgotcha_ callback hook, and is implicitly
   invoked at the end of each deferred-preemption library call made by a preemptible function.
   _This happens in the preemptible function's libset rather than the starting one_; this is
   essential because the callback's main task is to force the timer signal handler to run
   _immediately_ and check for a timeout, and we don't want the libset to inhibit preemption!


Building glibc from source
--------------------------
_Note: If you seek only to test_ libinger _with a very small program that doesn't require many
preemptible functions and has few dynamic library dependencies, and you are willing to build
without debug symbols, you may be able to skip this section provided you restrict the number of
libsets at runtime by exporting the `$LIBGOTCHA_NUMGROUPS` variable before invoking your program
(e.g., `$ LIBGOTCHA_NUMGROUPS=1 ./i_only_use_one_lpf_at_a_time`)._

Although _libinger_ is compatible with an unmodified glibc in principle, in practice the build
configuration used by most distributions is insufficient for two reasons:
 * By loading numerous copies of the application's libraries, we tend to exhaust the static storage
   pool provided by the dynamic linker.  If your program hits this limit, it will crash at load time
   with an error like: `yourprogram: libgotcha error: Unable to load ancillary copies of library:
   somelibrary.so: cannot allocate memory in static TLS block`.
 * Stock glibc builds are limited to 16 linker namespaces, enough to support only 15 preemptible
   functions at any given time.  If your program hits this limit, it will crash at runtime with an
   error like: `launch(): too many active timed functions`.

Unfortunately, these configuration parameters are baked into the dynamic linker at build time.
What's more, changing (at least) the latter alters the size of internal structures that are shared
between `ld-linux.so`, `libc.so`, `libpthread.so`, and others, so making changes requires rebuilding
all of glibc.  Fortunately, provided you set the prefix properly when building glibc, the dynamic
linker will know where to search (by absolute path) for the other libraries; as such, most
applications that depend on `libinger.so` need only define a custom interpreter path pointing to the
`ld-linux-x86-64.so.2` file in your build directory.

Note that glibc 2.32 eliminated the below `TLS_STATIC_SURPLUS` compile-time constant and replaced it
with a runtime tunable read from the environment.  Debian backported this change to their glibc 2.31
distribution.  If you are on an affected version, disregard the related steps below; instead, export
something like `GLIBC_TUNABLES=glibc.rtld.optional_static_tls=0x10000` when running your program.

Follow these steps to build your own glibc, where all paths are relative to the root of this
repository:
 1. Clone the glibc source code: `$ git clone git://sourceware.org/git/glibc`.  I'll assume you put
    it in `../glibc`, and have checked out whatever version you want to use.
 1. Edit `TLS_STATIC_SURPLUS` in `../glibc/elf/dl-tls.c` to raise the multiplier on `DL_NNS` by at
    least two orders of magnitude*.  Our current recommendation is a multiplier of 2000.
 1. Change `DL_NNS` in `../glibc/sysdeps/generic/ldsodefs.h` to exceed by at least one the maximum
    number of simultaneous preemptible functions your program will need*.  The default value of 16
    should be fine for use cases with low parallelism and where preemptible functions are usually
    allowed to run to completion before launching others; we have not tested values above 512.
 1. Make a new empty directory to use for the build and `cd` into it.  Let's say this is located at
    `../rtld`.
 1. Decide where you want to place your new glibc "installation."  I'll be using `../rtld/usr`.
 1. Still working in `../rtld`, configure the build:
    `$ ../glibc/configure --prefix=$PWD/usr --disable-werror`.
 1. Start the build: `$ make -j8`, assuming you have 8 cores.
 1. When the build finishes successfully, run `$ make install` to populate the "installation"
    directory you chose earlier.  Note that the `install` target appears to be finicky about running
    with multiple cores, so combining this step with the previous one can cause issues (at least)
    the first time you build glibc.
 1. Add to the `../rtld/usr/lib` directory a symlink to _each_ of the following libraries on your
    system: `libasm.so.1` (from elfutils), `libbz2.so.1.0`, `libdw.so.1` (also from elfutils),
    `libgcc_s.so.1`, `liblzma.so.5`, `libstd-*.so` (from Rust), and `libz.so.1`.
 1. Move back into the root of this repository and tell the build system where to find your custom
    dynamic linker build: `ln -s ../rtld/usr/lib/ld-linux-x86-64.so.2 ld.so`.

\* The `inger-atc20-cfp` tag records the revision used to run the _libinger_ microbenchmarks for the
   ATC paper, and is annotated with the build customizations applied to glibc and _libgotcha_ for
   that use case.  Similarly, the repository containing our full-system benchmarks contains
   annotated tags recording the configuration used to evaluate _hyper_ and _libpng_.  To see the
   annotations, use `git show`.  To run the benchmarks, use `cargo bench`.


Building _libinger_
-------------------
The _libinger_ library is built with Cargo, although Make is also required because of _libgotcha_.
 1. Follow the steps in the preceding section to build your own glibc root, or skip them at your own
    peril (in which case you will have to ignore the error in the following step).
 1. Working in the root of this repository, configure the build: `$ ./configure`.
 1. Build the library: `$ cargo build --release`.
 1. If you intend to use preemptible functions from C, generate the header:
    `$ cargo run --release >libinger.h`

The `libinger.so` library will be located in `target/release`; this and the header are the only
files required to build C programs against _libinger_.  For Rust programs, the `rlib` files under
`target/release/deps` must also be present during the build phase.

To build and view the HTML documentation for the Rust interface, simply do: `$ cargo doc --open`.
There is no documentation for the C interface, but the header should be "self documenting," _as they
say_.


Building programs against _libinger_
------------------------------------
_Note: This section describes how to build simple C and Rust programs against_ libinger _by invoking
the compiler directly.  If you want to use Cargo to build a Rust crate that depends on_ libinger _,
skip to the next one._

Let's assume you want to write a small program that uses _libinger_, and (for simplicity) that you
want to store the source file and executable in the root of this repository.

For a C program, you'll want to begin your file with `#include "libinger.h"` and build with some
variation of this command:
```
$ cc -fpic -Ltarget/release -Wl,-R\$ORIGIN/target/release -Wl,-I./ld.so -o prog prog.c -linger
```

For a Rust program, you should start your file with `extern crate inger;` and build like:
```
$ rustc -Ltarget/release/deps -Clink-arg=-Wl,-R\$ORIGIN/target/release -Clink-arg=-Wl,-I./ld.so prog.rs
```

Of course, you probably want to add other flags to your compiler invocation to request things like
language edition, optimization, debugging, and warnings.  If you are trying to use the system
dynamic linker instead of one that you built from source, omit the [`-Clink-arg=`]`-Wl,-I./ld.so`
switch, cross your fingers, and eat a bowl of lucky charms to minimize the probability that your
program runs out of static storage during initialization.

In the case of both languages, the resulting executable is distributable, and will continue to work
as long as it is run from a directory containing both `ld.so` and `target/release/libinger.so`.
Note however, that the dynamic linker pointed to by the former symlink is hard to distribute without
an installer because its default library search path is based on the (absolute) prefix path used
when configuring the build.


Building crates against _libinger_ with Cargo
---------------------------------------------
Cargo can build crates that depend on _libinger_, but this requires some extra configuration, not
least because it very aggressively prefers to build dependencies as static libraries rather than
shared ones.  To set things up, after configuring the build system in this repository, do the
following from the root of your other Cargo tree:
```
$ ln -s ../path/to/libinger/.cargo
```

If you have a nested tree of projects that depend on _libinger_, it suffices to do this once at the
outermost level of the directory structure (but still within a Cargo project directory!).

Now you'll just need to add an entry like this to your project's `Cargo.toml`:
```
[dependencies.inger]
path = "../path/to/libinger"
```


Debugging tips
--------------
If adding _libinger_ to your program causes it to segfault or otherwise crash, it's possible the
culprit is _libgotcha_'s support for intercepting dynamic accesses to global variables, which makes
use of heuristics.  Ordinarily we notify the application of accesses we were unable to resolve by
forwarding it a segmentation fault; this approach is intended to support applications and runtimes
that respond to segfaults, but it can sometimes obscure the problem.  See whether running your
program with forwarding disabled gives a more informative _libgotcha_ error:
```
$ LIBGOTCHA_ABORTSEGV= ./yourprogram
```

If you have produced a release build and later need to switch to a debugging one, please note that
**the build system does not support both types of builds simultaneously**: you must perform a full
clean in between by running `$ ./configure`.  If you are using the C interface, also note that
**you must regenerate the header without the `--release` switch**; otherwise, the type declaration's
size will not match the implementation!  Unless _your_ application uses Cargo as a build system,
you'll need to replace all instances of `release` with `debug` in the command you use to build it.
If substituting a debug build of _libinger_ causes your program to crash on initialization with an
error like `Unable to load ancillary copies of library`, either rebuild glibc with a higher
`TLS_STATIC_SURPLUS` or launch with a reduced number of libsets, e.g.:
```
$ LIBGOTCHA_NUMGROUPS=2 ./yourprogram
```

The `valgrind` suite (at least Memcheck and Callgrind from version 3.16.1) is known to work, but
currently conflicts with _libgotcha_'s global variable access interception.  You can work around
this by switching it off, at the risk of altering the semantics of your program:
```
$ LIBGOTCHA_NOGLOBALS= valgrind ./yourprogram #check it for memory errors
$ LIBGOTCHA_NOGLOBALS= valgrind --tool=callgrind ./yourprogram #profile it
```

Sadly, LLVM's sanitizers rely heavily on dynamic linking tricks that are incompatible with
_libgotcha_, so they are not available at this time.

The `gdb` debugger works both with and without global variable access interception, although we
recommend first trying without it for a much less confusing experience out of the box:
```
$ LIBGOTCHA_NOGLOBALS= gdb ./yourprogram
```

If you do need to preserve global variable semantics while debugging, you probably don't want to
step into _libgotcha_'s segfault handler unless you're trying to debug the feature itself.  You can
instruct `gdb` to ignore segmentation faults like so:
```
$ gdb -ex handle\ SIGSEGV\ noprint ./yourprogram
```

The `rr` reverse-debugging backend (at least version 5.4.0) also works both with and without global
variable access interception, but conflicts with _libgotcha_'s interception of certain calls into
the runtime dynamic loader.  Invoke it like so (omitting `LIBGOTCHA_NOGLOBALS=` if desired):
```
$ LIBGOTCHA_NODYNAMIC= LIBGOTCHA_NOGLOBALS= rr ./yourprogram #record execution for reverse debugging
```

If you step into code located outside the main libset, GDB will be missing symbol information, and
therefore unable to display backtraces or source code, or apply your symbol breakpoints.  Luckily,
_libgotcha_ includes a debugger script to fix this problem.  You must have the glibc sources
corresponding to the `ld.so` build you are using.  Add the following to your GDB arguments however
you are invoking it (via `gdb`, `rr replay`, etc.):
```
-x .../path/to/libinger/external/libgotcha/libgotcha.gdb -ex dir\ .../path/to/glibc/dlfcn:.../path/to/glibc/elf
```

When debugging the program directly with GDB rather than from a recorded rr execution capture, the
debugger and/or program may become overwhelmed by the extremely frequent preemption signals.  If the
debugger freezes or the program doesn't make progress when single stepping, try recompiling
_libinger_ with a higher (say, by an order of magnitude or so) `QUANTUM_MICROSECONDS` value.  If you
still have trouble single stepping or the time spent stopped is causing _libinger_ to preempt the
task you are trying to debug, you can disable preemption altogether by issuing a variation of the
following GDB command to cover the preemption signal(s) affecting your task's execution:
```
(gdb) handle SIGALRM nopass
```


Troubleshooting
---------------
**My distribution ships a version of glibc that is newer than 2.33.  What should I do?**

Such versions are untested.  If you do not encounter any obvious issues, great!  Otherwise, no need
to modify your installation; instead, just follow the steps under _Building glibc from source_ above
to build version 2.33.  When linking your executable, you will want to use GCC's `-Wl,-I` switch
(`-Clink-arg=-Wl,-I` in the case of rustc) to specify the newly-symlinked dynamic linker as the
program's interpreter.  If you are building the program using Cargo and its source is located in
tree or you have symlinked its `.cargo` to this repository's folder of the same name, passing the
flag should happen automatically.  Note that, if you get any version errors when running the
resulting binary, you may need to add a further `-L` and the path to the `lib` directory generated
by the glibc `install` target to your linker flags and rebuild.

**My distribution ships a version of elfutils that is newer than 0.176.  What should I do?**

It probably does!  The easiest thing to do is to downgrade the package.  For instance, if using
Debian, download (at least) the following binary packages from https://snapshot.debian.org:
`libasm1`, `libasm-dev`, `libdw1`, `libelf1`, and `libelf-dev`.  Put the packages into a new
directory, `cd` into it, and run `apt -s install ./*.deb`.  If it shows any dependency errors,
download additional packages to address them and add them to the directory; you will probably need
`gdb` version 9.2-1, `libpython3.8`, `libpython3.8-minimal`, and `libpython3.8-stdlib`.  Then
perform the downgrade by running `sudo apt install ./*.deb`.

**I get the assertion: `ELF64_ST_TYPE(st->st_info) != STT_GNU_IFUNC)`!**

This appears to occur on some Ubuntu systems because of the way Canonical packages zlib (`libz.so`).
The easiest workaround is to grab a closely matching package version from
https://packages.debian.org and extract the shared library file.  Then either export the
`$LD_LIBRARY_PATH` environment variable to the path to its containing folder when executing your
application, or just place the shared library in the `lib` directory generated by glibc's `install`
target if/when you built it from source.

**I get some other error at some point.**

Start by rerunning `./configure` to clean the build.  Make sure it finds all the dependencies.  Look
back at _System requirements_ and verify that you are not on a `cargo` version affected by the
regression, and that you have the described wrapper script in your `$PATH` if you have a
sufficiently new version of `bindgen` (a shell alias will not suffice).

Next, try building _libgotcha_ in isolation.  Just `cd` into `external/libgotcha` and type `make`
(without passing `-j`).  If you get errors here, go back to the previous paragraph, pinch yourself,
and check all that one more time.

If you get this far, try building _libinger_ without per--preemptive function thread-local variables
by passing `--features notls` to your Cargo command.  If you are still stuck, see whether you can
trace the source of the error using any of the techniques under _Debugging tips_ above.
