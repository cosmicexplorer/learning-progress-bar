cargo_subproject(
  name='terminal-wrapper',
  generated_resources=['thrift_ffi/thrift_ffi_bindings.h'],
  cargo_output_file='libterminal_wrapper.dylib',
  sources=rglobs('*.toml', '*.rs'),
  dependencies=[
    '//:rust-toolchain',
    'terminal/thrift:rust',
  ],
)

python_library(
  name='event-loop',
  dependencies=[
    ':streaming-interface',
  ],
)
