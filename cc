#!/bin/sh

args=""
for arg in "$@"
do
	case "$arg" in
	-Wl,--version-script*)
		;;
	*)
		args="$args '$arg'"
	esac
done

arg0="`basename "$0"`"
eval exec "'$arg0'"$args
