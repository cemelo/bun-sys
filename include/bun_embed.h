#ifndef BUN_EMBED_H
#define BUN_EMBED_H

#include <stddef.h>
#include <stdint.h>

typedef struct BunRuntime BunRuntime;

/* --- Lifecycle --- */
BunRuntime* bun_runtime_start(int argc, char** argv, uint64_t* out_global);
int bun_runtime_load_file(BunRuntime* rt, const char* path);
int bun_runtime_run_event_loop(BunRuntime* rt);
void bun_runtime_request_stop(BunRuntime* rt);
void bun_runtime_stop(BunRuntime* rt);
void* bun_runtime_global(BunRuntime* rt);
void bun_runtime_schedule(BunRuntime* rt, void (*callback)(void* ctx), void* ctx);

/* --- Handler registration (called during load_file) --- */
typedef void (*HandlerRegistrar)(
    const char* kind,
    const char* name,
    const char* event_type,
    void* fn_handle
);
void bun_runtime_set_registrar(BunRuntime* rt, HandlerRegistrar cb);

/* --- Async native functions (JSON-based, legacy) --- */
typedef void (*AsyncNativeFunction)(
    void* ctx,
    uint64_t promise_id,
    const char* json_args,
    size_t json_args_len
);
void bun_runtime_register_async_native(
    BunRuntime* rt, const char* name, AsyncNativeFunction fn, void* ctx);

/* --- Native functions (JSValue-based) --- */
typedef uint64_t (*NativeFunction)(
    void* ctx, void* global, const uint64_t* args, size_t argc);
typedef void (*NativeDestructor)(void* ctx);
void bun_runtime_register_native(
    BunRuntime* rt, const char* name, NativeFunction fn, void* ctx,
    NativeDestructor destructor);

typedef void (*AsyncNativeJsValueFunction)(
    void* ctx, uint64_t promise_id, void* global,
    const uint64_t* args, size_t argc);
void bun_runtime_register_async_native_jsvalue(
    BunRuntime* rt, const char* name, AsyncNativeJsValueFunction fn, void* ctx);

/* --- Promise resolution --- */
void bun_runtime_resolve_promise(
    BunRuntime* rt, uint64_t promise_id, const char* json_result, size_t json_result_len);
void bun_runtime_resolve_promise_jsvalue(
    BunRuntime* rt, uint64_t promise_id, uint64_t raw_value);
void bun_runtime_reject_promise(
    BunRuntime* rt, uint64_t promise_id, const char* error_message);

/* --- Invoke a handler (thread-safe, enqueues via ConcurrentTask) --- */
void bun_runtime_invoke(
    BunRuntime* rt, uint64_t promise_id, void* fn_handle,
    const char* json_args, size_t json_args_len);

typedef void (*InvokeComplete)(
    void* ctx, uint64_t promise_id, int success,
    const char* json_result, size_t json_result_len);
void bun_runtime_set_invoke_complete(BunRuntime* rt, InvokeComplete cb, void* ctx);

/* --- JSValue primitives (pure bit ops, no global needed) --- */
uint64_t bun_jsvalue_undefined(void);
uint64_t bun_jsvalue_null(void);
uint64_t bun_jsvalue_bool(int v);
uint64_t bun_jsvalue_int32(int32_t v);
uint64_t bun_jsvalue_double(double v);

/* --- JSValue heap constructors (need global) --- */
uint64_t bun_jsvalue_string(void* global, const char* s, size_t len);
uint64_t bun_jsvalue_object(void* global);
uint64_t bun_jsvalue_array(void* global, size_t len);

/* --- JSValue type checks (pure bit ops) --- */
int bun_jsvalue_is_undefined(uint64_t v);
int bun_jsvalue_is_null(uint64_t v);
int bun_jsvalue_is_boolean(uint64_t v);
int bun_jsvalue_is_number(uint64_t v);
int bun_jsvalue_is_string(uint64_t v);
int bun_jsvalue_is_object(uint64_t v);
int bun_jsvalue_is_function(uint64_t v);
int bun_jsvalue_is_cell(uint64_t v);

/* --- JSValue conversions --- */
int bun_jsvalue_to_bool(uint64_t v);
int32_t bun_jsvalue_as_int32(uint64_t v);
double bun_jsvalue_to_double(void* global, uint64_t v);
const char* bun_jsvalue_to_string(void* global, uint64_t v, size_t* out_len);
void bun_free_string(const char* s);

/* --- JSValue property access --- */
uint64_t bun_jsvalue_get(void* global, uint64_t obj, const char* key, size_t key_len);
void bun_jsvalue_set(void* global, uint64_t obj, const char* key, size_t key_len, uint64_t val);
uint64_t bun_jsvalue_get_index(void* global, uint64_t arr, uint32_t idx);
void bun_jsvalue_set_index(void* global, uint64_t arr, uint32_t idx, uint64_t val);

/* --- JSValue function calls --- */
uint64_t bun_jsvalue_call(
    void* global, uint64_t func, uint64_t this_val,
    const uint64_t* args, size_t argc);

/* --- JSValue exception handling --- */
int bun_jsvalue_has_exception(void* global);
uint64_t bun_jsvalue_get_exception(void* global);
void bun_jsvalue_clear_exception(void* global);

/* --- JSValue GC --- */
void bun_jsvalue_protect(BunRuntime* rt, uint64_t v);
void bun_jsvalue_unprotect(BunRuntime* rt, uint64_t v);

/* --- JSValue function creation --- */
uint64_t bun_jsvalue_create_function(
    BunRuntime* rt, NativeFunction fn, void* ctx, NativeDestructor destructor);

#endif
