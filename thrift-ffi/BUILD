cargo_subproject(
  name='thrift-ffi',
  generated_resources=['thrift-ffi_bindings.h'],
  cargo_output_file='libthrift-ffi.dylib',
  sources=rglobs('*.toml', '*.rs'),
  dependencies=[
    '//:rust-toolchain',
  ],
)
