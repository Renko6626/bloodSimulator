#define _GNU_SOURCE
#include <dlfcn.h>
#include <time.h>
#include <unistd.h>
#include <sys/syscall.h>
#include <poll.h>
#include <sys/epoll.h>
#include <stdio.h>
#include <stdlib.h>
#include <pthread.h>

/* Optional LD_PRELOAD: only waits >=1ms, wall and thread CPU, monotonic clock.
 * Idle worker waits are expected; aggregate by TID before interpreting results.
 * No GPU/driver configuration is changed. Use a separate traced control run. */
static FILE *output;
static __thread int nested;
static double ms(clockid_t id) { struct timespec t; clock_gettime(id,&t); return t.tv_sec*1000.0+t.tv_nsec/1e6; }
__attribute__((constructor)) static void init(void) {
    const char *path=getenv("BLOOD_WAIT_TRACE");
    if(path) { output=fopen(path,"w"); if(output) { setvbuf(output,NULL,_IOFBF,65536); fprintf(output,"tid,name,start_ms,wall_ms,thread_cpu_ms\n"); } }
}
static void report(const char *name,double start,double cpu) {
    double duration=ms(CLOCK_MONOTONIC)-start;
    if(output && duration>=1.0) fprintf(output,"%ld,%s,%.6f,%.6f,%.6f\n",syscall(SYS_gettid),name,start,duration,ms(CLOCK_THREAD_CPUTIME_ID)-cpu);
}
#define START nested++; double start=ms(CLOCK_MONOTONIC),cpu=ms(CLOCK_THREAD_CPUTIME_ID)
#define END(name) if(nested==1) report(name,start,cpu); nested--
int nanosleep(const struct timespec *req,struct timespec *rem) {
    static int (*real)(const struct timespec*,struct timespec*);
    if(!real) real=dlsym(RTLD_NEXT,"nanosleep");
    START; int ret=real(req,rem); END("nanosleep"); return ret;
}
int clock_nanosleep(clockid_t clock,int flags,const struct timespec *req,struct timespec *rem) {
    static int (*real)(clockid_t,int,const struct timespec*,struct timespec*);
    if(!real) real=dlsym(RTLD_NEXT,"clock_nanosleep");
    START; int ret=real(clock,flags,req,rem); END("clock_nanosleep"); return ret;
}
int usleep(useconds_t usec) {
    static int (*real)(useconds_t);
    if(!real) real=dlsym(RTLD_NEXT,"usleep");
    START; int ret=real(usec); END("usleep"); return ret;
}
int poll(struct pollfd *fds,nfds_t nfds,int timeout) {
    static int (*real)(struct pollfd*,nfds_t,int);
    if(!real) real=dlsym(RTLD_NEXT,"poll");
    START; int ret=real(fds,nfds,timeout); END("poll"); return ret;
}
int epoll_wait(int epfd,struct epoll_event *events,int maxevents,int timeout) {
    static int (*real)(int,struct epoll_event*,int,int);
    if(!real) real=dlsym(RTLD_NEXT,"epoll_wait");
    START; int ret=real(epfd,events,maxevents,timeout); END("epoll_wait"); return ret;
}
int pthread_cond_wait(pthread_cond_t *cond,pthread_mutex_t *mutex) {
    static int (*real)(pthread_cond_t*,pthread_mutex_t*);
    if(!real) real=dlsym(RTLD_NEXT,"pthread_cond_wait");
    START; int ret=real(cond,mutex); END("cond_wait"); return ret;
}
