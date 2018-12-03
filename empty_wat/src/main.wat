(module
  (type (;0;) (func (param i32 i32 i32) (result i32)))
  (type (;1;) (func (param i32 i32) (result i32)))
  (type (;2;) (func (param i32 i32)))
  (type (;3;) (func (param i32 i32 i32 i32 i32) (result i32)))
  (type (;4;) (func))
  (type (;5;) (func (param i32 i32 i32 i32 i32)))
  (type (;6;) (func (param i32 i32 i32)))
  (import "env" "hostcall_req_get_header" (func $hostcall_req_get_header (type 5)))
  (import "env" "hostcall_req_get_headers" (func $hostcall_req_get_headers (type 6)))
  (import "env" "hostcall_req_get_method" (func $hostcall_req_get_method (type 6)))
  (import "env" "hostcall_req_get_body" (func $hostcall_req_get_body (type 6)))
  (import "env" "hostcall_req_get_path" (func $hostcall_req_get_path (type 6)))
  (import "env" "hostcall_resp_set_header" (func $hostcall_resp_set_header (type 3)))
  (import "env" "hostcall_resp_set_body" (func $hostcall_resp_set_body (type 0)))
  (import "env" "hostcall_resp_set_response_code" (func $hostcall_resp_set_response_code (type 1)))
  (import "env" "hostcall_panic_hook" (func $hostcall_panic_hook (type 2)))

  (export "run" (func $run))

  ;; allocate a page to hold our constants
  (memory (;0;) 1)

  ;; argument for `req_get_header("X-Fastly-Debug")`
  (data (i32.const 0x00) "X-Fastly-Debug")

  ;; `resp_set_header("X-Gussie", &["is", "a", "very", "good", "dog"])`
  ;; argument 1
  (data (i32.const 0x0E) "X-Gussie")
  ;; contents of argument 2
  (data (i32.const 0x16) "is")
  (data (i32.const 0x18) "a")
  (data (i32.const 0x19) "very")
  (data (i32.const 0x1d) "good")
  (data (i32.const 0x21) "dog")
  ;; argument 2 (slice of slices)
  (data (i32.const 0x24)
    "\16\00\00\00" "\02\00\00\00"
    "\18\00\00\00" "\01\00\00\00"
    "\19\00\00\00" "\04\00\00\00"
    "\1d\00\00\00" "\04\00\00\00"
    "\21\00\00\00" "\03\00\00\00")

  (func $run (type 4)

    ;; `req_get_header("X-Fastly-Debug")`

    i32.const 0x4c
    i32.const 0x50
    i32.const 0
    i32.const 0x00
    i32.const 14
    call $hostcall_req_get_header

    ;; `resp_set_header("X-Gussie", &["is", "a", "very", "good", "dog"])`
    i32.const 0
    i32.const 0x0E
    i32.const 8
    i32.const 0x24
    i32.const 5
    call $hostcall_resp_set_header

    return)
)
