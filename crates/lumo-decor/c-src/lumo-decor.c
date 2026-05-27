// lumo-decor.c — libdecor plugin Lumo OS.
//
// Implementa interface libdecor_plugin_interface + exporta symbol
// `libdecor_plugin_description` que loader libdecor procura via dlsym.
//
// Etapa atual (M1): minimal load + reserve 32px top border.
// Sem render real ainda — apps recebem 32px de offset top mas titlebar
// fica preta. Proxima iteracao: alocar wl_shm_pool + render via draw.h.
//
// Estado libdecor source: src/libdecor-fallback.c (template) e
// src/libdecor-cairo.c (full render reference).

#include <libdecor.h>
#include <wayland-client.h>

#include <errno.h>
#include <poll.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "draw.h"

// libdecor-plugin.h e privado, replico structs essenciais aqui pra
// nao depender de header interno do upstream.

struct libdecor_plugin {
    struct libdecor_plugin_private *priv;
};

typedef struct libdecor_plugin *(*libdecor_plugin_constructor)(struct libdecor *context);

#define LIBDECOR_PLUGIN_PRIORITY_HIGH 1000
#define LIBDECOR_PLUGIN_PRIORITY_MEDIUM 100
#define LIBDECOR_PLUGIN_PRIORITY_LOW 0

struct libdecor_plugin_priority {
    const char *desktop;
    int priority;
};

enum libdecor_plugin_capabilities {
    LIBDECOR_PLUGIN_CAPABILITY_BASE = 1 << 0,
};

struct libdecor_plugin_description {
    int api_version;
    char *description;
    enum libdecor_plugin_capabilities capabilities;
    const struct libdecor_plugin_priority *priorities;
    libdecor_plugin_constructor constructor;
    char *conflicting_symbols[1024];
};

struct libdecor_plugin_interface {
    void (*destroy)(struct libdecor_plugin *plugin);
    int (*get_fd)(struct libdecor_plugin *plugin);
    int (*dispatch)(struct libdecor_plugin *plugin, int timeout);
    struct libdecor_frame *(*frame_new)(struct libdecor_plugin *plugin);
    void (*frame_free)(struct libdecor_plugin *plugin,
                       struct libdecor_frame *frame);
    void (*frame_commit)(struct libdecor_plugin *plugin,
                         struct libdecor_frame *frame,
                         struct libdecor_state *state,
                         struct libdecor_configuration *configuration);
    void (*frame_property_changed)(struct libdecor_plugin *plugin,
                                   struct libdecor_frame *frame);
    void (*frame_popup_grab)(struct libdecor_plugin *plugin,
                             struct libdecor_frame *frame,
                             const char *seat_name);
    void (*frame_popup_ungrab)(struct libdecor_plugin *plugin,
                               struct libdecor_frame *frame,
                               const char *seat_name);
    bool (*frame_get_border_size)(struct libdecor_plugin *plugin,
                                  struct libdecor_frame *frame,
                                  struct libdecor_configuration *configuration,
                                  int *left,
                                  int *right,
                                  int *top,
                                  int *bottom);
    void (*reserved0)(void);
    void (*reserved1)(void);
    void (*reserved2)(void);
    void (*reserved3)(void);
    void (*reserved4)(void);
    void (*reserved5)(void);
    void (*reserved6)(void);
    void (*reserved7)(void);
    void (*reserved8)(void);
    void (*reserved9)(void);
};

// Exports do libdecor lib (linkados via -ldecor-0).
extern struct wl_display *libdecor_get_wl_display(struct libdecor *context);
extern void libdecor_plugin_init(struct libdecor_plugin *plugin,
                                 struct libdecor *context,
                                 struct libdecor_plugin_interface *iface);
extern void libdecor_plugin_release(struct libdecor_plugin *plugin);
extern void libdecor_notify_plugin_ready(struct libdecor *context);

// Plugin estrutura.
struct lumo_plugin {
    struct libdecor_plugin plugin;
    struct libdecor *context;
};

// ============================================================
// Interface implementacao
// ============================================================

static void lumo_destroy(struct libdecor_plugin *plugin)
{
    libdecor_plugin_release(plugin);
    free(plugin);
}

static int lumo_get_fd(struct libdecor_plugin *plugin)
{
    struct lumo_plugin *self = (struct lumo_plugin *)plugin;
    return wl_display_get_fd(libdecor_get_wl_display(self->context));
}

