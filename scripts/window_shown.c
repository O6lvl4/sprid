// window_shown PID — wait until process PID has a window on screen, then
// print the time (seconds since the epoch) and exit. For bench.py:
// a terminal has started when its window shows, not when its program runs.
#include <CoreGraphics/CoreGraphics.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/time.h>
#include <unistd.h>

static int shown(int pid) {
    CFArrayRef list = CGWindowListCopyWindowInfo(
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID);
    if (!list) return 0;
    int found = 0;
    for (CFIndex i = 0; i < CFArrayGetCount(list) && !found; i++) {
        CFDictionaryRef w = CFArrayGetValueAtIndex(list, i);
        int owner = 0, layer = -1;
        CFNumberGetValue(CFDictionaryGetValue(w, kCGWindowOwnerPID), kCFNumberIntType, &owner);
        CFNumberGetValue(CFDictionaryGetValue(w, kCGWindowLayer), kCFNumberIntType, &layer);
        // Layer 0: an ordinary window, not a menu-bar item or an overlay.
        found = owner == pid && layer == 0;
    }
    CFRelease(list);
    return found;
}

int main(int argc, char **argv) {
    if (argc < 2) return 2;
    int pid = atoi(argv[1]);
    for (int i = 0; i < 30000; i++) {
        if (shown(pid)) {
            struct timeval tv;
            gettimeofday(&tv, NULL);
            printf("%.6f\n", tv.tv_sec + tv.tv_usec / 1e6);
            return 0;
        }
        usleep(1000);
    }
    return 1;
}
