# Spike-only patch of opencascade-sys 0.3.0 (SP-1)

Throwaway. Tests whether the MSVC build failure is only the `Handle_T` alias problem.

Under MSVC (`_MSC_VER >= 1800`), OpenCASCADE 7.8 defines `Handle_T` as a class derived from
`opencascade::handle<T>` (Standard_Handle.hxx, DEFINE_STANDARD_HANDLECLASS); elsewhere it is a typedef.
The crate's headers build `std::unique_ptr<Handle_T>(new opencascade::handle<T>(...))`, which converts a
base pointer to a derived one: error C2440 on MSVC, accepted by GCC and Clang.

The only change: `new opencascade::handle<T>(` became `new Handle_T(` in `include/*.hxx` (sed).

Also: `build.rs` now emits `cargo:rerun-if-changed=include`, so header edits rebuild the bridge.
