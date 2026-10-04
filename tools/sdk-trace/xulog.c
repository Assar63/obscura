// LD_PRELOAD shim: log UVC XU queries (UVCIOC_CTRL_QUERY) made through ioctl().
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <linux/uvcvideo.h>
#include <linux/usb/video.h>

static int (*real_ioctl)(int, unsigned long, ...);

static void dump(const char *tag, struct uvc_xu_control_query *q) {
    fprintf(stderr, "XU %s unit=%u sel=%u q=0x%02x len=%u:", tag, q->unit, q->selector, q->query, q->size);
    unsigned n = q->size;
    while (n > 0 && q->data[n - 1] == 0) n--; // trim zero padding
    for (unsigned i = 0; i < n; i++) fprintf(stderr, " %02x", q->data[i]);
    fprintf(stderr, "\n");
}

int ioctl(int fd, unsigned long req, ...) {
    va_list ap;
    va_start(ap, req);
    void *arg = va_arg(ap, void *);
    va_end(ap);
    if (!real_ioctl) real_ioctl = dlsym(RTLD_NEXT, "ioctl");
    struct uvc_xu_control_query *q = arg;
    int is_xu = req == UVCIOC_CTRL_QUERY;
    if (is_xu && q->query == UVC_SET_CUR) dump("SET", q);
    int r = real_ioctl(fd, req, arg);
    if (is_xu && q->query == UVC_GET_CUR) {
        if (r == 0) dump("GET", q);
        else fprintf(stderr, "XU GET sel=%u failed\n", q->selector);
    }
    return r;
}
