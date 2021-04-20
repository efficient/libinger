#!/bin/sh

cc="cc"
interp=""
for arg in "$@"
do
	case "$arg" in
	-linger)
		symlink="`dirname "$0"`/`basename "$0"`.so"
		if [ -z "$LIBINGER_LINKER" ] && readlink -e "$symlink" >/dev/null
		then
			LIBINGER_LINKER="$symlink"
		fi

		if [ -n "$LIBINGER_LINKER" ]
		then
			interp="-Wl,-I`readlink -f "$LIBINGER_LINKER"`"
		fi
		;;
	*/libgotcha-*.rlib)
		cc="`dirname "$0"`/cc"
		;;
	esac
done

exec "$cc" $interp "$@"
