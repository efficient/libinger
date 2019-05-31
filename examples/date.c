#include <stdio.h>
#include <time.h>

int main(void) {
	time_t ts = time(NULL);
	struct tm *tm = localtime(&ts);
	printf(
		"%02d/%02d/%02d %02d:%02d:%02d %s\n",
		tm->tm_mon + 1,
		tm->tm_mday,
		tm->tm_year + 1900,
		tm->tm_hour % 12 ? tm->tm_hour % 12 : 12,
		tm->tm_min,
		tm->tm_sec,
		tm->tm_hour < 12 ? "AM" : "PM"
	);

	return 0;
}
