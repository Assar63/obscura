// Listen to the OBSBOT SDK's event and status callbacks, to find out which
// events a camera pushes (target lost, gestures, Vox SE connect/battery/
// mute...) and how they reach the host. Run it under xulog.so and/or
// strace while the user makes gestures or uses a microphone.
//
// evprobe [seconds]   (default 120)
//
// Event numbers are RmEventType in dev.hpp; tools/sdk-trace/evnames.py
// turns them into names. Status pushes are printed as the bytes that
// changed in Device::CameraStatus (the selector 6 layout).
//
// Product type defaults to the Tiny 3; set OBSBOT_PRODUCT=<ObsbotProductType
// number> to listen to another model.
#include <dev/devs.hpp>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <mutex>
#include <thread>

static std::mutex out;
static unsigned char last[sizeof(Device::CameraStatus)];
static bool have_last = false;

static double now() {
    using namespace std::chrono;
    return duration_cast<milliseconds>(system_clock::now().time_since_epoch()).count() / 1000.0;
}
static void mark(const char *s) {
    std::lock_guard<std::mutex> l(out);
    fprintf(stderr, "=== %.3f %s\n", now(), s);
    fflush(stderr);
}
static void onChanged(std::string, bool, void *) {}
static ObsbotProductType product() {
    const char *p = getenv("OBSBOT_PRODUCT");
    return p ? (ObsbotProductType)atoi(p) : ObsbotProdTiny3;
}

static void onStatus(const char *kind, const void *data) {
    std::lock_guard<std::mutex> l(out);
    const unsigned char *b = static_cast<const unsigned char *>(data);
    if (!have_last) {
        fprintf(stderr, "=== %.3f %s status (first):", now(), kind);
        for (size_t i = 0; i < 48; i++) fprintf(stderr, " %02x", b[i]);
        fprintf(stderr, "\n");
    } else if (memcmp(last, b, sizeof last) != 0) {
        fprintf(stderr, "=== %.3f %s status changed:", now(), kind);
        for (size_t i = 0; i < sizeof last; i++)
            if (last[i] != b[i]) fprintf(stderr, " [%zu]%02x->%02x", i, last[i], b[i]);
        fprintf(stderr, "\n");
    }
    memcpy(last, b, sizeof last);
    have_last = true;
    fflush(stderr);
}

int main(int argc, char **argv) {
    int seconds = argc > 1 ? atoi(argv[1]) : 120;
    Devices::get().setDevChangedCallback(onChanged, nullptr);
    Devices::get().setEnableMdnsScan(false);
    std::shared_ptr<Device> dev;
    for (int i = 0; i < 30 && !dev; i++) {
        std::this_thread::sleep_for(std::chrono::milliseconds(500));
        for (auto &d : Devices::get().getDevList())
            if (d->productType() == product()) dev = d;
    }
    if (!dev) { mark("no camera of the selected product type found"); return 1; }
    mark(("found " + dev->devName()).c_str());

    dev->setDevEventNotifyCallbackFunc(
        [](void *, int32_t type, const void *result) {
            std::lock_guard<std::mutex> l(out);
            fprintf(stderr, "=== %.3f EVENT %d", now(), type);
            if (result) {
                const unsigned char *r = static_cast<const unsigned char *>(result);
                fprintf(stderr, " result bytes:");
                for (int i = 0; i < 8; i++) fprintf(stderr, " %02x", r[i]);
            }
            fprintf(stderr, "\n");
            fflush(stderr);
        },
        nullptr);
    dev->setDevStatusCallbackFunc([](void *, const void *d) { onStatus("slow", d); }, nullptr);
    dev->setFastDevStatusCallbackFunc(
        [](void *, const void *d, const std::string &) { onStatus("fast", d); }, nullptr);
    dev->enableDevStatusCallback(true);

    char buf[64];
    snprintf(buf, sizeof buf, "listening for %d s", seconds);
    mark(buf);
    std::this_thread::sleep_for(std::chrono::seconds(seconds));
    dev->enableDevStatusCallback(false);
    mark("done");
    return 0;
}
