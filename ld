#!/bin/sh

interp=""
for arg in "$@"
do
	if [ "$arg" = "-linger" ]
	then
		if [ -z "$LIBINGER_LINKER" ]
		then
			LIBINGER_LINKER="`dirname "$0"`/ld.so"
		fi
		interp="-Wl,-I`readlink -f "$LIBINGER_LINKER"`"
		break
	fi
done

exec cc $interp "$@"
