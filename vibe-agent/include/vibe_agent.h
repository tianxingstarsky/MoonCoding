#ifndef VIBE_AGENT_H
#define VIBE_AGENT_H

#include <stdint.h>

#if defined(_WIN32)
#  if defined(VIBE_AGENT_BUILD)
#    define VIBE_API __declspec(dllexport)
#  else
#    define VIBE_API __declspec(dllimport)
#  endif
#else
#  define VIBE_API
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef struct VibeHandle VibeHandle;
typedef void (*VibeEventCallback)(const char *event_json, void *user_data);

VIBE_API uint32_t vibe_api_version(void);

/*
 * options_json:
 * {
 *   "workspace": "absolute/path",
 *   "session_id": "optional-stable-id",
 *   "base_url": "optional",
 *   "model": "optional",
 *   "api_key": "optional"
 * }
 */
VIBE_API VibeHandle *vibe_init(
    const char *options_json,
    VibeEventCallback callback,
    void *user_data);

/* Starts asynchronous agent work. Returns 0 on success. */
VIBE_API int32_t vibe_send(VibeHandle *handle, const char *input);
VIBE_API int32_t vibe_interrupt(VibeHandle *handle);

/*
 * Tree functions returning char* use the envelope:
 * {"ok":true,"data":...} or {"ok":false,"error":"..."}.
 * Release each returned string with vibe_string_free.
 */
VIBE_API char *vibe_tree_get_json(VibeHandle *handle);
VIBE_API char *vibe_sessions_get_json(VibeHandle *handle);
VIBE_API char *vibe_session_get_json(VibeHandle *handle, const char *session_id);
VIBE_API char *vibe_tree_add_node(VibeHandle *handle, const char *request_json);
VIBE_API char *vibe_tree_update_node(VibeHandle *handle, const char *request_json);
VIBE_API char *vibe_tree_delete_node(VibeHandle *handle, const char *request_json);
VIBE_API char *vibe_tree_release_fields(VibeHandle *handle, const char *request_json);

VIBE_API int32_t vibe_tree_review_node(VibeHandle *handle, const char *node_id);
VIBE_API int32_t vibe_tree_review_all(VibeHandle *handle);

VIBE_API char *vibe_apps_list_json(VibeHandle *handle);
VIBE_API char *vibe_apps_get_json(VibeHandle *handle, const char *name);
VIBE_API char *vibe_apps_read_entry(VibeHandle *handle, const char *name);

/* Direct micro-app runtime — no agent/chat loop. Returns 0 on success. */
VIBE_API int32_t vibe_apps_start(VibeHandle *handle, const char *name);
VIBE_API int32_t vibe_apps_send(VibeHandle *handle, const char *event_json);
VIBE_API int32_t vibe_apps_stop(VibeHandle *handle);
VIBE_API char *vibe_apps_status_json(VibeHandle *handle);

VIBE_API char *vibe_last_error(void);
VIBE_API void vibe_string_free(char *value);
VIBE_API void vibe_destroy(VibeHandle *handle);

#ifdef __cplusplus
}
#endif

#endif
