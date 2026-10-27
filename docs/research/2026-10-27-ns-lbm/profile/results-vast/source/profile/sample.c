#define _GNU_SOURCE
#include <pthread.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <ucontext.h>
#include <sys/syscall.h>
#include <dlfcn.h>

/* Linux x86-64 per-thread CPU timer, self-PC only; no unwinding in handler.
   Atomic counters must be lock-free. Payload is dumped after stage returns 0. */
#define LIMIT 1000000
struct Sample { uint64_t pc; uint32_t tid,stage; };
static struct Sample samples[LIMIT];
static _Atomic unsigned current_stage, used, overrun, failures, timers;
static _Thread_local int local_tid;
static _Thread_local timer_t local_timer;
static _Thread_local int timer_started;
static int signal_number;
static long interval_ns;
static const char *output;
static int (*real_create)(pthread_t *,const pthread_attr_t *,void *(*)(void *),void *);

void ns_prof_stage(unsigned stage) { atomic_store_explicit(&current_stage,stage,memory_order_relaxed); }
static void handler(int sig,siginfo_t *info,void *context) {
    (void)sig;
    unsigned stage=atomic_load_explicit(&current_stage,memory_order_relaxed);
    if (!stage) return;
    if (info->si_overrun>0) atomic_fetch_add_explicit(&overrun,(unsigned)info->si_overrun,memory_order_relaxed);
    unsigned idx=atomic_fetch_add_explicit(&used,1,memory_order_relaxed);
    if (idx<LIMIT) {
        ucontext_t *uc=context;
        samples[idx]=(struct Sample){(uint64_t)uc->uc_mcontext.gregs[REG_RIP],(uint32_t)local_tid,stage};
    }
}
static void start_timer(void) {
    if (!output) return;
    local_tid=(int)syscall(SYS_gettid);
    struct sigevent event={0};
    event.sigev_notify=SIGEV_THREAD_ID;
    event.sigev_signo=signal_number;
    event._sigev_un._tid=local_tid;
    if(timer_create(CLOCK_THREAD_CPUTIME_ID,&event,&local_timer)) {atomic_fetch_add(&failures,1);return;}
    struct itimerspec it={{0,interval_ns},{0,interval_ns}};
    if(timer_settime(local_timer,0,&it,NULL)) {atomic_fetch_add(&failures,1);timer_delete(local_timer);return;}
    timer_started=1;atomic_fetch_add(&timers,1);
}
struct Start {void *(*fn)(void *);void *arg;};
static void *start(void *arg) {
    struct Start entry=*(struct Start *)arg;free(arg);start_timer();
    void *result=entry.fn(entry.arg);
    if(timer_started)timer_delete(local_timer);
    return result;
}
int pthread_create(pthread_t *thread,const pthread_attr_t *attr,void *(*fn)(void *),void *arg) {
    if(!real_create)real_create=dlsym(RTLD_NEXT,"pthread_create");
    if(!output)return real_create(thread,attr,fn,arg);
    struct Start *entry=malloc(sizeof(*entry));
    if(!entry)return 12;
    *entry=(struct Start){fn,arg};
    int err=real_create(thread,attr,start,entry);if(err)free(entry);return err;
}
__attribute__((constructor)) static void init(void) {
    output=getenv("NS_SAMPLES");if(!output)return;
    if(!atomic_is_lock_free(&used)) {fputs("non-lock-free sample counter\n",stderr);abort();}
    interval_ns=2004008; /* approximately 499 Hz of each thread's CPU time */
    signal_number=SIGRTMIN+7;
    struct sigaction sa={0};sa.sa_sigaction=handler;sa.sa_flags=SA_SIGINFO|SA_RESTART;
    sigemptyset(&sa.sa_mask);
    if(sigaction(signal_number,&sa,NULL)){perror("sigaction");abort();}
    real_create=dlsym(RTLD_NEXT,"pthread_create");start_timer();
}
__attribute__((destructor)) static void finish(void) {
    if(!output)return;
    atomic_store(&current_stage,0);
    if(timer_started)timer_delete(local_timer);
    unsigned n=atomic_load(&used);unsigned stored=n<LIMIT?n:LIMIT;
    FILE *f=fopen(output,"wb");if(!f){perror("samples");return;}
    fwrite(samples,sizeof(*samples),stored,f);fclose(f);
    char path[4096];snprintf(path,sizeof(path),"%s.maps",output);
    FILE *src=fopen("/proc/self/maps","r");f=fopen(path,"w");
    if(src&&f){char line[4096];while(fgets(line,sizeof(line),src))fputs(line,f);}if(src)fclose(src);if(f)fclose(f);
    snprintf(path,sizeof(path),"%s.json",output);f=fopen(path,"w");
    if(f){fprintf(f,"{\"records\":%u,\"stored\":%u,\"overruns_in_active_stages\":%u,\"timer_failures\":%u,\"timers\":%u,\"interval_ns\":%ld,\"main_tid\":%d,\"format\":\"little-endian u64 PC,u32 tid,u32 stage\"}\n",n,stored,atomic_load(&overrun),atomic_load(&failures),atomic_load(&timers),interval_ns,(int)getpid());fclose(f);}
}