static int lumo_dispatch(struct libdecor_plugin *plugin, int timeout)
{
    struct lumo_plugin *self = (struct lumo_plugin *)plugin;
    struct wl_display *wl_display = libdecor_get_wl_display(self->context);
    struct pollfd fds[1];
    int dispatch_count = 0;

    while (wl_display_prepare_read(wl_display) != 0) {
        dispatch_count += wl_display_dispatch_pending(wl_display);
    }

    if (wl_display_flush(wl_display) < 0 && errno != EAGAIN) {
        wl_display_cancel_read(wl_display);
        return -errno;
    }

    fds[0].fd = wl_display_get_fd(wl_display);
    fds[0].events = POLLIN;
    fds[0].revents = 0;

    int ret = poll(fds, 1, timeout);
    if (ret > 0 && (fds[0].revents & POLLIN)) {
        wl_display_read_events(wl_display);
        dispatch_count += wl_display_dispatch_pending(wl_display);
    } else {
        wl_display_cancel_read(wl_display);
        if (ret < 0) return -errno;
    }
    return dispatch_count;
}

static struct libdecor_frame *lumo_frame_new(struct libdecor_plugin *plugin)
{
    (void)plugin;
    // Aloca minimo. libdecor faz init real internamente.
    return calloc(1, sizeof(void *));
}

static void lumo_frame_free(struct libdecor_plugin *plugin,
                            struct libdecor_frame *frame)
{
    (void)plugin;
    (void)frame;
    // libdecor gerencia frame lifecycle. Sem free explicito aqui.
}

static void lumo_frame_commit(struct libdecor_plugin *plugin,
                              struct libdecor_frame *frame,
                              struct libdecor_state *state,
                              struct libdecor_configuration *configuration)
{
    (void)plugin;
    (void)frame;
    (void)state;
    (void)configuration;
    // M1: no-op. Render real via wl_shm_pool ficara em M2.
    // Plugin reporta border_size top=32 — app vira 32px offset; sem
    // pixels desenhados ai, fica preto (compositor pinta SSD por cima
    // como overlay temporario ate M2 ficar pronto).
}

static void lumo_frame_property_changed(struct libdecor_plugin *plugin,
                                        struct libdecor_frame *frame)
{
    (void)plugin;
    (void)frame;
}

static void lumo_frame_popup_grab(struct libdecor_plugin *plugin,
                                  struct libdecor_frame *frame,
                                  const char *seat_name)
{
    (void)plugin;
    (void)frame;
    (void)seat_name;
}

static void lumo_frame_popup_ungrab(struct libdecor_plugin *plugin,
                                    struct libdecor_frame *frame,
                                    const char *seat_name)
{
    (void)plugin;
    (void)frame;
    (void)seat_name;
}

static bool lumo_frame_get_border_size(struct libdecor_plugin *plugin,
                                        struct libdecor_frame *frame,
                                        struct libdecor_configuration *configuration,
                                        int *left,
                                        int *right,
                                        int *top,
                                        int *bottom)
{
    (void)plugin;
    (void)frame;
    (void)configuration;
    if (left) *left = 0;
    if (right) *right = 0;
    if (top) *top = LUMO_TITLEBAR_HEIGHT;
    if (bottom) *bottom = 0;
    return true;
}

static struct libdecor_plugin_interface lumo_plugin_iface = {
    .destroy = lumo_destroy,
    .get_fd = lumo_get_fd,
    .dispatch = lumo_dispatch,
    .frame_new = lumo_frame_new,
    .frame_free = lumo_frame_free,
    .frame_commit = lumo_frame_commit,
    .frame_property_changed = lumo_frame_property_changed,
    .frame_popup_grab = lumo_frame_popup_grab,
    .frame_popup_ungrab = lumo_frame_popup_ungrab,
    .frame_get_border_size = lumo_frame_get_border_size,
};

static struct libdecor_plugin *lumo_plugin_new(struct libdecor *context)
{
    struct lumo_plugin *self = calloc(1, sizeof(*self));
    if (!self) return NULL;
    libdecor_plugin_init(&self->plugin, context, &lumo_plugin_iface);
    self->context = context;
    libdecor_notify_plugin_ready(context);
    return &self->plugin;
}

// ============================================================
// Plugin description exported (libdecor loader procura via dlsym).
// ============================================================

static const struct libdecor_plugin_priority lumo_priorities[] = {
    { "lumo", LIBDECOR_PLUGIN_PRIORITY_HIGH },
    { NULL, LIBDECOR_PLUGIN_PRIORITY_MEDIUM },
};

__attribute__((visibility("default")))
const struct libdecor_plugin_description libdecor_plugin_description = {
    .api_version = 1,
    .description = "Lumo OS native decorations",
    .capabilities = LIBDECOR_PLUGIN_CAPABILITY_BASE,
    .priorities = lumo_priorities,
    .constructor = lumo_plugin_new,
    .conflicting_symbols = { NULL },
};
