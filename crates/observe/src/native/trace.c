#include <os/log.h>
#include <os/signpost.h>
#include <pthread.h>
#include <stdint.h>
#include <time.h>

static os_log_t logger;
static pthread_once_t once = PTHREAD_ONCE_INIT;
static void initialize(void) {
    logger = os_log_create("io.plectron.observe", OS_LOG_CATEGORY_POINTS_OF_INTEREST);
}
int plectron_observe_begin(uint64_t id, uint64_t session, uint64_t parent, const char *name) {
    pthread_once(&once, initialize);
    if (!os_signpost_enabled(logger)) return 0;
    os_signpost_interval_begin(logger, id, "scope", "session=%llu parent=%llu name=%{public}s", session, parent, name);
    return 1;
}
void plectron_observe_end(uint64_t id, const char *outcome) {
    os_signpost_interval_end(logger, id, "scope", "outcome=%{public}s", outcome);
}
uint64_t plectron_observe_cpu(void) {
    struct timespec value;
    if (clock_gettime(CLOCK_THREAD_CPUTIME_ID, &value) != 0) return UINT64_MAX;
    return (uint64_t)value.tv_sec * 1000000000 + value.tv_nsec;
}
uint64_t plectron_observe_thread(void) {
    uint64_t id;
    if (pthread_threadid_np(NULL, &id) != 0) return 0;
    return id;
}
