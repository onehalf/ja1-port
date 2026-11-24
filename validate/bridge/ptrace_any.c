/* LD_PRELOAD shim for DOSBox-X: opt the process in to being read by any process of the same user
   (/proc/PID/mem), which Yama ptrace_scope=1 otherwise allows only for ancestors. Used by tools/jactl.py. */
#include <sys/prctl.h>
#ifndef PR_SET_PTRACER_ANY
#define PR_SET_PTRACER_ANY ((unsigned long)-1)
#endif
__attribute__((constructor)) static void ptrace_any(void) { prctl(PR_SET_PTRACER, PR_SET_PTRACER_ANY, 0, 0, 0); }
