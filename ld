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

		if [ -n "$LIBINGER_LINKER" ]
		then
			interp="-Wl,-I`readlink -f "$LIBINGER_LINKER"`"
		fi

		break
	fi
done

exec cc $interp "$@"
