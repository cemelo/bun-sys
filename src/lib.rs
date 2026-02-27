#![allow(non_camel_case_types)]

use std::os::raw::{c_char, c_int, c_void};

/// Opaque handle to a Bun runtime instance.
#[repr(C)]
pub struct BunRuntime {
    _opaque: [u8; 0],
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

extern "C" {
    /// Starts a new Bun VM. Returns null on failure.
    /// `out_global` receives the raw u64 encoding of globalThis.
    pub fn bun_runtime_start(
        argc: c_int,
        argv: *mut *mut c_char,
        out_global: *mut u64,
    ) -> *mut BunRuntime;

    /// Loads and evaluates a JS/TS module. Returns 0 on success, -1 on failure.
    pub fn bun_runtime_load_file(rt: *mut BunRuntime, path: *const c_char) -> c_int;

    /// Ticks the event loop. Returns 1 to continue, 0 if shutdown was requested.
    pub fn bun_runtime_run_event_loop(rt: *mut BunRuntime) -> c_int;

    /// Signals the event loop to stop. Thread-safe.
    pub fn bun_runtime_request_stop(rt: *mut BunRuntime);

    /// Destroys the runtime and frees resources. Must be called from the JS thread.
    pub fn bun_runtime_stop(rt: *mut BunRuntime);

    /// Returns the JSGlobalObject pointer as an opaque handle.
    pub fn bun_runtime_global(rt: *mut BunRuntime) -> *mut c_void;

    /// Schedules a callback on the JS event loop thread. Thread-safe.
    pub fn bun_runtime_schedule(
        rt: *mut BunRuntime,
        callback: Option<extern "C" fn(ctx: *mut c_void)>,
        ctx: *mut c_void,
    );
}

// ---------------------------------------------------------------------------
// Handler registration
// ---------------------------------------------------------------------------

extern "C" {
    /// Sets the handler registrar callback. Called during module loading.
    pub fn bun_runtime_set_registrar(
        rt: *mut BunRuntime,
        cb: Option<
            extern "C" fn(
                kind: *const c_char,
                name: *const c_char,
                event_type: *const c_char,
                fn_handle: *mut c_void,
            ),
        >,
    );
}

// ---------------------------------------------------------------------------
// Native function registration (JSON-based, legacy)
// ---------------------------------------------------------------------------

extern "C" {
    /// Registers an async native function (JSON args) on the global object.
    pub fn bun_runtime_register_async_native(
        rt: *mut BunRuntime,
        name: *const c_char,
        func: Option<
            extern "C" fn(ctx: *mut c_void, promise_id: u64, json_args: *const u8, len: usize),
        >,
        ctx: *mut c_void,
    );
}

// ---------------------------------------------------------------------------
// Native function registration (JSValue-based)
// ---------------------------------------------------------------------------

extern "C" {
    /// Registers a synchronous native function (JSValue args) on the global object.
    pub fn bun_runtime_register_native(
        rt: *mut BunRuntime,
        name: *const c_char,
        func: Option<
            extern "C" fn(ctx: *mut c_void, global: *mut c_void, args: *const u64, argc: usize) -> u64,
        >,
        ctx: *mut c_void,
        destructor: Option<extern "C" fn(ctx: *mut c_void)>,
    );

    /// Registers an async native function (JSValue args) on the global object.
    pub fn bun_runtime_register_async_native_jsvalue(
        rt: *mut BunRuntime,
        name: *const c_char,
        func: Option<
            extern "C" fn(
                ctx: *mut c_void,
                promise_id: u64,
                global: *mut c_void,
                args: *const u64,
                argc: usize,
            ),
        >,
        ctx: *mut c_void,
    );
}

// ---------------------------------------------------------------------------
// Promise resolution
// ---------------------------------------------------------------------------

extern "C" {
    /// Resolves a promise with a JSON string. Thread-safe.
    pub fn bun_runtime_resolve_promise(
        rt: *mut BunRuntime,
        promise_id: u64,
        json_result: *const u8,
        json_result_len: usize,
    );

    /// Resolves a promise with a raw JSValue. Thread-safe.
    pub fn bun_runtime_resolve_promise_jsvalue(
        rt: *mut BunRuntime,
        promise_id: u64,
        raw_value: u64,
    );

    /// Rejects a promise with an error message. Thread-safe.
    pub fn bun_runtime_reject_promise(
        rt: *mut BunRuntime,
        promise_id: u64,
        error_message: *const c_char,
    );
}

// ---------------------------------------------------------------------------
// Handler invocation
// ---------------------------------------------------------------------------

extern "C" {
    /// Invokes a JS handler function with JSON args. Thread-safe.
    pub fn bun_runtime_invoke(
        rt: *mut BunRuntime,
        promise_id: u64,
        fn_handle: *mut c_void,
        json_args: *const u8,
        json_args_len: usize,
    );

    /// Sets the invoke-complete callback.
    pub fn bun_runtime_set_invoke_complete(
        rt: *mut BunRuntime,
        cb: Option<
            extern "C" fn(
                ctx: *mut c_void,
                promise_id: u64,
                success: c_int,
                json_result: *const u8,
                json_result_len: usize,
            ),
        >,
        ctx: *mut c_void,
    );
}

// ---------------------------------------------------------------------------
// JSValue primitives
// ---------------------------------------------------------------------------

extern "C" {
    /// Returns the raw JSValue encoding for `undefined`.
    pub fn bun_jsvalue_undefined() -> u64;

    /// Returns the raw JSValue encoding for `null`.
    pub fn bun_jsvalue_null() -> u64;

    /// Returns the raw JSValue encoding for a boolean.
    pub fn bun_jsvalue_bool(v: c_int) -> u64;

    /// Returns the raw JSValue encoding for a 32-bit integer.
    pub fn bun_jsvalue_int32(v: i32) -> u64;

    /// Returns the raw JSValue encoding for a double.
    pub fn bun_jsvalue_double(v: f64) -> u64;
}

// ---------------------------------------------------------------------------
// JSValue heap-allocated constructors
// ---------------------------------------------------------------------------

extern "C" {
    /// Creates a JS string from a UTF-8 byte slice.
    pub fn bun_jsvalue_string(global: *mut c_void, s: *const u8, len: usize) -> u64;

    /// Creates an empty JS object.
    pub fn bun_jsvalue_object(global: *mut c_void) -> u64;

    /// Creates a JS array with the given length.
    pub fn bun_jsvalue_array(global: *mut c_void, len: usize) -> u64;
}

// ---------------------------------------------------------------------------
// JSValue type checks
// ---------------------------------------------------------------------------

extern "C" {
    pub fn bun_jsvalue_is_undefined(v: u64) -> c_int;
    pub fn bun_jsvalue_is_null(v: u64) -> c_int;
    pub fn bun_jsvalue_is_boolean(v: u64) -> c_int;
    pub fn bun_jsvalue_is_number(v: u64) -> c_int;
    pub fn bun_jsvalue_is_string(v: u64) -> c_int;
    pub fn bun_jsvalue_is_object(v: u64) -> c_int;
    pub fn bun_jsvalue_is_function(v: u64) -> c_int;
    pub fn bun_jsvalue_is_cell(v: u64) -> c_int;
}

// ---------------------------------------------------------------------------
// JSValue conversions
// ---------------------------------------------------------------------------

extern "C" {
    /// Converts a JSValue to a boolean (pure bit op).
    pub fn bun_jsvalue_to_bool(v: u64) -> c_int;

    /// Extracts an int32 from a JSValue (returns 0 if not int32).
    pub fn bun_jsvalue_as_int32(v: u64) -> i32;

    /// Converts a JSValue to a double (returns 0.0 if not a number).
    pub fn bun_jsvalue_to_double(global: *mut c_void, v: u64) -> f64;

    /// Converts a JSValue to a UTF-8 string. Sets `out_len` to the length.
    /// The returned pointer is owned by JSC and must be freed with `bun_free_string`.
    pub fn bun_jsvalue_to_string(
        global: *mut c_void,
        v: u64,
        out_len: *mut usize,
    ) -> *const u8;

    /// Frees a string returned by `bun_jsvalue_to_string`.
    pub fn bun_free_string(s: *const u8);
}

// ---------------------------------------------------------------------------
// JSValue property access
// ---------------------------------------------------------------------------

extern "C" {
    /// Gets a property by name. Returns undefined if not found.
    pub fn bun_jsvalue_get(
        global: *mut c_void,
        obj: u64,
        key: *const u8,
        key_len: usize,
    ) -> u64;

    /// Sets a property by name.
    pub fn bun_jsvalue_set(
        global: *mut c_void,
        obj: u64,
        key: *const u8,
        key_len: usize,
        val: u64,
    );

    /// Gets an array element by index.
    pub fn bun_jsvalue_get_index(global: *mut c_void, arr: u64, idx: u32) -> u64;

    /// Sets an array element by index.
    pub fn bun_jsvalue_set_index(global: *mut c_void, arr: u64, idx: u32, val: u64);
}

// ---------------------------------------------------------------------------
// JSValue function calls
// ---------------------------------------------------------------------------

extern "C" {
    /// Calls a JS function with the given arguments.
    pub fn bun_jsvalue_call(
        global: *mut c_void,
        func: u64,
        this: u64,
        args: *const u64,
        argc: usize,
    ) -> u64;
}

// ---------------------------------------------------------------------------
// JSValue exception handling
// ---------------------------------------------------------------------------

extern "C" {
    /// Checks if there is a pending exception.
    pub fn bun_jsvalue_has_exception(global: *mut c_void) -> c_int;

    /// Gets the pending exception value.
    pub fn bun_jsvalue_get_exception(global: *mut c_void) -> u64;

    /// Clears the pending exception.
    pub fn bun_jsvalue_clear_exception(global: *mut c_void);
}

// ---------------------------------------------------------------------------
// JSValue GC (runtime-scoped)
// ---------------------------------------------------------------------------

extern "C" {
    /// Protects a JSValue from garbage collection.
    pub fn bun_jsvalue_protect(rt: *mut BunRuntime, v: u64);

    /// Removes GC protection from a JSValue.
    pub fn bun_jsvalue_unprotect(rt: *mut BunRuntime, v: u64);
}

// ---------------------------------------------------------------------------
// Anonymous function creation
// ---------------------------------------------------------------------------

extern "C" {
    /// Creates an anonymous JS function backed by a C callback.
    /// The destructor is called when the runtime shuts down.
    pub fn bun_jsvalue_create_function(
        rt: *mut BunRuntime,
        callback: Option<
            extern "C" fn(ctx: *mut c_void, global: *mut c_void, args: *const u64, argc: usize) -> u64,
        >,
        ctx: *mut c_void,
        destructor: Option<extern "C" fn(ctx: *mut c_void)>,
    ) -> u64;
}
