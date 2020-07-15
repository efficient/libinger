_libtimetravel_
===============
This is _libtimetravel_, a library for making it easier to safely use unstructured control flow in
Rust.  Language features such as RAII make this rather difficult to reason about directly, and
consequently mean that it is quite easy to introduce horrible and very nonobvious memory errors.
Fundamentally, the library is a wrapper around POSIX contexts that imposes a bit more structure and
also performs runtime checks.  However, it also adds at least one nifty trick _not_ available with
vanilla contexts: the ability to switch back into the context passed to a signal handler, which is
accomplished by asserting another signal and then replacing that handler's context with that of the
old handler!  No proof of correctness is (or will be) provided, so it's likely you can still shoot
yourself in the foot if you try to do anything too radical; in particular, the runtime checks are
disabled in release builds.

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
