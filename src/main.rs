extern crate inger;

use inger::ffi::Linger;
use std::mem::size_of;

fn main() {
	print!(
		"\
		#ifndef LIBINGER_H_\n\
		#define LIBINGER_H_\n\
		\n\
		#include <stdbool.h>\n\
		#include <stdint.h>\n\
		\n\
		typedef struct {{\n\
			bool is_complete;\n\
			uint8_t continuation[{}];\n\
		}} linger_t;\n\
		\n\
		linger_t launch(void (*)(void *), uint64_t, void *);\n\
		void resume(linger_t *, uint64_t);\n\
		void cancel(linger_t *);\n\
		\n\
		#endif\n\
		",
		size_of::<Linger>(),
	);
}
