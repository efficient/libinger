#ifndef THREADS_H_
#define THREADS_H_

#ifndef __STDC_NO_THREADS__
// Defer to the SYStem header.  This works as long as there's no sys/ folder in our tree.
# include <sys/../threads.h>
#else
# define thread_local _Thread_local
#endif

#endif
