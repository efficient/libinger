#!/bin/sh

if [ -z "$LIBINGER_LINKER" ]
then
	LIBINGER_LINKER="`dirname "$0"`/ld.so"
fi
LIBINGER_LINKER="`readlink -f "$LIBINGER_LINKER"`"

if [ "`basename "$0"`" = "ld" ]
then
	exec ld "$@"
elif [ -n "$LIBINGER_LINKER" ]
then
	exec cc "-Wl,-I$LIBINGER_LINKER" "$@"
else
	exec cc "$@"
fi
