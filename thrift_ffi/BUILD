cargo_subproject(
  name='thrift_ffi',
  generated_resources=['src/thrift_ffi_bindings.h'],
  cargo_output_file='libthrift_ffi.dylib',
  sources=rglobs('*.toml', '*.rs'),
  dependencies=[
    '//:rust-toolchain',
  ],
)
