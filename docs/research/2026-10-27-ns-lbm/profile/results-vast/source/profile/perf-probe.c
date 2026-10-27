#define _GNU_SOURCE
#include <linux/perf_event.h>
#include <sys/syscall.h>
#include <unistd.h>
#include <string.h>
#include <stdio.h>
#include <errno.h>
int main(void) {
    struct perf_event_attr a={0};a.size=sizeof(a);a.type=PERF_TYPE_SOFTWARE;a.config=PERF_COUNT_SW_TASK_CLOCK;
    a.disabled=1;a.exclude_kernel=1;a.exclude_hv=1;
    int fd=syscall(__NR_perf_event_open,&a,0,-1,-1,0);
    if(fd<0){printf("perf_event_open task-clock (self, user-space): %s (errno=%d)\n",strerror(errno),errno);return 1;}
    close(fd);puts("perf_event_open allowed");return 0;
}
