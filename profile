#!/bin/sh

if [ "$#" -lt "2" ]
then
	cat <<-tac
		USAGE: $0 <test> <profiler>...

		<test> is which benchmark to run (or an empty string for all)
		<profiler> is the profiler and command-line arguments, e.g.,:
		  time
		  strace -c
		  valgrind --tool=callgrind --collect-atstart=no --toggle-collect=bencher::Bencher::iter

		Only release builds of the inger benchmark are supported.  Note that you probably
		want to enable debug symbols by adding the following to Cargo.toml before building:
		  [profile.bench]
		  debug = true
		  [profile.release]
		  debug = true
	tac
	exit 1
fi

test="$1"
shift

LIBGOTCHA_NOGLOBALS= LD_LIBRARY_PATH=target/release/deps exec "$@" target/release/inger-*[!.]? $test
