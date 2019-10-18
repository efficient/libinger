#!/bin/sh

interp=""
for arg in "$@"
do
	if [ "$arg" = "-linger" ]
	then
		symlink="`dirname "$0"`/`basename "$0"`.so"
		if [ -z "$LIBINGER_LINKER" ] && readlink -e "$symlink" >/dev/null
		then
			LIBINGER_LINKER="$symlink"
		fi

		if [ -n "$LIBINGER_LINKER" ]
		then
			interp="-Wl,-I`readlink -f "$LIBINGER_LINKER"`"
		fi

		break
	fi
done

exec cc $interp "$@"
