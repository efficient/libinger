#include "../libinger.h"

#ifdef BREAKDOWN
# include <gperftools/profiler.h>
#endif
#include <sys/resource.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>
#include <time.h>

#ifdef BREAKDOWN
# define ITERS 75
#else
# define ITERS 1000000
#endif

#ifdef BREAKDOWN
# pragma weak ProfilerFlush
# pragma weak ProfilerStart
# pragma weak ProfilerStop
#endif

struct bothtimes {
	unsigned long usr;
	unsigned long sys;
	long pfs;
};

static unsigned long nsnow(void) {
	struct timespec tv;
	clock_gettime(CLOCK_REALTIME, &tv);
	return tv.tv_sec * 1000000000 + tv.tv_nsec;
}

static struct bothtimes usboth(void) {
	struct rusage ru;
	getrusage(RUSAGE_SELF, &ru);
	return (struct bothtimes) {
		.usr = ru.ru_utime.tv_sec * 1000000 + ru.ru_utime.tv_usec,
		.sys = ru.ru_stime.tv_sec * 1000000 + ru.ru_stime.tv_usec,
		.pfs = ru.ru_majflt + ru.ru_minflt,
	};
}

static void nop(void *ign) {
	(void) ign;
}

int main(int argc, char **argv) {
	unsigned long each[ITERS + 1];
#ifdef BREAKDOWN
	struct bothtimes breakdown[ITERS + 1];
#endif

	char fname[strlen(*argv) + sizeof ".prof." + 2];
	unsigned long nsthen = nsnow();
	for(int iter = 0; iter < ITERS; ++iter) {
#ifdef BREAKDOWN
		if(ProfilerFlush && ProfilerStart && ProfilerStop)
			switch(iter) {
			case 51:
			case 65:
				ProfilerFlush();
				ProfilerStop();
			case 5:
				sprintf(fname, "%s.prof.%02d", *argv, iter);
				ProfilerStart(fname);
			}
		putc('.', stderr);
		each[iter] = nsnow();
		breakdown[iter] = usboth();
#endif
		launch(nop, UINT64_MAX, NULL);
	}
	each[ITERS] = nsnow();
#ifdef BREAKDOWN
	breakdown[ITERS] = usboth();
	if(ProfilerFlush && ProfilerStop) {
		ProfilerFlush();
		ProfilerStop();
	}
	putc('\n', stderr);
#endif

	unsigned long nswhen = (nsnow() - nsthen) / ITERS;
#ifdef BREAKDOWN
	for(int iter = 0; iter < ITERS; ++iter) {
		unsigned long time = each[iter + 1] - each[iter];
		printf("%2d %lu.%03lu μs\n", iter, time / 1000, time % 1000);

		unsigned long usr = breakdown[iter + 1].usr - breakdown[iter].usr;
		unsigned long sys = breakdown[iter + 1].sys - breakdown[iter].sys;
		long pfs = breakdown[iter + 1].pfs - breakdown[iter].pfs;
		printf("(%lu μs usr, %lu μs sys, %ld pfs)\n", usr, sys, pfs);
	}
#endif
	printf("%ld.%03ld μs\n", nswhen / 1000, nswhen % 1000);

	return 0;
}
